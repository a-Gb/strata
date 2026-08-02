//! Exact ordered byte-pair counting.

use strata_core::DomainError;

/// Exact counts for ordered byte pairs at one stride.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DigramCounts {
    /// Source-byte distance between the first and second byte of each pair.
    pub stride: usize,
    /// Heap-backed row-major counts, indexed as `first_byte * 256 + second_byte`.
    ///
    /// This vector always has exactly `256 * 256` elements.
    pub counts: Vec<u64>,
}

impl DigramCounts {
    /// Returns the exact count for the ordered pair `(first, second)`.
    #[must_use]
    pub fn count(&self, first: u8, second: u8) -> u64 {
        self.counts[digram_index(first, second)]
    }
}

/// Counts ordered byte pairs separated by `stride` source bytes.
///
/// For every valid `i`, this increments the cell for `(data[i],
/// data[i + stride])` exactly once. Empty input and input shorter than the
/// stride produce an all-zero matrix. `stride` must be nonzero.
///
/// # Errors
///
/// Returns [`DomainError::InvalidTransform`] when `stride` is zero.
pub fn digram_counts(data: &[u8], stride: usize) -> Result<DigramCounts, DomainError> {
    if stride == 0 {
        return Err(DomainError::InvalidTransform(
            "POC digram stride must be nonzero".to_owned(),
        ));
    }

    let mut result = DigramCounts {
        stride,
        counts: vec![0_u64; 256 * 256],
    };
    if stride >= data.len() {
        return Ok(result);
    }

    for index in 0..(data.len() - stride) {
        let cell = digram_index(data[index], data[index + stride]);
        result.counts[cell] = result.counts[cell]
            .checked_add(1)
            .ok_or(DomainError::RangeOverflow)?;
    }
    Ok(result)
}

fn digram_index(first: u8, second: u8) -> usize {
    usize::from(first) * 256 + usize::from(second)
}
