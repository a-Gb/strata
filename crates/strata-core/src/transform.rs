//! Reproducible transform graph specifications.

use crate::TransformNodeId;

#[derive(Debug, Clone, PartialEq, Eq)]
/// Semantic shape of values accepted or produced by a transform.
pub enum DataDomain {
    /// Uninterpreted eight-bit source values.
    Bytes,
    /// Individual binary values.
    Bits,
    /// Unsigned integer words of a declared width.
    UnsignedWords {
        /// Width of each word in bits.
        width_bits: u16,
    },
    /// Signed integer words of a declared width.
    SignedWords {
        /// Width of each word in bits.
        width_bits: u16,
    },
    /// Floating-point values of a declared width.
    Floats {
        /// Width of each floating-point value in bits.
        width_bits: u16,
    },
    /// Text decoded with a named encoding.
    Text {
        /// Stable encoding name.
        encoding: String,
    },
    /// Parsed records governed by a versioned schema.
    Records {
        /// Stable identifier of the record schema.
        schema_id: String,
    },
    /// One scalar value at each coordinate.
    ScalarField,
    /// One vector value at each coordinate.
    VectorField,
    /// Application-defined domain identified by name.
    Custom(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Reproducibility guarantee supplied by a transform implementation.
pub enum Determinism {
    /// Identical inputs and parameters produce byte-identical outputs.
    Deterministic,
    /// Outputs may differ only within a declared numeric tolerance.
    NumericTolerance,
    /// Outputs are reproducible when the recorded seed is reused.
    Seeded,
    /// Outputs are investigative leads and may depend on heuristic choices.
    Heuristic,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Versioned declaration of one node in a reproducible transform graph.
pub struct TransformNodeSpec {
    /// Stable node identity within the graph.
    pub id: TransformNodeId,
    /// Transform kind understood by its implementation.
    pub kind: String,
    /// Semantic domain accepted by the transform.
    pub input_domain: DataDomain,
    /// Semantic domain produced by the transform.
    pub output_domain: DataDomain,
    /// Canonical JSON parameters controlling the transform.
    pub parameter_json: String,
    /// Reproducibility guarantee of the implementation.
    pub determinism: Determinism,
    /// Whether the transform provides a defined inverse.
    pub reversible: bool,
    /// Canonical inverse specification when reversal is supported.
    pub inverse_spec_json: Option<String>,
    /// Description of information loss, if any.
    pub loss_model: Option<String>,
    /// Stable implementation and semantics identifier used for provenance.
    pub implementation_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
/// Directed acyclic specification of reproducible transform operations.
pub struct TransformGraphSpec {
    /// Transform nodes available in the graph.
    pub nodes: Vec<TransformNodeSpec>,
    /// Directed edges represented as `(input, output)` node identities.
    pub edges: Vec<(TransformNodeId, TransformNodeId)>,
}
