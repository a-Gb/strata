//! Checked half-open source ranges.

use crate::DomainError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
/// Checked half-open byte range `[start, end)` in an address space.
pub struct ByteRange {
    /// Inclusive first byte offset.
    pub start: u64,
    /// Exclusive byte offset immediately after the range.
    pub end: u64,
}

impl ByteRange {
    /// Creates a checked half-open byte range.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError::InvalidRange`] when `end` is before `start`.
    pub const fn new(start: u64, end: u64) -> Result<Self, DomainError> {
        if start > end {
            return Err(DomainError::InvalidRange { start, end });
        }
        Ok(Self { start, end })
    }

    #[must_use]
    /// Returns the number of bytes in the range.
    pub const fn len(self) -> u64 {
        self.end - self.start
    }

    #[must_use]
    /// Returns whether the range contains no bytes.
    pub const fn is_empty(self) -> bool {
        self.start == self.end
    }

    #[must_use]
    /// Returns whether `offset` belongs to this half-open range.
    pub const fn contains(self, offset: u64) -> bool {
        self.start <= offset && offset < self.end
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
/// Ordered collection of byte ranges associated with one semantic value.
pub struct ByteRangeSet {
    /// Must be normalized before crossing a public service boundary.
    pub ranges: Vec<ByteRange>,
}

impl ByteRangeSet {
    #[must_use]
    /// Returns the checked sum of all range lengths, or `None` on overflow.
    pub fn total_len(&self) -> Option<u64> {
        self.ranges
            .iter()
            .try_fold(0_u64, |acc, range| acc.checked_add(range.len()))
    }
}
