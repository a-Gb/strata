//! Acceptance checks for deterministic inputs used by programmable-video examples.

use strata_core::DomainError;
use strata_test_support::{
    poc_fixtures::{composite_firmware, interleaved_sensor_image, investigation_binary},
    projection_fixtures::projection_golden_fixtures,
};

const COMPOSITE: &[u8] = include_bytes!("../../../fixtures/video/composite-firmware-v1.bin");
const INVESTIGATION: &[u8] = include_bytes!("../../../fixtures/video/investigation-xor-v1.bin");
const INTERLEAVED: &[u8] = include_bytes!("../../../fixtures/video/interleaved-sensor-v1.bin");
const BITPLANE: &[u8] = include_bytes!("../../../fixtures/video/bitplane-image-v1.bin");

#[test]
fn checked_video_inputs_match_their_semantic_generators() -> Result<(), DomainError> {
    assert_eq!(COMPOSITE, composite_firmware()?.bytes);
    assert_eq!(INVESTIGATION, investigation_binary()?.bytes);
    assert_eq!(INTERLEAVED, interleaved_sensor_image()?.bytes);

    let bitplane = projection_golden_fixtures()
        .into_iter()
        .find(|fixture| fixture.id == "projection-planar-image-v1");
    assert!(bitplane.is_some());
    if let Some(bitplane) = bitplane {
        assert_eq!(BITPLANE, bitplane.bytes);
    }
    Ok(())
}
