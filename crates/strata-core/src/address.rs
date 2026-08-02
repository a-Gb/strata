//! Coordinate and address-space descriptions.

use crate::ByteRange;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
/// Stable identifier for one coordinate system used to address source or derived data.
pub struct AddressSpaceId(pub String);

#[derive(Debug, Clone, PartialEq, Eq)]
/// Semantic kind of coordinate system represented by an [`AddressSpaceId`].
pub enum AddressSpaceKind {
    /// Zero-based byte offsets in the immutable source.
    FileOffset,
    /// Addresses interpreted in a process or executable virtual address space.
    VirtualAddress,
    /// Physical device or memory addresses.
    PhysicalAddress,
    /// Offsets relative to the beginning of a parsed segment.
    SegmentRelative,
    /// Monotonic positions in a stream rather than a seekable file.
    StreamSequence,
    /// Indices into values produced by a deterministic transform.
    DerivedValueIndex,
    /// Application-defined address semantics identified by name.
    Custom(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Number of source and destination addresses that a mapping may relate.
pub enum MappingCardinality {
    /// Each source address maps to exactly one destination address.
    OneToOne,
    /// One source address may contribute to several destination addresses.
    OneToMany,
    /// Several source addresses may contribute to one destination address.
    ManyToOne,
    /// Several source addresses may relate to several destination addresses.
    ManyToMany,
    /// The mapping is estimated and may not retain exact correspondence.
    Approximate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Declarative contract for converting between two address spaces.
pub struct AddressMappingSpec {
    /// Address space accepted by the mapping.
    pub from: AddressSpaceId,
    /// Address space produced by the mapping.
    pub to: AddressSpaceId,
    /// Exact input range over which the mapping is defined.
    pub valid_domain: ByteRange,
    /// Relationship shape between input and output addresses.
    pub cardinality: MappingCardinality,
    /// Whether an inverse mapping is implemented for the valid domain.
    pub invertible: bool,
    /// Stable implementation and semantics identifier used for provenance.
    pub implementation_id: String,
}
