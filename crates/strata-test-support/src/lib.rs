//! Deterministic fixture generators and cross-backend test contracts.
#![forbid(unsafe_code)]

use strata_core::{ByteRange, DomainError};

/// Deterministic, license-safe sources used by the interactive POC examples.
pub mod poc_fixtures;
/// Deterministic golden sources for projection semantics and visual regression.
pub mod projection_fixtures;

#[derive(Debug, Clone, PartialEq, Eq)]
/// Reproducible byte patterns shared by unit and differential tests.
pub enum SyntheticPattern {
    /// Fill the requested length with one byte value.
    Constant(u8),
    /// Alternate two byte values, starting with the first.
    Alternating(u8, u8),
    /// Repeat the complete ascending byte ramp `0x00..=0xff`.
    Ramp,
    /// Encode a wrapping integer counter at a fixed word width.
    Counter {
        /// Counter width in bytes.
        width_bytes: u8,
        /// Whether words use little-endian byte order.
        little_endian: bool,
    },
    /// Emit deterministic records with a fixed byte length.
    PeriodicRecord {
        /// Width of each generated record.
        record_len: u32,
        /// Seed controlling reproducible record contents.
        seed: u64,
    },
    /// Emit a deterministic pseudo-random byte stream.
    DeterministicNoise {
        /// Seed controlling the reproducible stream.
        seed: u64,
    },
    /// Join a low-complexity prefix to deterministic high-variation bytes.
    EntropyBoundary {
        /// Offset of the intended complexity transition.
        split: u64,
        /// Seed controlling the high-variation suffix.
        seed: u64,
    },
    /// Interleave a deterministic set of logical channels sample by sample.
    InterleavedChannels {
        /// Number of interleaved channels.
        channels: u8,
        /// Number of samples emitted for each channel.
        samples: u64,
    },
}

/// Builds license-safe fixture bytes from a declared deterministic pattern.
pub trait FixtureFactory {
    /// Materializes exactly `length` bytes for `pattern`.
    ///
    /// # Errors
    ///
    /// Returns an error when the requested dimensions overflow or violate the
    /// selected pattern's bounded domain.
    fn build(&self, pattern: SyntheticPattern, length: u64) -> Result<Vec<u8>, DomainError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// One analyzer input and expected exactness contract for CPU/GPU comparison.
pub struct DifferentialCase {
    /// Stable human-readable case name.
    pub name: String,
    /// Exact source range supplied to both backends.
    pub source_range: ByteRange,
    /// Analyzer implementation exercised by the case.
    pub analyzer_id: String,
    /// Canonical analyzer parameter JSON.
    pub parameter_json: String,
    /// Whether the two backends must agree bit-for-bit.
    pub expected_exact: bool,
}
