//! Headless client for the same bounded source and analysis runtime as the GUI.
#![forbid(unsafe_code)]

use std::{
    env, fmt,
    path::{Path, PathBuf},
    process::ExitCode,
    time::Duration,
};

use serde::Serialize;
use strata_analysis::production::{
    ProductionRuntimeConfig, STRUCTURE_ENTROPY_SEMANTICS, StructureEntropyPreset,
};
use strata_core::{
    AnalysisRequestId, ByteRange, ByteRangeSet, DomainError, Priority, SourceGeneration, SourceId,
};
use strata_runtime::{AttachedSource, InvestigationRuntime, RuntimeStructureRequest};
use strata_source::DigestState;

const REQUEST_ID: AnalysisRequestId = AnalysisRequestId(1);
const HASH_CHUNK_BYTES: u64 = 4 * 1024 * 1024;
const ANALYSIS_TIMEOUT: Duration = Duration::from_secs(120);

fn main() -> ExitCode {
    let arguments = env::args().skip(1).collect::<Vec<_>>();
    match run(&arguments) {
        Ok(output) => {
            println!("{output}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{}", error.message);
            ExitCode::from(error.code)
        }
    }
}

fn run(arguments: &[String]) -> Result<String, CliError> {
    if arguments
        .iter()
        .any(|argument| argument == "-h" || argument == "--help")
    {
        return Ok(help_text().to_owned());
    }
    let command = parse_arguments(arguments)?;
    let result = analyze(&command)?;
    serde_json::to_string_pretty(&result)
        .map_err(|error| CliError::internal(format!("cannot encode result: {error}")))
}

#[derive(Debug)]
struct AnalyzeCommand {
    source_path: PathBuf,
    preset_path: PathBuf,
    range: Option<ByteRange>,
}

fn parse_arguments(arguments: &[String]) -> Result<AnalyzeCommand, CliError> {
    if arguments.first().map(String::as_str) != Some("analyze") {
        return Err(CliError::usage(help_text()));
    }
    let source_path = arguments
        .get(1)
        .filter(|argument| !argument.starts_with('-'))
        .map(PathBuf::from)
        .ok_or_else(|| CliError::usage("analyze requires a source path"))?;
    let mut preset_path = None;
    let mut range = None;
    let mut index = 2_usize;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--preset" => {
                let value = arguments
                    .get(index.saturating_add(1))
                    .ok_or_else(|| CliError::usage("--preset requires a JSON path"))?;
                preset_path = Some(PathBuf::from(value));
                index = index.saturating_add(2);
            }
            "--range" => {
                let value = arguments
                    .get(index.saturating_add(1))
                    .ok_or_else(|| CliError::usage("--range requires START:END"))?;
                range = Some(parse_range(value)?);
                index = index.saturating_add(2);
            }
            "--output-format" => {
                let value = arguments
                    .get(index.saturating_add(1))
                    .ok_or_else(|| CliError::usage("--output-format requires json"))?;
                if value != "json" {
                    return Err(CliError::usage("only --output-format json is supported"));
                }
                index = index.saturating_add(2);
            }
            unknown => {
                return Err(CliError::usage(format!("unknown argument: {unknown}")));
            }
        }
    }
    Ok(AnalyzeCommand {
        source_path,
        preset_path: preset_path.ok_or_else(|| CliError::usage("--preset is required"))?,
        range,
    })
}

fn parse_range(value: &str) -> Result<ByteRange, CliError> {
    let (start, end) = value
        .split_once(':')
        .ok_or_else(|| CliError::usage("range must be START:END"))?;
    ByteRange::new(parse_offset(start)?, parse_offset(end)?)
        .map_err(|error| CliError::usage(format!("invalid range: {error}")))
}

fn parse_offset(value: &str) -> Result<u64, CliError> {
    value.strip_prefix("0x").map_or_else(
        || {
            value
                .parse::<u64>()
                .map_err(|error| CliError::usage(format!("invalid decimal offset: {error}")))
        },
        |hex| {
            u64::from_str_radix(hex, 16)
                .map_err(|error| CliError::usage(format!("invalid hexadecimal offset: {error}")))
        },
    )
}

fn analyze(command: &AnalyzeCommand) -> Result<CliEnvelope, CliError> {
    let preset = read_preset(&command.preset_path)?;
    preset
        .validate()
        .map_err(|error| CliError::usage(format!("invalid preset: {error}")))?;
    let source = AttachedSource::open_local(&command.source_path, SourceId(1), SourceGeneration(0))
        .map_err(|error| CliError::source(&command.source_path, "open", &error))?;
    let descriptor = source.descriptor();
    let source_length = descriptor
        .length
        .ok_or_else(|| CliError::source_message("source length is unknown"))?;
    let range = command.range.unwrap_or(ByteRange {
        start: 0,
        end: source_length,
    });
    if range.end > source_length {
        return Err(CliError::usage(format!(
            "range ends at 0x{:x}, beyond source length 0x{source_length:x}",
            range.end
        )));
    }
    let source_digest = seal_source_digest(&source)?;
    let runtime = InvestigationRuntime::new(ProductionRuntimeConfig::default())
        .map_err(|error| CliError::domain(&error))?;
    let outcome = runtime
        .analyze_structure_blocking(
            RuntimeStructureRequest {
                request_id: REQUEST_ID,
                source,
                ranges: ByteRangeSet {
                    ranges: vec![range],
                },
                preset,
                priority: Priority::ExportCritical,
            },
            ANALYSIS_TIMEOUT,
        )
        .map_err(|error| CliError::domain(&error))?;
    let artifact = outcome.artifact;

    Ok(CliEnvelope {
        schema_version: "0.1.0",
        request_id: format!("{:032x}", REQUEST_ID.0),
        status: "complete",
        result: CliResult {
            analyzer: STRUCTURE_ENTROPY_SEMANTICS,
            artifact_digest: artifact.artifact_digest.clone(),
            source_digest: source_digest.clone(),
            source_length,
            generation: artifact.generation.0,
            covered_ranges: artifact
                .covered_ranges
                .ranges
                .iter()
                .map(|range| CliRange {
                    start: range.start,
                    end: range.end,
                })
                .collect(),
            preset,
            classified_bytes: artifact
                .classified_ranges
                .iter()
                .map(|classified| classified.classes.len())
                .sum(),
            entropy_blocks: artifact.entropy_blocks.len(),
            exactness: "exact",
            completeness: "complete",
        },
        warnings: Vec::new(),
        provenance_roots: vec![source_digest],
        metrics: CliMetrics {
            bytes_read: range.len(),
            cache: if outcome.cache_hit { "hit" } else { "miss" },
        },
    })
}

fn read_preset(path: &Path) -> Result<StructureEntropyPreset, CliError> {
    let json = std::fs::read_to_string(path).map_err(|error| {
        CliError::usage(format!("cannot read preset {}: {error}", path.display()))
    })?;
    serde_json::from_str(&json)
        .map_err(|error| CliError::usage(format!("invalid preset {}: {error}", path.display())))
}

fn seal_source_digest(source: &AttachedSource) -> Result<String, CliError> {
    loop {
        let progress = source
            .advance_digest(HASH_CHUNK_BYTES)
            .map_err(|error| CliError::domain(&error))?;
        if progress.state == DigestState::Sealed {
            return progress
                .content_digest
                .ok_or_else(|| CliError::internal("sealed source digest is missing"));
        }
    }
}

#[derive(Debug, Serialize)]
struct CliEnvelope {
    schema_version: &'static str,
    request_id: String,
    status: &'static str,
    result: CliResult,
    warnings: Vec<String>,
    provenance_roots: Vec<String>,
    metrics: CliMetrics,
}

#[derive(Debug, Serialize)]
struct CliResult {
    analyzer: &'static str,
    artifact_digest: String,
    source_digest: String,
    source_length: u64,
    generation: u64,
    covered_ranges: Vec<CliRange>,
    preset: StructureEntropyPreset,
    classified_bytes: usize,
    entropy_blocks: usize,
    exactness: &'static str,
    completeness: &'static str,
}

#[derive(Debug, Serialize)]
struct CliRange {
    start: u64,
    end: u64,
}

#[derive(Debug, Serialize)]
struct CliMetrics {
    bytes_read: u64,
    cache: &'static str,
}

#[derive(Debug)]
struct CliError {
    code: u8,
    message: String,
}

impl CliError {
    fn usage(message: impl Into<String>) -> Self {
        Self {
            code: 2,
            message: message.into(),
        }
    }

    fn source(path: &Path, operation: &str, error: &std::io::Error) -> Self {
        Self {
            code: 3,
            message: format!("cannot {operation} source {}: {error}", path.display()),
        }
    }

    fn source_message(message: impl Into<String>) -> Self {
        Self {
            code: 3,
            message: message.into(),
        }
    }

    fn domain(error: &DomainError) -> Self {
        let code = match error {
            DomainError::SourceMismatch | DomainError::StaleGeneration => 4,
            DomainError::ResourceLimit(_) | DomainError::Cancelled => 6,
            DomainError::InvalidRange { .. }
            | DomainError::RangeOverflow
            | DomainError::UnsupportedCapability(_)
            | DomainError::InvalidTransform(_)
            | DomainError::InvalidView(_) => 2,
            DomainError::Internal(_) => 9,
        };
        Self {
            code,
            message: error.to_string(),
        }
    }

    fn internal(message: impl Into<String>) -> Self {
        Self {
            code: 9,
            message: message.into(),
        }
    }
}

impl fmt::Display for CliError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

const fn help_text() -> &'static str {
    "Strata CLI\n\n  strata analyze SOURCE --preset PRESET.json [--range START:END] [--output-format json]"
}
