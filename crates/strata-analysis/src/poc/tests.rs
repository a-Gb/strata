use super::{
    ByteClass, DiscoveryConfig, DiscoveryEvidence, DiscoveryKind, MAX_DISCOVERY_BYTES,
    MIN_XOR_CORRELATED_DISTINCT_BYTES, ResonanceMetric, block_shannon_entropy, byte_histogram,
    classify_byte, digram_counts, discover_findings, selection_resonance,
};
use strata_core::DomainError;

#[test]
fn classifies_bytes_for_atlas_colouring() {
    assert_eq!(classify_byte(0), ByteClass::Zero);
    assert_eq!(classify_byte(0xff), ByteClass::AllOnes);
    assert_eq!(classify_byte(b' '), ByteClass::Whitespace);
    assert_eq!(classify_byte(b'A'), ByteClass::PrintableAscii);
    assert_eq!(classify_byte(0x1f), ByteClass::Control);
    assert_eq!(classify_byte(0x80), ByteClass::HighBit);
}

#[test]
fn counts_a_known_histogram() {
    let histogram = byte_histogram(&[0, 1, 1, 255]);
    assert_eq!(histogram.bins[0], 1);
    assert_eq!(histogram.bins[1], 2);
    assert_eq!(histogram.bins[255], 1);
    assert_eq!(histogram.bins.iter().sum::<u64>(), 4);
}

#[test]
fn distinguishes_constant_and_balanced_entropy() -> Result<(), DomainError> {
    let constant = block_shannon_entropy(&[42; 8], 8)?;
    let balanced = block_shannon_entropy(&[0, 1, 0, 1, 0, 1, 0, 1], 8)?;

    assert_eq!(constant[0].offset, 0);
    assert_eq!(constant[0].length, 8);
    assert!(constant[0].shannon_entropy_bits.abs() < f64::EPSILON);
    assert!((balanced[0].shannon_entropy_bits - 1.0).abs() < f64::EPSILON);
    Ok(())
}

#[test]
fn retains_offsets_for_a_short_final_entropy_block() -> Result<(), DomainError> {
    let blocks = block_shannon_entropy(&[0, 1, 2, 3, 4], 2)?;
    assert_eq!(blocks.len(), 3);
    assert_eq!((blocks[2].offset, blocks[2].length), (4, 1));
    Ok(())
}

#[test]
fn counts_digrams_at_the_requested_stride() -> Result<(), DomainError> {
    let counts = digram_counts(&[1, 2, 1, 2], 2)?;
    assert_eq!(counts.stride, 2);
    assert_eq!(counts.count(1, 1), 1);
    assert_eq!(counts.count(2, 2), 1);
    assert_eq!(counts.count(1, 2), 0);
    Ok(())
}

#[test]
fn rejects_zero_sized_analysis_windows() {
    assert!(matches!(
        block_shannon_entropy(&[1], 0),
        Err(DomainError::InvalidTransform(_))
    ));
    assert!(matches!(
        digram_counts(&[1], 0),
        Err(DomainError::InvalidTransform(_))
    ));
}

#[test]
fn resonance_finds_a_repeated_exact_window() -> Result<(), DomainError> {
    let scan = selection_resonance(
        &[1, 2, 3, 4, 9, 9, 1, 2, 3, 4],
        0,
        4,
        1,
        32,
        ResonanceMetric::ExactBytes,
    )?;
    let repeated = scan
        .matches
        .iter()
        .find(|candidate| candidate.offset == 6)
        .ok_or_else(|| DomainError::Internal("missing repeated window".to_owned()))?;
    assert!((repeated.score - 1.0).abs() < f64::EPSILON);
    assert_eq!(repeated.length, 4);
    Ok(())
}

#[test]
fn byte_shape_survives_value_substitution() -> Result<(), DomainError> {
    let data = [b'A', 0, b'B', 0, 0xff, 0xff, b'Z', 0, b'Y', 0];
    let shape = selection_resonance(&data, 0, 4, 1, 32, ResonanceMetric::ByteShape)?;
    let exact = selection_resonance(&data, 0, 4, 1, 32, ResonanceMetric::ExactBytes)?;
    let shape_match = shape
        .matches
        .iter()
        .find(|candidate| candidate.offset == 6)
        .ok_or_else(|| DomainError::Internal("missing shape candidate".to_owned()))?;
    let exact_match = exact
        .matches
        .iter()
        .find(|candidate| candidate.offset == 6)
        .ok_or_else(|| DomainError::Internal("missing exact candidate".to_owned()))?;
    assert!((shape_match.score - 1.0).abs() < f64::EPSILON);
    assert!(exact_match.score < shape_match.score);
    Ok(())
}

#[test]
fn resonance_sampling_is_bounded_and_retains_probe() -> Result<(), DomainError> {
    let data = vec![7_u8; 4_096];
    let scan = selection_resonance(&data, 1_337, 32, 1, 16, ResonanceMetric::Texture)?;
    assert!(scan.matches.len() <= 17);
    assert!(scan.sampled_step > 1);
    assert!(scan.matches.iter().any(|candidate| {
        candidate.offset == 1_337 && (candidate.score - 1.0).abs() < f64::EPSILON
    }));
    Ok(())
}

#[test]
fn discovery_reports_exact_repeated_window_evidence() -> Result<(), DomainError> {
    let data = [
        0xde, 0xad, 0xbe, 0xef, 0x11, 0x22, 0x33, 0x44, 0xde, 0xad, 0xbe, 0xef,
    ];
    let findings = discover_findings(
        &data,
        DiscoveryConfig {
            max_inspected_bytes: data.len(),
            repeated_window_size: 4,
            max_windows: 8,
            max_findings: 4,
            xor_minimum_confidence: 1.0,
            xor_minimum_gain: 1.0,
        },
    )?;
    let repeated = findings
        .iter()
        .find(|finding| finding.kind == DiscoveryKind::RepeatedWindow)
        .ok_or_else(|| DomainError::Internal("missing repeat finding".to_owned()))?;
    assert_eq!(repeated.source_ranges[0].start, 0);
    assert_eq!(repeated.source_ranges[0].end, 4);
    assert_eq!(repeated.source_ranges[1].start, 8);
    assert_eq!(repeated.source_ranges[1].end, 12);
    assert!((repeated.confidence - 1.0).abs() < f64::EPSILON);
    assert!(matches!(
        repeated.evidence,
        DiscoveryEvidence::RepeatedWindow {
            identical_byte_count: 4,
            ..
        }
    ));
    Ok(())
}

#[test]
fn discovery_reports_a_single_byte_xor_hypothesis() -> Result<(), DomainError> {
    let key = 0xa5_u8;
    let plain = b"strata evidence: byte windows repeat cleanly\n";
    let encoded: Vec<u8> = plain.iter().map(|byte| *byte ^ key).collect();
    let findings = discover_findings(
        &encoded,
        DiscoveryConfig {
            max_inspected_bytes: encoded.len(),
            repeated_window_size: 64,
            max_windows: 1,
            max_findings: 16,
            xor_minimum_confidence: 0.75,
            xor_minimum_gain: 0.30,
        },
    )?;
    let hypothesis = findings.iter().find(|finding| {
        matches!(
            finding.evidence,
            DiscoveryEvidence::SingleByteXor {
                key: candidate_key,
                ..
            } if candidate_key == key
        )
    });
    let hypothesis =
        hypothesis.ok_or_else(|| DomainError::Internal("missing XOR hypothesis".to_owned()))?;
    assert_eq!(hypothesis.kind, DiscoveryKind::SingleByteXor);
    assert_eq!(hypothesis.source_ranges.len(), 1);
    assert_eq!(hypothesis.source_ranges[0].end, encoded.len() as u64);
    assert!(hypothesis.confidence >= 0.75);
    Ok(())
}

#[test]
fn discovery_links_separate_xor_correlated_regions() -> Result<(), DomainError> {
    let key = 0xa5_u8;
    let primary = [
        0x10, 0x25, 0x37, 0x49, 0x5b, 0x6d, 0x7f, 0x81, 0x93, 0xa4, 0xb6, 0xc8, 0xda, 0xec, 0xfe,
        0x0f, 0x12, 0x24, 0x36, 0x48, 0x5a, 0x6c, 0x7e, 0x80, 0x92, 0xa3, 0xb5, 0xc7, 0xd9, 0xeb,
        0xfd, 0x0e,
    ];
    let mut data = vec![0_u8; 16];
    data.extend(primary);
    data.extend([0_u8; 16]);
    data.extend(primary.iter().map(|byte| *byte ^ key));

    let findings = discover_findings(
        &data,
        DiscoveryConfig {
            max_inspected_bytes: data.len(),
            repeated_window_size: 16,
            max_windows: 16,
            max_findings: 12,
            xor_minimum_confidence: 1.0,
            xor_minimum_gain: 1.0,
        },
    )?;
    let correlated = findings
        .iter()
        .find(|finding| finding.kind == DiscoveryKind::XorCorrelatedWindow)
        .ok_or_else(|| DomainError::Internal("missing XOR correlation finding".to_owned()))?;
    assert_eq!(correlated.source_ranges[0].start, 16);
    assert_eq!(correlated.source_ranges[0].end, 48);
    assert_eq!(correlated.source_ranges[1].start, 64);
    assert_eq!(correlated.source_ranges[1].end, 96);
    assert!(matches!(
        correlated.evidence,
        DiscoveryEvidence::XorCorrelatedWindow {
            key: candidate_key,
            correlated_byte_count: 32,
            distinct_byte_count,
            ..
        } if candidate_key == key && usize::from(distinct_byte_count) >= MIN_XOR_CORRELATED_DISTINCT_BYTES
    ));
    Ok(())
}

#[test]
fn xor_correlation_ignores_low_diversity_padding() -> Result<(), DomainError> {
    let data = [0_u8, 0, 0, 0, 0xff, 0xff, 0xff, 0xff];
    let findings = discover_findings(
        &data,
        DiscoveryConfig {
            max_inspected_bytes: data.len(),
            repeated_window_size: 4,
            max_windows: 4,
            max_findings: 6,
            xor_minimum_confidence: 1.0,
            xor_minimum_gain: 1.0,
        },
    )?;
    assert!(
        findings
            .iter()
            .all(|finding| finding.kind != DiscoveryKind::XorCorrelatedWindow)
    );
    Ok(())
}

#[test]
fn discovery_is_stable_and_rejects_unbounded_configuration() -> Result<(), DomainError> {
    let data = [1, 2, 3, 4, 1, 2, 3, 4];
    let config = DiscoveryConfig {
        max_inspected_bytes: data.len(),
        repeated_window_size: 4,
        max_windows: 8,
        max_findings: 4,
        xor_minimum_confidence: 1.0,
        xor_minimum_gain: 1.0,
    };
    assert_eq!(
        discover_findings(&data, config)?,
        discover_findings(&data, config)?
    );
    assert!(matches!(
        discover_findings(
            &data,
            DiscoveryConfig {
                max_inspected_bytes: MAX_DISCOVERY_BYTES + 1,
                ..config
            }
        ),
        Err(DomainError::InvalidTransform(_))
    ));
    Ok(())
}
