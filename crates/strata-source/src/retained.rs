//! Immutable in-memory byte source for fixtures and already-retained snapshots.

use std::sync::Arc;

use sha2::{Digest, Sha256};
use strata_core::{AddressSpaceId, DomainError, SourceGeneration, SourceId};

use crate::{
    BoxFuture, ByteChunk, ByteSource, DigestState, ReadRequest, SourceCapabilities,
    SourceDescriptor,
};

/// Immutable retained bytes implementing the same bounded source contract as files.
#[derive(Debug, Clone)]
pub struct RetainedByteSource {
    descriptor: SourceDescriptor,
    bytes: Arc<[u8]>,
}

impl RetainedByteSource {
    /// Retains the bytes and seals their SHA-256 immediately.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError::RangeOverflow`] when the byte length cannot be
    /// represented by the source contract.
    pub fn new(
        id: SourceId,
        generation: SourceGeneration,
        display_name: impl Into<String>,
        bytes: Arc<[u8]>,
    ) -> Result<Self, DomainError> {
        let length = u64::try_from(bytes.len()).map_err(|_| DomainError::RangeOverflow)?;
        let content_digest = format!("{:x}", Sha256::digest(&bytes));
        Ok(Self {
            descriptor: SourceDescriptor {
                id,
                generation,
                display_name: display_name.into(),
                length: Some(length),
                primary_address_space: AddressSpaceId("file-offset".to_owned()),
                capabilities: SourceCapabilities(
                    SourceCapabilities::KNOWN_LENGTH
                        | SourceCapabilities::RANDOM_READ
                        | SourceCapabilities::SEQUENTIAL_READ
                        | SourceCapabilities::SPARSE_RANGES
                        | SourceCapabilities::STABLE_SNAPSHOT,
                ),
                content_digest: Some(content_digest),
                digest_state: DigestState::Sealed,
                unstable: false,
            },
            bytes,
        })
    }

    /// Reads normalized ranges and enforces the caller's total byte budget.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError`] when the request targets another source or
    /// generation, ranges are invalid, arithmetic overflows, or the read
    /// exceeds its declared budget.
    pub fn read_bounded(&self, request: &ReadRequest) -> Result<ByteChunk, DomainError> {
        if request.source_id != self.descriptor.id {
            return Err(DomainError::SourceMismatch);
        }
        if request.generation != self.descriptor.generation {
            return Err(DomainError::StaleGeneration);
        }
        let total = request
            .ranges
            .total_len()
            .ok_or(DomainError::RangeOverflow)?;
        if total > request.maximum_bytes {
            return Err(DomainError::ResourceLimit(format!(
                "read requests {total} bytes but budget is {}",
                request.maximum_bytes
            )));
        }
        let capacity = usize::try_from(total).map_err(|_| {
            DomainError::ResourceLimit("read does not fit platform memory".to_owned())
        })?;
        let mut output = Vec::with_capacity(capacity);
        let mut previous_end = 0_u64;
        for (index, range) in request.ranges.ranges.iter().enumerate() {
            if range.start > range.end
                || range.end > self.descriptor.length.unwrap_or(0)
                || (index > 0 && range.start < previous_end)
            {
                return Err(DomainError::InvalidRange {
                    start: range.start,
                    end: range.end,
                });
            }
            let start = usize::try_from(range.start).map_err(|_| DomainError::RangeOverflow)?;
            let end = usize::try_from(range.end).map_err(|_| DomainError::RangeOverflow)?;
            output.extend_from_slice(self.bytes.get(start..end).ok_or(
                DomainError::InvalidRange {
                    start: range.start,
                    end: range.end,
                },
            )?);
            previous_end = range.end;
        }
        Ok(ByteChunk {
            source_id: self.descriptor.id,
            generation: self.descriptor.generation,
            ranges: request.ranges.clone(),
            bytes: output,
            complete: true,
        })
    }
}

impl ByteSource for RetainedByteSource {
    fn descriptor(&self) -> SourceDescriptor {
        self.descriptor.clone()
    }

    fn read(&self, request: ReadRequest) -> BoxFuture<'_, Result<ByteChunk, DomainError>> {
        Box::pin(async move { self.read_bounded(&request) })
    }
}
