//! Developer utility for validating an external UFSC pack and optional source.

use std::{env, error::Error, path::PathBuf, time::Instant};

use strata_analysis::signatures::{SignatureCatalog, SignatureScanConfig};

fn main() -> Result<(), Box<dyn Error>> {
    let mut arguments = env::args_os().skip(1);
    let pack_path = arguments
        .next()
        .map(PathBuf::from)
        .ok_or("usage: inspect_signature_pack PACK.json [SOURCE]")?;
    let source_path = arguments.next().map(PathBuf::from);
    if arguments.next().is_some() {
        return Err("usage: inspect_signature_pack PACK.json [SOURCE]".into());
    }

    let pack_bytes = std::fs::read(&pack_path)?;
    let import_started = Instant::now();
    let catalog = SignatureCatalog::from_ufsc_json(&pack_bytes)?;
    let import_elapsed = import_started.elapsed();
    let stats = catalog.stats();
    println!(
        "{} {}\npack: {}\ndigest: {}\naccepted: {}\nembedded eligible: {}\nskipped: {}\nimport: {:.1?}",
        catalog.name(),
        catalog.version(),
        pack_path.display(),
        catalog.digest(),
        stats.accepted_rules,
        catalog.embedded_rule_count(),
        stats.skipped_records(),
        import_elapsed
    );

    let Some(source_path) = source_path else {
        return Ok(());
    };
    let source = std::fs::read(&source_path)?;
    let scan_started = Instant::now();
    let report = catalog.scan(&source, SignatureScanConfig::default())?;
    let scan_elapsed = scan_started.elapsed();
    println!(
        "source: {}\nbytes: {}\nmatches: {}\ntruncated: {}\nscan: {:.1?}",
        source_path.display(),
        source.len(),
        report.matches.len(),
        report.truncated,
        scan_elapsed
    );
    for matched in report.matches.iter().take(32) {
        let labels = matched
            .evidence
            .candidates
            .iter()
            .map(|candidate| candidate.label.as_str())
            .collect::<Vec<_>>()
            .join(" | ");
        println!(
            "0x{:08x}..0x{:08x}\t{:.3}\t{:?}\t{}\t{}",
            matched.source_range.start,
            matched.source_range.end,
            matched.confidence,
            matched.evidence.mode,
            matched.evidence.pattern_hex,
            labels
        );
    }
    Ok(())
}
