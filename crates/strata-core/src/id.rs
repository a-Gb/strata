//! Opaque identities shared across layers.

macro_rules! id_type {
    ($name:ident, $documentation:literal) => {
        #[doc = $documentation]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $name(pub u128);
    };
}

id_type!(SourceId, "Opaque identity of one immutable source.");
id_type!(
    SelectionId,
    "Opaque identity of a view-independent selection."
);
id_type!(
    TransformNodeId,
    "Opaque identity of a transform graph node."
);
id_type!(AnalysisRequestId, "Opaque identity of an analysis request.");
id_type!(
    ArtifactId,
    "Opaque identity of a derived analysis artifact."
);
id_type!(ViewId, "Opaque identity of a view instance.");
id_type!(SessionId, "Opaque identity of an investigation session.");
id_type!(CommandId, "Opaque identity of a command or journal event.");
id_type!(
    EvidenceId,
    "Opaque identity of a provenance-bearing evidence record."
);
id_type!(PluginId, "Opaque identity of a plugin package.");
id_type!(
    ProvenanceToken,
    "Opaque identity resolving to a derivation or provenance record."
);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
/// Monotonic version of source state used to reject stale derived work.
pub struct SourceGeneration(pub u64);
