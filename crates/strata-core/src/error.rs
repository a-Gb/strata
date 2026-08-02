//! Stable error categories used at service boundaries.

#[derive(Debug, Clone, PartialEq, Eq)]
/// Stable failure categories shared across Strata service boundaries.
pub enum DomainError {
    /// A half-open range has an end before its start.
    InvalidRange {
        /// Requested range start.
        start: u64,
        /// Requested exclusive range end.
        end: u64,
    },
    /// Checked address or length arithmetic overflowed.
    RangeOverflow,
    /// Data or metadata belongs to a different immutable source.
    SourceMismatch,
    /// Work targeted a source generation that is no longer current.
    StaleGeneration,
    /// The active backend cannot provide the requested capability.
    UnsupportedCapability(String),
    /// A transform specification violates its semantic contract.
    InvalidTransform(String),
    /// A view specification cannot be represented safely.
    InvalidView(String),
    /// A declared memory, time, or output bound would be exceeded.
    ResourceLimit(String),
    /// Work was cancelled before publication.
    Cancelled,
    /// An invariant failed without a more specific public category.
    Internal(String),
}

impl core::fmt::Display for DomainError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for DomainError {}
