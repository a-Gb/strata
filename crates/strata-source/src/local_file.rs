//! Read-only local-file source with bounded reads and progressive identity hashing.

use std::{
    fs::{File, OpenOptions},
    io,
    os::unix::fs::{FileExt, MetadataExt},
    path::{Path, PathBuf},
    sync::{Mutex, RwLock},
};

use sha2::{Digest, Sha256};
use strata_core::{AddressSpaceId, ByteRangeSet, DomainError, SourceGeneration, SourceId};

use crate::{
    BoxFuture, ByteChunk, ByteSource, DigestState, ReadRequest, SourceCapabilities,
    SourceDescriptor,
};

const HASH_READ_CHUNK_BYTES: usize = 1024 * 1024;

/// Kernel-backed identity captured from an opened local file.
///
/// The device and inode keep path replacement distinct from the already-open
/// source. Length and modification time detect changes to that inode. This is
/// an identity guard, not a claim that another process cannot mutate the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LocalFileIdentity {
    /// Filesystem device identifier.
    pub device: u64,
    /// Filesystem inode identifier.
    pub inode: u64,
    /// Length observed when the source was opened.
    pub length: u64,
    /// Whole seconds in the filesystem modification timestamp.
    pub modified_seconds: i64,
    /// Nanosecond component in the filesystem modification timestamp.
    pub modified_nanoseconds: i64,
}

impl LocalFileIdentity {
    fn from_metadata(metadata: &std::fs::Metadata) -> Self {
        Self {
            device: metadata.dev(),
            inode: metadata.ino(),
            length: metadata.len(),
            modified_seconds: metadata.mtime(),
            modified_nanoseconds: metadata.mtime_nsec(),
        }
    }
}

/// Observable progress for the source's canonical SHA-256 digest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HashProgress {
    /// Current digest lifecycle state.
    pub state: DigestState,
    /// Number of source bytes incorporated so far.
    pub bytes_hashed: u64,
    /// Source length captured at open time.
    pub total_bytes: u64,
    /// Lowercase SHA-256 when the complete source has been sealed.
    pub content_digest: Option<String>,
}

#[derive(Debug)]
struct ProgressiveHash {
    hasher: Sha256,
    next_offset: u64,
    digest: Option<String>,
}

impl Default for ProgressiveHash {
    fn default() -> Self {
        Self {
            hasher: Sha256::new(),
            next_offset: 0,
            digest: None,
        }
    }
}

/// A read-only local source that never reads more than a request's byte budget.
#[derive(Debug)]
pub struct LocalFileSource {
    path: PathBuf,
    file: File,
    identity: LocalFileIdentity,
    descriptor: RwLock<SourceDescriptor>,
    hash: Mutex<ProgressiveHash>,
}

impl LocalFileSource {
    /// Opens a regular file without write access and captures its stable identity.
    ///
    /// # Errors
    ///
    /// Returns an I/O error when the path cannot be opened or inspected, or
    /// when it does not identify a regular file.
    pub fn open(
        path: impl AsRef<Path>,
        id: SourceId,
        generation: SourceGeneration,
    ) -> io::Result<Self> {
        let path = path.as_ref();
        let file = OpenOptions::new().read(true).open(path)?;
        let metadata = file.metadata()?;
        if !metadata.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "local source must be a regular file",
            ));
        }

        let identity = LocalFileIdentity::from_metadata(&metadata);
        let display_name = path
            .file_name()
            .and_then(std::ffi::OsStr::to_str)
            .filter(|name| !name.is_empty())
            .unwrap_or("local source")
            .to_owned();
        let capabilities = SourceCapabilities(
            SourceCapabilities::KNOWN_LENGTH
                | SourceCapabilities::RANDOM_READ
                | SourceCapabilities::SEQUENTIAL_READ
                | SourceCapabilities::SPARSE_RANGES,
        );

        Ok(Self {
            path: path.to_path_buf(),
            file,
            identity,
            descriptor: RwLock::new(SourceDescriptor {
                id,
                generation,
                display_name,
                length: Some(identity.length),
                primary_address_space: AddressSpaceId("file-offset".to_owned()),
                capabilities,
                content_digest: None,
                digest_state: DigestState::Unknown,
                unstable: false,
            }),
            hash: Mutex::new(ProgressiveHash::default()),
        })
    }

    /// Returns the path retained by the live host. Session serialization must
    /// continue to omit this value.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Returns the identity captured from the opened file handle.
    #[must_use]
    pub const fn identity(&self) -> LocalFileIdentity {
        self.identity
    }

    /// Returns an owned, point-in-time descriptor snapshot.
    #[must_use]
    pub fn descriptor_snapshot(&self) -> SourceDescriptor {
        match self.descriptor.read() {
            Ok(descriptor) => descriptor.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        }
    }

    /// Reads the requested ranges synchronously in their normalized order.
    ///
    /// The returned bytes are concatenated range-by-range and never exceed
    /// `maximum_bytes`. The request fails before allocation when its ranges are
    /// invalid, stale, outside the opened identity, or over budget.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError`] when identity verification fails, the request is
    /// stale or mismatched, ranges are invalid, arithmetic overflows, the byte
    /// budget is exceeded, or a file read fails.
    pub fn read_bounded(&self, request: &ReadRequest) -> Result<ByteChunk, DomainError> {
        self.verify_identity()?;
        let descriptor = self.descriptor_snapshot();
        if request.source_id != descriptor.id {
            return Err(DomainError::SourceMismatch);
        }
        if request.generation != descriptor.generation {
            return Err(DomainError::StaleGeneration);
        }

        validate_ranges(&request.ranges, self.identity.length)?;
        let total_bytes = request
            .ranges
            .total_len()
            .ok_or(DomainError::RangeOverflow)?;
        if total_bytes > request.maximum_bytes {
            return Err(DomainError::ResourceLimit(format!(
                "read requests {total_bytes} bytes but budget is {}",
                request.maximum_bytes
            )));
        }
        let capacity = usize::try_from(total_bytes).map_err(|_| {
            DomainError::ResourceLimit("read does not fit platform memory".to_owned())
        })?;
        let mut bytes = Vec::with_capacity(capacity);

        for range in &request.ranges.ranges {
            self.read_exact_range(range.start, range.len(), &mut bytes)?;
        }
        self.verify_identity()?;

        Ok(ByteChunk {
            source_id: descriptor.id,
            generation: descriptor.generation,
            ranges: request.ranges.clone(),
            bytes,
            complete: true,
        })
    }

    /// Advances the canonical SHA-256 by at most `maximum_bytes`.
    ///
    /// Repeated calls resume at the prior offset. Completion seals the digest
    /// into descriptor snapshots. A detected identity change permanently marks
    /// the source unstable and fails the digest.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError`] when the budget is zero, the source identity
    /// changes, checked arithmetic overflows, memory bounds are exceeded, or a
    /// file read fails.
    pub fn advance_hash(&self, maximum_bytes: u64) -> Result<HashProgress, DomainError> {
        if maximum_bytes == 0 {
            return Err(DomainError::ResourceLimit(
                "hash budget must be greater than zero".to_owned(),
            ));
        }
        self.verify_identity()?;

        let mut hash = match self.hash.lock() {
            Ok(hash) => hash,
            Err(poisoned) => poisoned.into_inner(),
        };
        if let Some(digest) = &hash.digest {
            return Ok(HashProgress {
                state: DigestState::Sealed,
                bytes_hashed: self.identity.length,
                total_bytes: self.identity.length,
                content_digest: Some(digest.clone()),
            });
        }

        let remaining = self.identity.length.saturating_sub(hash.next_offset);
        let target = remaining.min(maximum_bytes);
        let mut advanced = 0_u64;
        while advanced < target {
            let remaining_budget = target - advanced;
            let chunk_len_u64 = remaining_budget.min(HASH_READ_CHUNK_BYTES as u64);
            let chunk_len = usize::try_from(chunk_len_u64).map_err(|_| {
                DomainError::ResourceLimit("hash chunk does not fit platform memory".to_owned())
            })?;
            let mut buffer = vec![0_u8; chunk_len];
            let offset = hash
                .next_offset
                .checked_add(advanced)
                .ok_or(DomainError::RangeOverflow)?;
            self.read_exact_at(&mut buffer, offset)?;
            hash.hasher.update(&buffer);
            advanced = advanced
                .checked_add(chunk_len_u64)
                .ok_or(DomainError::RangeOverflow)?;
        }
        hash.next_offset = hash
            .next_offset
            .checked_add(advanced)
            .ok_or(DomainError::RangeOverflow)?;

        self.verify_identity()?;
        if hash.next_offset == self.identity.length {
            let digest = format!("{:x}", hash.hasher.clone().finalize());
            hash.digest = Some(digest.clone());
            self.update_digest(DigestState::Sealed, Some(digest.clone()));
            return Ok(HashProgress {
                state: DigestState::Sealed,
                bytes_hashed: hash.next_offset,
                total_bytes: self.identity.length,
                content_digest: Some(digest),
            });
        }

        self.update_digest(DigestState::Provisional, None);
        Ok(HashProgress {
            state: DigestState::Provisional,
            bytes_hashed: hash.next_offset,
            total_bytes: self.identity.length,
            content_digest: None,
        })
    }

    fn read_exact_range(
        &self,
        start: u64,
        length: u64,
        destination: &mut Vec<u8>,
    ) -> Result<(), DomainError> {
        let length = usize::try_from(length).map_err(|_| {
            DomainError::ResourceLimit("range does not fit platform memory".to_owned())
        })?;
        let original_len = destination.len();
        destination.resize(
            original_len
                .checked_add(length)
                .ok_or(DomainError::RangeOverflow)?,
            0,
        );
        let range = destination
            .get_mut(original_len..)
            .ok_or(DomainError::RangeOverflow)?;
        self.read_exact_at(range, start)
    }

    fn read_exact_at(&self, mut buffer: &mut [u8], mut offset: u64) -> Result<(), DomainError> {
        while !buffer.is_empty() {
            let read = self
                .file
                .read_at(buffer, offset)
                .map_err(|error| source_io_error("read", &error))?;
            if read == 0 {
                self.mark_unstable();
                return Err(DomainError::SourceMismatch);
            }
            let read_u64 = u64::try_from(read).map_err(|_| DomainError::RangeOverflow)?;
            offset = offset
                .checked_add(read_u64)
                .ok_or(DomainError::RangeOverflow)?;
            buffer = buffer.get_mut(read..).ok_or(DomainError::RangeOverflow)?;
        }
        Ok(())
    }

    fn verify_identity(&self) -> Result<(), DomainError> {
        let current = self
            .file
            .metadata()
            .map_err(|error| source_io_error("metadata", &error))?;
        if LocalFileIdentity::from_metadata(&current) != self.identity {
            self.mark_unstable();
            return Err(DomainError::SourceMismatch);
        }
        Ok(())
    }

    fn update_digest(&self, state: DigestState, digest: Option<String>) {
        let mut descriptor = match self.descriptor.write() {
            Ok(descriptor) => descriptor,
            Err(poisoned) => poisoned.into_inner(),
        };
        descriptor.digest_state = state;
        descriptor.content_digest = digest;
    }

    fn mark_unstable(&self) {
        let mut descriptor = match self.descriptor.write() {
            Ok(descriptor) => descriptor,
            Err(poisoned) => poisoned.into_inner(),
        };
        descriptor.unstable = true;
        descriptor.digest_state = DigestState::Failed;
        descriptor.content_digest = None;
    }
}

impl ByteSource for LocalFileSource {
    fn descriptor(&self) -> SourceDescriptor {
        self.descriptor_snapshot()
    }

    fn read(&self, request: ReadRequest) -> BoxFuture<'_, Result<ByteChunk, DomainError>> {
        Box::pin(async move { self.read_bounded(&request) })
    }
}

fn validate_ranges(ranges: &ByteRangeSet, source_length: u64) -> Result<(), DomainError> {
    let mut previous_end = 0_u64;
    for (index, range) in ranges.ranges.iter().enumerate() {
        if range.start > range.end || range.end > source_length {
            return Err(DomainError::InvalidRange {
                start: range.start,
                end: range.end,
            });
        }
        if index > 0 && range.start < previous_end {
            return Err(DomainError::InvalidRange {
                start: range.start,
                end: range.end,
            });
        }
        previous_end = range.end;
    }
    Ok(())
}

fn source_io_error(operation: &str, error: &io::Error) -> DomainError {
    DomainError::Internal(format!("local source {operation} failed: {error}"))
}

#[cfg(test)]
mod tests {
    use std::{
        error::Error,
        fs,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };

    use strata_core::{ByteRange, Priority};

    use super::*;

    static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn fixture_path(label: &str) -> PathBuf {
        let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "strata-source-{label}-{}-{counter}.bin",
            std::process::id()
        ))
    }

    fn request(ranges: Vec<ByteRange>, maximum_bytes: u64) -> ReadRequest {
        ReadRequest {
            source_id: SourceId(7),
            generation: SourceGeneration(3),
            ranges: ByteRangeSet { ranges },
            priority: Priority::Interactive,
            maximum_bytes,
        }
    }

    #[test]
    fn sparse_reads_are_exact_and_budgeted() -> Result<(), Box<dyn Error>> {
        let path = fixture_path("bounded");
        fs::write(&path, b"0123456789abcdef")?;
        let source = LocalFileSource::open(&path, SourceId(7), SourceGeneration(3))?;
        let chunk = source.read_bounded(&request(
            vec![ByteRange::new(1, 4)?, ByteRange::new(10, 13)?],
            6,
        ))?;
        assert_eq!(chunk.bytes, b"123abc");
        assert!(chunk.complete);

        let over_budget = source.read_bounded(&request(vec![ByteRange::new(0, 7)?], 6));
        assert!(matches!(over_budget, Err(DomainError::ResourceLimit(_))));
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn stale_requests_are_rejected() -> Result<(), Box<dyn Error>> {
        let path = fixture_path("stale");
        fs::write(&path, b"abc")?;
        let source = LocalFileSource::open(&path, SourceId(7), SourceGeneration(3))?;
        let mut stale = request(vec![ByteRange::new(0, 1)?], 1);
        stale.generation = SourceGeneration(4);
        assert_eq!(
            source.read_bounded(&stale),
            Err(DomainError::StaleGeneration)
        );
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn progressive_hash_seals_known_digest() -> Result<(), Box<dyn Error>> {
        let path = fixture_path("hash");
        fs::write(&path, b"abc")?;
        let source = LocalFileSource::open(&path, SourceId(7), SourceGeneration(3))?;

        let partial = source.advance_hash(1)?;
        assert_eq!(partial.state, DigestState::Provisional);
        assert_eq!(partial.bytes_hashed, 1);
        assert!(partial.content_digest.is_none());

        let sealed = source.advance_hash(2)?;
        assert_eq!(sealed.state, DigestState::Sealed);
        assert_eq!(
            sealed.content_digest.as_deref(),
            Some("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad")
        );
        assert_eq!(
            source.descriptor_snapshot().digest_state,
            DigestState::Sealed
        );
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn mutation_marks_source_unstable() -> Result<(), Box<dyn Error>> {
        let path = fixture_path("mutation");
        fs::write(&path, b"abc")?;
        let source = LocalFileSource::open(&path, SourceId(7), SourceGeneration(3))?;
        fs::write(&path, b"changed length")?;

        let result = source.read_bounded(&request(vec![ByteRange::new(0, 1)?], 1));
        assert_eq!(result, Err(DomainError::SourceMismatch));
        let descriptor = source.descriptor_snapshot();
        assert!(descriptor.unstable);
        assert_eq!(descriptor.digest_state, DigestState::Failed);
        fs::remove_file(path)?;
        Ok(())
    }
}
