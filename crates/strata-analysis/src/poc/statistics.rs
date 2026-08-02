//! Exact byte classification, histograms, and block entropy.

use strata_core::DomainError;

/// A coarse, stable byte category for atlas colouring.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ByteClass {
    /// The `0x00` byte, often used for empty or padded data.
    Zero,
    /// The `0xff` byte, often used for erased flash or padding.
    AllOnes,
    /// ASCII whitespace: tab, line feed, vertical tab, form feed, carriage return, or space.
    Whitespace,
    /// Printable 7-bit ASCII excluding space (`0x21..=0x7e`).
    PrintableAscii,
    /// Non-whitespace ASCII controls, including `0x7f`.
    Control,
    /// Bytes whose high bit is set (`0x80..=0xff`).
    HighBit,
}

/// Classifies one byte for a structural atlas.
#[must_use]
pub const fn classify_byte(byte: u8) -> ByteClass {
    match byte {
        0 => ByteClass::Zero,
        0xff => ByteClass::AllOnes,
        b'\t' | b'\n' | 0x0b | 0x0c | b'\r' | b' ' => ByteClass::Whitespace,
        0x21..=0x7e => ByteClass::PrintableAscii,
        0x80..=0xff => ByteClass::HighBit,
        _ => ByteClass::Control,
    }
}

/// An exact frequency table for every possible byte value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ByteHistogram {
    /// Counts indexed by the corresponding byte value.
    pub bins: [u64; 256],
}

/// Counts each byte in `data` exactly once.
#[must_use]
pub fn byte_histogram(data: &[u8]) -> ByteHistogram {
    let mut bins = [0_u64; 256];
    for &byte in data {
        bins[usize::from(byte)] += 1;
    }
    ByteHistogram { bins }
}

/// The Shannon entropy of one contiguous input block, measured in bits per byte.
#[derive(Debug, Clone, PartialEq)]
pub struct EntropyBlock {
    /// Zero-based byte offset in the input slice.
    pub offset: u64,
    /// Exact number of source bytes represented by this block.
    pub length: u64,
    /// Shannon entropy in bits per byte, in the inclusive range `0.0..=8.0`.
    pub shannon_entropy_bits: f64,
}

/// Splits `data` into blocks and calculates their Shannon entropy.
///
/// Every returned block has a source-relative `offset` and an exact `length`.
/// The final block is included even when it is shorter than `block_size`.
/// `block_size` must be nonzero.
///
/// # Errors
///
/// Returns [`DomainError::InvalidTransform`] when `block_size` is zero, and
/// [`DomainError::RangeOverflow`] if a platform-sized offset cannot be
/// represented as a `u64` source offset.
pub fn block_shannon_entropy(
    data: &[u8],
    block_size: usize,
) -> Result<Vec<EntropyBlock>, DomainError> {
    if block_size == 0 {
        return Err(DomainError::InvalidTransform(
            "POC entropy block size must be nonzero".to_owned(),
        ));
    }

    let mut blocks = Vec::with_capacity(data.len().div_ceil(block_size));
    let mut offset = 0_usize;
    for block in data.chunks(block_size) {
        let length = block.len();
        let source_offset = u64::try_from(offset).map_err(|_| DomainError::RangeOverflow)?;
        let source_length = u64::try_from(length).map_err(|_| DomainError::RangeOverflow)?;
        blocks.push(EntropyBlock {
            offset: source_offset,
            length: source_length,
            shannon_entropy_bits: shannon_entropy_bits(block),
        });
        offset = offset
            .checked_add(length)
            .ok_or(DomainError::RangeOverflow)?;
    }
    Ok(blocks)
}

#[allow(clippy::cast_precision_loss)]
pub(super) fn shannon_entropy_bits(block: &[u8]) -> f64 {
    if block.is_empty() {
        return 0.0;
    }

    let histogram = byte_histogram(block);
    let length = block.len() as f64;
    histogram
        .bins
        .iter()
        .filter(|&&count| count != 0)
        .map(|&count| {
            let probability = count as f64 / length;
            -probability * probability.log2()
        })
        .sum()
}
