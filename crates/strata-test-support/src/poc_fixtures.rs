//! Deterministic, synthetic fixture sources for the Strata POC.
//!
//! These fixtures deliberately contain no third-party or proprietary bytes.  Their
//! metadata records half-open source byte ranges so every POC selection can be
//! checked against known ground truth.

use strata_core::{ByteRange, DomainError};

const INVESTIGATION_XOR_KEY: u8 = 0xa7;

/// A named, semantically known part of a composite firmware-like source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FirmwareRegion {
    /// Stable label for presentation and test lookup.
    pub name: &'static str,
    /// The deliberately synthesized content class.
    pub kind: FirmwareRegionKind,
    /// Exact half-open byte range in [`CompositeFirmwareFixture::bytes`].
    pub range: ByteRange,
}

/// Content classes represented by the firmware demonstration source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FirmwareRegionKind {
    /// Repeated erased-flash padding bytes.
    Padding,
    /// Printable ASCII configuration and diagnostic text.
    Text,
    /// Little-endian fixed-width table records.
    Table,
    /// Deterministic pseudo-random bytes standing in for packed payload data.
    HighComplexity,
}

/// Composite source for the Structural Atlas and Transition Grammar examples.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompositeFirmwareFixture {
    /// Complete synthetic source bytes.
    pub bytes: Vec<u8>,
    /// Non-overlapping, exhaustive ground-truth regions.
    pub regions: Vec<FirmwareRegion>,
}

/// Builds a compact firmware-like image with four visibly distinct byte regions.
///
/// The image is 1,024 bytes and is reproducible on every supported platform.
///
/// # Errors
///
/// Returns [`DomainError::RangeOverflow`] if a platform cannot represent one of
/// the fixture's fixed byte lengths.
pub fn composite_firmware() -> Result<CompositeFirmwareFixture, DomainError> {
    let mut bytes = Vec::with_capacity(1_024);
    let mut regions = Vec::with_capacity(4);

    append_firmware_region(
        &mut bytes,
        &mut regions,
        "erased-flash-padding",
        FirmwareRegionKind::Padding,
        &[0xff; 256],
    )?;

    let text = b"STRATA POC\nboard=helios-7\nmode=diagnostic\nstatus=ready\n";
    let mut text_bytes = Vec::with_capacity(256);
    while text_bytes.len() < 256 {
        let remaining = 256_u64
            .checked_sub(u64::try_from(text_bytes.len()).map_err(|_| DomainError::RangeOverflow)?)
            .ok_or(DomainError::RangeOverflow)?;
        let chunk_len = usize::try_from(
            remaining.min(u64::try_from(text.len()).map_err(|_| DomainError::RangeOverflow)?),
        )
        .map_err(|_| DomainError::RangeOverflow)?;
        text_bytes.extend_from_slice(&text[..chunk_len]);
    }
    append_firmware_region(
        &mut bytes,
        &mut regions,
        "diagnostic-text",
        FirmwareRegionKind::Text,
        &text_bytes,
    )?;

    let mut table = Vec::with_capacity(256);
    for entry in 0_u16..32 {
        table.extend_from_slice(&entry.to_le_bytes());
        table.extend_from_slice(&(0x4000_u16 + entry.saturating_mul(16)).to_le_bytes());
        table.extend_from_slice(&(0xa500_u16 | entry).to_le_bytes());
        table.extend_from_slice(&entry.saturating_mul(3).to_le_bytes());
    }
    append_firmware_region(
        &mut bytes,
        &mut regions,
        "little-endian-vector-table",
        FirmwareRegionKind::Table,
        &table,
    )?;

    let mut noise = Vec::with_capacity(256);
    let mut state = 0x5eed_1234_7abc_def0_u64;
    for _ in 0..256 {
        state = next_state(state);
        noise.push(state.to_be_bytes()[4]);
    }
    append_firmware_region(
        &mut bytes,
        &mut regions,
        "packed-payload-like",
        FirmwareRegionKind::HighComplexity,
        &noise,
    )?;

    Ok(CompositeFirmwareFixture { bytes, regions })
}

/// Exact lane and record geometry for the interleaved sensor demonstration source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InterleavedSensorLayout {
    /// Samples in each synthetic image row.
    pub width_samples: u32,
    /// Synthetic image rows.
    pub height_rows: u32,
    /// Interleaved lanes in sample order.
    pub lanes: u8,
    /// Bytes stored for each lane sample.
    pub bytes_per_lane_sample: u8,
    /// Bytes in one complete interleaved sample record.
    pub record_stride_bytes: u32,
}

/// Interleaved fixed-width source for Record and Interleave Lab demonstrations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterleavedSensorFixture {
    /// Complete synthetic interleaved source bytes.
    pub bytes: Vec<u8>,
    /// Fixed geometry used to generate the source.
    pub layout: InterleavedSensorLayout,
    /// Exact byte range covering the whole source.
    pub source_range: ByteRange,
}

/// Builds a three-lane, 16-bit, fixed-width sensor/image-like byte source.
///
/// Each record stores red, green, and blue lane samples in little-endian order.
/// The lane values use different gradients so a correct stride/deinterleave
/// hypothesis visibly reconstructs three distinct fields.
///
/// # Errors
///
/// Returns [`DomainError::RangeOverflow`] if the fixed fixture geometry cannot
/// be represented by the current platform.
pub fn interleaved_sensor_image() -> Result<InterleavedSensorFixture, DomainError> {
    let layout = InterleavedSensorLayout {
        width_samples: 24,
        height_rows: 16,
        lanes: 3,
        bytes_per_lane_sample: 2,
        record_stride_bytes: 6,
    };
    let sample_count = u64::from(layout.width_samples)
        .checked_mul(u64::from(layout.height_rows))
        .ok_or(DomainError::RangeOverflow)?;
    let byte_len = sample_count
        .checked_mul(u64::from(layout.record_stride_bytes))
        .ok_or(DomainError::RangeOverflow)?;
    let capacity = usize::try_from(byte_len).map_err(|_| DomainError::RangeOverflow)?;
    let mut bytes = Vec::with_capacity(capacity);

    for row in 0..layout.height_rows {
        for column in 0..layout.width_samples {
            let red = u16::try_from(column.saturating_mul(0x0400))
                .map_err(|_| DomainError::RangeOverflow)?;
            let green = u16::try_from(row.saturating_mul(0x0800))
                .map_err(|_| DomainError::RangeOverflow)?;
            let blue = u16::try_from((row ^ column).saturating_mul(0x0200))
                .map_err(|_| DomainError::RangeOverflow)?;
            bytes.extend_from_slice(&red.to_le_bytes());
            bytes.extend_from_slice(&green.to_le_bytes());
            bytes.extend_from_slice(&blue.to_le_bytes());
        }
    }
    let source_range = ByteRange::new(0, byte_len)?;
    Ok(InterleavedSensorFixture {
        bytes,
        layout,
        source_range,
    })
}

/// A known content class in the discovery-oriented binary fixture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvestigationRegionKind {
    /// A recognizable fixed signature followed by printable header fields.
    SignatureHeader,
    /// Printable source identification text and fixed header padding.
    PreambleText,
    /// A regular little-endian table with 32 fixed-width records.
    FixedWidthTable,
    /// Structured data that has a corresponding obfuscated copy elsewhere.
    CorrelatedSource,
    /// Byte-for-byte XOR encoding of [`InvestigationRegionKind::CorrelatedSource`].
    XorEncodedCopy,
    /// Deterministic high-variation bytes following the structured regions.
    HighVariationPayload,
    /// A synthetic embedded object marked by the standard PNG signature.
    EmbeddedObject,
    /// A repeated 16-byte motif used to test recurrence detection.
    RepeatedBlock,
    /// Alternating reserved bytes after the meaningful content.
    ReservedPadding,
}

/// Named ground truth for one region of [`InvestigationFixture`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvestigationRegion {
    /// Stable label for presentation and test lookup.
    pub name: &'static str,
    /// Deliberately generated region class.
    pub kind: InvestigationRegionKind,
    /// Exact half-open byte range in [`InvestigationFixture::bytes`].
    pub range: ByteRange,
}

/// Exact parameters for the hidden copy relationship in [`InvestigationFixture`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct XorCopyTruth {
    /// Plain structured source bytes.
    pub source_range: ByteRange,
    /// Obfuscated bytes at matching relative offsets.
    pub encoded_range: ByteRange,
    /// The fixed XOR key: `encoded[offset] == source[offset] ^ xor_key`.
    pub xor_key: u8,
}

/// A compact source designed to exercise structure and relationship discovery.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvestigationFixture {
    /// Complete synthetic source bytes.
    pub bytes: Vec<u8>,
    /// Non-overlapping, exhaustive named source ranges.
    pub regions: Vec<InvestigationRegion>,
    /// Exact truth for the intentionally obfuscated correlated copy.
    pub xor_copy: XorCopyTruth,
}

/// Builds a safe discovery binary with visible structure and a hidden XOR copy.
///
/// The 1,536-byte source contains a recognizable signature/header, 32 eight-byte
/// table records, sixteen 16-byte structured records, an XOR-encoded copy of
/// those records, high-variation bytes, a synthetic embedded PNG-signature
/// object, a repeated motif, and a reserved tail. The relation is intentionally
/// not described in the byte stream: callers can use [`XorCopyTruth`] only as
/// test ground truth.
///
/// # Errors
///
/// Returns [`DomainError::RangeOverflow`] if a platform cannot represent a
/// fixed fixture offset or length.
pub fn investigation_binary() -> Result<InvestigationFixture, DomainError> {
    let mut bytes = Vec::with_capacity(1_536);
    let mut regions = Vec::with_capacity(7);

    let mut preamble = Vec::with_capacity(64);
    preamble.extend_from_slice(b"STRATA\0DISCOVERY\x01\nrevision=1\n");
    preamble.resize(64, 0x20);
    append_investigation_region(
        &mut bytes,
        &mut regions,
        "strata-discovery-signature-header",
        InvestigationRegionKind::SignatureHeader,
        &preamble,
    )?;

    let table = fixed_width_index_table();
    append_investigation_region(
        &mut bytes,
        &mut regions,
        "fixed-width-index-table",
        InvestigationRegionKind::FixedWidthTable,
        &table,
    )?;

    let source_records = structured_calibration_records();
    let source_range = append_investigation_region(
        &mut bytes,
        &mut regions,
        "structured-calibration-records",
        InvestigationRegionKind::CorrelatedSource,
        &source_records,
    )?;

    let encoded_records: Vec<u8> = source_records
        .iter()
        .map(|byte| byte ^ INVESTIGATION_XOR_KEY)
        .collect();
    let encoded_range = append_investigation_region(
        &mut bytes,
        &mut regions,
        "xor-obfuscated-calibration-copy",
        InvestigationRegionKind::XorEncodedCopy,
        &encoded_records,
    )?;

    let (high_variation, embedded_object) = investigation_payloads();
    append_investigation_region(
        &mut bytes,
        &mut regions,
        "high-variation-payload-like",
        InvestigationRegionKind::HighVariationPayload,
        &high_variation,
    )?;

    append_investigation_region(
        &mut bytes,
        &mut regions,
        "embedded-png-signature-object",
        InvestigationRegionKind::EmbeddedObject,
        &embedded_object,
    )?;

    let repeated = repeated_investigation_motif();
    append_investigation_region(
        &mut bytes,
        &mut regions,
        "repeated-16-byte-motif",
        InvestigationRegionKind::RepeatedBlock,
        &repeated,
    )?;

    let padding = reserved_investigation_padding();
    append_investigation_region(
        &mut bytes,
        &mut regions,
        "reserved-alternating-tail",
        InvestigationRegionKind::ReservedPadding,
        &padding,
    )?;

    Ok(InvestigationFixture {
        bytes,
        regions,
        xor_copy: XorCopyTruth {
            source_range,
            encoded_range,
            xor_key: INVESTIGATION_XOR_KEY,
        },
    })
}

fn fixed_width_index_table() -> Vec<u8> {
    let mut table = Vec::with_capacity(256);
    for entry in 0_u16..32 {
        table.extend_from_slice(&entry.to_le_bytes());
        table.extend_from_slice(&(0x6000_u16 + entry.saturating_mul(32)).to_le_bytes());
        table.extend_from_slice(&(0x9c00_u16 | entry).to_le_bytes());
        table.extend_from_slice(&entry.rotate_left(3).to_le_bytes());
    }
    table
}

fn structured_calibration_records() -> Vec<u8> {
    let mut records = Vec::with_capacity(256);
    for record in 0_u16..16 {
        records.extend_from_slice(&record.to_le_bytes());
        records.extend_from_slice(&(0x2000_u16 + record.saturating_mul(64)).to_le_bytes());
        records.extend_from_slice(&(0xa300_u16 | record).to_le_bytes());
        records.extend_from_slice(&record.rotate_left(5).to_le_bytes());
        let low_byte = record.to_le_bytes()[0];
        for field in 0_u8..8 {
            records.push(low_byte.wrapping_add(field.wrapping_mul(17)));
        }
    }
    records
}

fn investigation_payloads() -> (Vec<u8>, Vec<u8>) {
    let mut high_variation = Vec::with_capacity(192);
    let mut state = 0x8b4f_2c19_d7e6_a503_u64;
    for _ in 0..192 {
        state = next_state(state);
        high_variation.push(state.to_le_bytes()[3]);
    }
    let mut embedded_object = Vec::with_capacity(64);
    embedded_object.extend_from_slice(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]);
    for _ in 0..56 {
        state = next_state(state);
        embedded_object.push(state.to_le_bytes()[5]);
    }
    (high_variation, embedded_object)
}

fn repeated_investigation_motif() -> Vec<u8> {
    b"RECURRENCE-MOTIF".repeat(16)
}

fn reserved_investigation_padding() -> Vec<u8> {
    let mut padding = Vec::with_capacity(192);
    for index in 0_u8..96 {
        padding.push(0x00);
        padding.push(index ^ 0xff);
    }
    padding
}

/// A region classification for the aligned revision-diff POC fixture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RevisionRegionKind {
    /// Bytes are identical at the same source offsets in both revisions.
    Unchanged,
    /// Bytes differ at the same source offsets in both revisions.
    Changed,
    /// A new block occupies padding reserved at the same aligned offsets.
    InsertedIntoReservedSpace,
    /// A block is removed from one aligned location and copied into another.
    MovedLike,
}

/// A named source range and its expected aligned-diff behavior.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RevisionRegion {
    /// Stable label for presentation and test lookup.
    pub name: &'static str,
    /// Intended interpretation of this aligned source range.
    pub kind: RevisionRegionKind,
    /// Exact half-open range, identical in the before and after sources.
    pub range: ByteRange,
}

/// The meaning of exact comparison in the revision-diff POC.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExactDiffInterpretation {
    /// Both buffers use a common fixed address space and length.
    pub same_offset_alignment: bool,
    /// Insertions overwrite reserved padding; no offset reflow is implied.
    pub inserted_into_reserved_space: bool,
    /// A move-like result is an inference from paired aligned changed spans.
    pub moved_blocks_are_inferred: bool,
}

/// Semantic category for one exact comparison claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComparisonTruthKind {
    /// The same bytes occur at the same offsets in both revisions.
    Unchanged,
    /// Corresponding byte ranges differ at the same offsets.
    Modified,
    /// Bytes have no semantic predecessor and occupy reserved after-space.
    NewlyIntroduced,
    /// The same byte sequence is present at distinct source and destination ranges.
    Moved,
}

/// Exact pre/post ranges for a comparison result.
///
/// `before_range` is absent only for semantically new material; the underlying
/// fixture still uses a fixed aligned address space, where the after range was
/// reserved padding before introduction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComparisonTruth {
    /// Stable label for presentation and test lookup.
    pub name: &'static str,
    /// Exact semantic relationship between the source ranges.
    pub kind: ComparisonTruthKind,
    /// Exact range in the earlier source, when a semantic predecessor exists.
    pub before_range: Option<ByteRange>,
    /// Exact range in the later source, when a semantic successor exists.
    pub after_range: Option<ByteRange>,
}

/// Paired fixed-address revisions for the Revision Diff POC view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RevisionPairFixture {
    /// Earlier synthetic revision.
    pub before: Vec<u8>,
    /// Later synthetic revision at the same logical addresses.
    pub after: Vec<u8>,
    /// Ground-truth aligned ranges for display and verification.
    pub regions: Vec<RevisionRegion>,
    /// Explicit constraints for interpreting this pair as an exact diff.
    pub interpretation: ExactDiffInterpretation,
    /// Exact pre/post relation truth for all comparison categories in the POC.
    pub exact_truth: Vec<ComparisonTruth>,
}

/// Builds two equal-length revisions with unchanged, changed, inserted, and move-like spans.
///
/// `before` and `after` are exactly aligned.  The insertion is represented by
/// replacing reserved `0xff` bytes, and the move-like event is represented by a
/// removed source span plus an identical copied destination span.  Therefore an
/// exact byte diff can prove changed offsets; identifying the move remains a
/// higher-level inference rather than a claimed relocation record.
///
/// # Errors
///
/// Returns [`DomainError::RangeOverflow`] if an internal fixed offset cannot be
/// represented on the current platform.
pub fn aligned_revision_pair() -> Result<RevisionPairFixture, DomainError> {
    let mut before = vec![0xff; 512];
    let mut after = before.clone();
    let stable = b"STRATA-REVISION-BASELINE";
    copy_at(&mut before, 0, stable)?;
    copy_at(&mut after, 0, stable)?;

    let changed_before = b"build=100; feature=off;";
    let changed_after = b"build=101; feature=on ;";
    copy_at(&mut before, 64, changed_before)?;
    copy_at(&mut after, 64, changed_after)?;

    let inserted = b"CAPS:atlas,digram,record,diff";
    copy_at(&mut after, 128, inserted)?;

    let moved = b"MOVED-BLOCK:calibration-v1";
    copy_at(&mut before, 192, moved)?;
    copy_at(&mut after, 256, moved)?;

    let regions = vec![
        RevisionRegion {
            name: "unchanged-baseline",
            kind: RevisionRegionKind::Unchanged,
            range: ByteRange::new(0, 32)?,
        },
        RevisionRegion {
            name: "aligned-configuration-change",
            kind: RevisionRegionKind::Changed,
            range: ByteRange::new(64, 87)?,
        },
        RevisionRegion {
            name: "inserted-capabilities-in-reserved-space",
            kind: RevisionRegionKind::InsertedIntoReservedSpace,
            range: ByteRange::new(128, 157)?,
        },
        RevisionRegion {
            name: "moved-like-source",
            kind: RevisionRegionKind::MovedLike,
            range: ByteRange::new(192, 218)?,
        },
        RevisionRegion {
            name: "moved-like-destination",
            kind: RevisionRegionKind::MovedLike,
            range: ByteRange::new(256, 282)?,
        },
    ];
    let exact_truth = vec![
        ComparisonTruth {
            name: "unchanged-baseline",
            kind: ComparisonTruthKind::Unchanged,
            before_range: Some(ByteRange::new(0, 32)?),
            after_range: Some(ByteRange::new(0, 32)?),
        },
        ComparisonTruth {
            name: "modified-configuration",
            kind: ComparisonTruthKind::Modified,
            before_range: Some(ByteRange::new(64, 87)?),
            after_range: Some(ByteRange::new(64, 87)?),
        },
        ComparisonTruth {
            name: "newly-introduced-capabilities",
            kind: ComparisonTruthKind::NewlyIntroduced,
            before_range: None,
            after_range: Some(ByteRange::new(128, 157)?),
        },
        ComparisonTruth {
            name: "moved-calibration-block",
            kind: ComparisonTruthKind::Moved,
            before_range: Some(ByteRange::new(192, 218)?),
            after_range: Some(ByteRange::new(256, 282)?),
        },
    ];
    Ok(RevisionPairFixture {
        before,
        after,
        regions,
        interpretation: ExactDiffInterpretation {
            same_offset_alignment: true,
            inserted_into_reserved_space: true,
            moved_blocks_are_inferred: true,
        },
        exact_truth,
    })
}

fn append_investigation_region(
    bytes: &mut Vec<u8>,
    regions: &mut Vec<InvestigationRegion>,
    name: &'static str,
    kind: InvestigationRegionKind,
    contents: &[u8],
) -> Result<ByteRange, DomainError> {
    let start = u64::try_from(bytes.len()).map_err(|_| DomainError::RangeOverflow)?;
    let contents_len = u64::try_from(contents.len()).map_err(|_| DomainError::RangeOverflow)?;
    let end = start
        .checked_add(contents_len)
        .ok_or(DomainError::RangeOverflow)?;
    let range = ByteRange::new(start, end)?;
    bytes.extend_from_slice(contents);
    regions.push(InvestigationRegion { name, kind, range });
    Ok(range)
}

fn append_firmware_region(
    bytes: &mut Vec<u8>,
    regions: &mut Vec<FirmwareRegion>,
    name: &'static str,
    kind: FirmwareRegionKind,
    contents: &[u8],
) -> Result<(), DomainError> {
    let start = u64::try_from(bytes.len()).map_err(|_| DomainError::RangeOverflow)?;
    let contents_len = u64::try_from(contents.len()).map_err(|_| DomainError::RangeOverflow)?;
    let end = start
        .checked_add(contents_len)
        .ok_or(DomainError::RangeOverflow)?;
    bytes.extend_from_slice(contents);
    regions.push(FirmwareRegion {
        name,
        kind,
        range: ByteRange::new(start, end)?,
    });
    Ok(())
}

fn copy_at(target: &mut [u8], offset: usize, contents: &[u8]) -> Result<(), DomainError> {
    let end = offset
        .checked_add(contents.len())
        .ok_or(DomainError::RangeOverflow)?;
    let start_u64 = u64::try_from(offset).map_err(|_| DomainError::RangeOverflow)?;
    let end_u64 = u64::try_from(end).map_err(|_| DomainError::RangeOverflow)?;
    let destination = target
        .get_mut(offset..end)
        .ok_or(DomainError::InvalidRange {
            start: start_u64,
            end: end_u64,
        })?;
    destination.copy_from_slice(contents);
    Ok(())
}

const fn next_state(state: u64) -> u64 {
    state
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn composite_firmware_is_reproducible_and_has_known_regions() -> Result<(), DomainError> {
        let first = composite_firmware()?;
        let second = composite_firmware()?;
        assert_eq!(first, second);
        assert_eq!(first.bytes.len(), 1_024);
        assert_eq!(first.regions.len(), 4);
        let padding = first
            .regions
            .first()
            .ok_or_else(|| DomainError::Internal("missing padding region".to_owned()))?;
        assert_eq!(padding.range, ByteRange::new(0, 256)?);
        assert_eq!(first.regions[1].kind, FirmwareRegionKind::Text);
        assert!(first.bytes[256..512].starts_with(b"STRATA POC\n"));
        assert_eq!(first.regions[3].kind, FirmwareRegionKind::HighComplexity);
        Ok(())
    }

    #[test]
    fn sensor_fixture_has_fixed_interleaved_markers() -> Result<(), DomainError> {
        let fixture = interleaved_sensor_image()?;
        assert_eq!(fixture.bytes.len(), 2_304);
        assert_eq!(fixture.layout.record_stride_bytes, 6);
        assert_eq!(fixture.source_range, ByteRange::new(0, 2_304)?);
        assert_eq!(&fixture.bytes[0..6], &[0, 0, 0, 0, 0, 0]);
        assert_eq!(&fixture.bytes[6..12], &[0, 4, 0, 0, 0, 2]);
        let second_row = usize::try_from(fixture.layout.width_samples)
            .map_err(|_| DomainError::RangeOverflow)?
            .checked_mul(6)
            .ok_or(DomainError::RangeOverflow)?;
        assert_eq!(
            &fixture.bytes[second_row..second_row + 6],
            &[0, 0, 0, 8, 0, 2]
        );
        Ok(())
    }

    #[test]
    fn investigation_binary_exposes_structure_and_exact_xor_copy_truth() -> Result<(), DomainError>
    {
        let first = investigation_binary()?;
        let second = investigation_binary()?;
        assert_eq!(first, second);
        assert_eq!(first.bytes.len(), 1_536);
        assert_eq!(first.regions.len(), 8);
        assert_eq!(first.regions[0].range, ByteRange::new(0, 64)?);
        assert_eq!(first.regions[1].range, ByteRange::new(64, 320)?);
        assert_eq!(
            first.regions[0].kind,
            InvestigationRegionKind::SignatureHeader
        );
        assert_eq!(
            first.regions[2].kind,
            InvestigationRegionKind::CorrelatedSource
        );
        assert_eq!(
            first.regions[3].kind,
            InvestigationRegionKind::XorEncodedCopy
        );
        assert_eq!(first.xor_copy.source_range, ByteRange::new(320, 576)?);
        assert_eq!(first.xor_copy.encoded_range, ByteRange::new(576, 832)?);
        assert_eq!(&first.bytes[0..6], b"STRATA");
        assert_eq!(first.bytes[6], 0);
        assert_eq!(&first.bytes[64..72], &[0, 0, 0, 0x60, 0, 0x9c, 0, 0]);
        assert_eq!(first.regions[4].range, ByteRange::new(832, 1_024)?);
        assert_eq!(
            first.regions[5].kind,
            InvestigationRegionKind::EmbeddedObject
        );
        assert_eq!(first.regions[5].range, ByteRange::new(1_024, 1_088)?);
        assert_eq!(
            &first.bytes[1_024..1_032],
            &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]
        );
        assert_eq!(
            first.regions[6].kind,
            InvestigationRegionKind::RepeatedBlock
        );
        assert_eq!(&first.bytes[1_088..1_104], b"RECURRENCE-MOTIF");
        assert_eq!(&first.bytes[1_104..1_120], b"RECURRENCE-MOTIF");

        let mut distinct_high_variation_bytes = [false; 256];
        for byte in &first.bytes[832..1_024] {
            distinct_high_variation_bytes[usize::from(*byte)] = true;
        }
        assert!(
            distinct_high_variation_bytes
                .iter()
                .filter(|seen| **seen)
                .count()
                > 128
        );

        let source = slice_for_range(&first.bytes, first.xor_copy.source_range)?;
        let encoded = slice_for_range(&first.bytes, first.xor_copy.encoded_range)?;
        assert_eq!(source.len(), encoded.len());
        assert!(
            source
                .iter()
                .zip(encoded)
                .all(|(plain, obfuscated)| *plain ^ first.xor_copy.xor_key == *obfuscated)
        );
        Ok(())
    }

    #[test]
    fn revision_pair_records_exact_alignment_and_move_evidence() -> Result<(), DomainError> {
        let fixture = aligned_revision_pair()?;
        assert_eq!(fixture.before.len(), fixture.after.len());
        assert!(fixture.interpretation.same_offset_alignment);
        assert!(fixture.interpretation.inserted_into_reserved_space);
        assert!(fixture.interpretation.moved_blocks_are_inferred);
        assert_eq!(fixture.exact_truth.len(), 4);
        assert_eq!(
            fixture.exact_truth,
            vec![
                ComparisonTruth {
                    name: "unchanged-baseline",
                    kind: ComparisonTruthKind::Unchanged,
                    before_range: Some(ByteRange::new(0, 32)?),
                    after_range: Some(ByteRange::new(0, 32)?),
                },
                ComparisonTruth {
                    name: "modified-configuration",
                    kind: ComparisonTruthKind::Modified,
                    before_range: Some(ByteRange::new(64, 87)?),
                    after_range: Some(ByteRange::new(64, 87)?),
                },
                ComparisonTruth {
                    name: "newly-introduced-capabilities",
                    kind: ComparisonTruthKind::NewlyIntroduced,
                    before_range: None,
                    after_range: Some(ByteRange::new(128, 157)?),
                },
                ComparisonTruth {
                    name: "moved-calibration-block",
                    kind: ComparisonTruthKind::Moved,
                    before_range: Some(ByteRange::new(192, 218)?),
                    after_range: Some(ByteRange::new(256, 282)?),
                },
            ]
        );
        assert_eq!(&fixture.before[128..157], &[0xff; 29]);
        assert_eq!(&fixture.after[128..157], b"CAPS:atlas,digram,record,diff");
        assert_eq!(&fixture.before[192..218], b"MOVED-BLOCK:calibration-v1");
        assert_eq!(&fixture.after[256..282], b"MOVED-BLOCK:calibration-v1");
        assert_eq!(&fixture.after[192..218], &[0xff; 26]);
        Ok(())
    }

    fn slice_for_range(bytes: &[u8], range: ByteRange) -> Result<&[u8], DomainError> {
        let start = usize::try_from(range.start).map_err(|_| DomainError::RangeOverflow)?;
        let end = usize::try_from(range.end).map_err(|_| DomainError::RangeOverflow)?;
        bytes.get(start..end).ok_or(DomainError::InvalidRange {
            start: range.start,
            end: range.end,
        })
    }
}
