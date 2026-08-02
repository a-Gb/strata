//! Cross-client golden proving the CLI publishes the shared artifact digest.

use std::{
    error::Error,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};

use serde_json::Value;
use strata_analysis::production::{StructureEntropyPreset, build_structure_entropy_artifact};
use strata_core::{ByteRange, ByteRangeSet, SourceGeneration, SourceId};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

#[test]
fn cli_digest_matches_shared_gui_artifact() -> Result<(), Box<dyn Error>> {
    let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let base = std::env::temp_dir().join(format!(
        "strata-cli-parity-{}-{counter}",
        std::process::id()
    ));
    let source_path = base.with_extension("bin");
    let preset_path = base.with_extension("json");
    let bytes = (0_u16..4096)
        .map(|value| (value.wrapping_mul(37) & 0xff) as u8)
        .collect::<Vec<_>>();
    std::fs::write(&source_path, &bytes)?;
    std::fs::write(
        &preset_path,
        br#"{"atlas_width":32,"entropy_block_size":64}"#,
    )?;

    let output = Command::new(env!("CARGO_BIN_EXE_strata"))
        .arg("analyze")
        .arg(&source_path)
        .arg("--preset")
        .arg(&preset_path)
        .arg("--output-format")
        .arg("json")
        .output()?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).into_owned().into());
    }
    let stdout = String::from_utf8(output.stdout)?;
    let envelope: Value = serde_json::from_str(&stdout)?;
    let cli_digest = envelope
        .pointer("/result/artifact_digest")
        .and_then(Value::as_str)
        .ok_or("CLI artifact digest is missing")?;

    let range = ByteRange::new(0, u64::try_from(bytes.len())?)?;
    let expected = build_structure_entropy_artifact(
        SourceId(999),
        SourceGeneration(42),
        ByteRangeSet {
            ranges: vec![range],
        },
        StructureEntropyPreset {
            atlas_width: 32,
            entropy_block_size: 64,
        },
        &[(range, bytes)],
    )?;
    assert_eq!(cli_digest, expected.artifact_digest);
    assert!(!stdout.contains(&source_path.display().to_string()));

    std::fs::remove_file(source_path)?;
    std::fs::remove_file(preset_path)?;
    Ok(())
}
