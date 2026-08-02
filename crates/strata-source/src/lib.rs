//! Source capability and bounded range-read interfaces.
#![forbid(unsafe_code)]

use std::{future::Future, pin::Pin};

use strata_core::{
    AddressSpaceId, ByteRangeSet, DomainError, Priority, SourceGeneration, SourceId,
};

mod local_file;
mod retained;

pub use local_file::{HashProgress, LocalFileIdentity, LocalFileSource};
pub use retained::RetainedByteSource;

/// Heap-pinned, sendable future returned by object-safe source traits.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
/// Bit set declaring operations supported by a byte source.
pub struct SourceCapabilities(pub u64);

impl SourceCapabilities {
    /// The source can report a stable logical length.
    pub const KNOWN_LENGTH: u64 = 1 << 0;
    /// The source supports reads from arbitrary offsets.
    pub const RANDOM_READ: u64 = 1 << 1;
    /// The source supports forward sequential reads.
    pub const SEQUENTIAL_READ: u64 = 1 << 2;
    /// One request may contain several discontiguous ranges.
    pub const SPARSE_RANGES: u64 = 1 << 3;
    /// Reads refer to a stable snapshot rather than a changing stream.
    pub const STABLE_SNAPSHOT: u64 = 1 << 4;
    /// The source may publish newer generations over time.
    pub const LIVE_UPDATES: u64 = 1 << 5;
    /// The source exposes mappings to additional address spaces.
    pub const ADDRESS_MAPPINGS: u64 = 1 << 6;
    /// Reading the source requires elevated host privileges.
    pub const PRIVILEGED: u64 = 1 << 7;
    /// Reading the source may require a network operation.
    pub const REMOTE: u64 = 1 << 8;

    #[must_use]
    /// Returns whether every bit in `capability` is present.
    pub const fn contains(self, capability: u64) -> bool {
        self.0 & capability == capability
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Source metadata safe to share across analysis and presentation layers.
pub struct SourceDescriptor {
    /// Opaque immutable-source identity.
    pub id: SourceId,
    /// Current source generation.
    pub generation: SourceGeneration,
    /// Redacted user-facing source name.
    pub display_name: String,
    /// Logical source length when known.
    pub length: Option<u64>,
    /// Address space used by read requests.
    pub primary_address_space: AddressSpaceId,
    /// Operations supported by the source.
    pub capabilities: SourceCapabilities,
    /// Lowercase content digest when available.
    pub content_digest: Option<String>,
    /// Lifecycle state of `content_digest`.
    pub digest_state: DigestState,
    /// Whether mutation or identity drift invalidated the source snapshot.
    pub unstable: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Lifecycle state of a source content digest.
pub enum DigestState {
    /// No digest work has started.
    Unknown,
    /// A progressive digest has incorporated only part of the source.
    Provisional,
    /// The complete stable source has been hashed.
    Sealed,
    /// Hashing failed or source mutation invalidated prior progress.
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Bounded request for normalized source ranges.
pub struct ReadRequest {
    /// Source that must satisfy the request.
    pub source_id: SourceId,
    /// Required source generation, used to reject stale reads.
    pub generation: SourceGeneration,
    /// Ordered non-overlapping ranges to read.
    pub ranges: ByteRangeSet,
    /// Scheduling importance of the read.
    pub priority: Priority,
    /// Hard upper bound on returned bytes.
    pub maximum_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Bytes returned for a bounded source read.
pub struct ByteChunk {
    /// Source that produced the bytes.
    pub source_id: SourceId,
    /// Source generation used for the read.
    pub generation: SourceGeneration,
    /// Ranges represented by the concatenated byte payload.
    pub ranges: ByteRangeSet,
    /// Concatenated bytes in range order.
    pub bytes: Vec<u8>,
    /// Whether all requested ranges were returned.
    pub complete: bool,
}

/// Object-safe boundary for immutable, bounded byte access.
pub trait ByteSource: Send + Sync {
    /// Returns a point-in-time source descriptor.
    fn descriptor(&self) -> SourceDescriptor;

    /// Reads normalized ranges without exceeding the request budget.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError`] when the source identity or generation differs,
    /// ranges are invalid, a resource limit is exceeded, or the backend fails.
    fn read(&self, request: ReadRequest) -> BoxFuture<'_, Result<ByteChunk, DomainError>>;
}

/// Registry boundary for active byte sources.
pub trait SourceManager: Send + Sync {
    /// Returns the active source for `id`, if present.
    fn get(&self, id: SourceId) -> Option<&dyn ByteSource>;

    /// Releases host resources associated with an active source.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError`] when the source cannot be closed cleanly.
    fn close(&self, id: SourceId) -> BoxFuture<'_, Result<(), DomainError>>;
}
