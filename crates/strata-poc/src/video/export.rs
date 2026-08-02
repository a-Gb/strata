//! Program I/O, bounded frame orchestration, provenance manifests, and MP4 encoding.

use std::{
    fmt::Write as _,
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{SystemTime, UNIX_EPOCH},
};

use image::{ColorType, ImageFormat};
use serde::Serialize;
use sha2::{Digest, Sha256};
use strata_analysis::projection_p1::analyze_p1_tile;
use strata_core::ByteRange;

use crate::projection::{
    ProjectionComposition, ProjectionKind, ProjectionSample, ProjectionSamplingConfig,
    sample_projection_samples_at_offset, sample_projection_samples_with_config,
};
use crate::{composition_uses_p1, p1_analysis_config, p1_feature_request, p1_point_budget};

use super::program::{AnimationProgram, VideoExportReport};
use super::render::render_frame;

#[derive(Serialize)]
struct AnimationManifest<'a> {
    schema: &'static str,
    source_sha256: &'a str,
    frame_count: u32,
    program: &'a AnimationProgram,
}

pub(crate) fn load_animation_program(path: &Path) -> Result<AnimationProgram, String> {
    let bytes = fs::read(path)
        .map_err(|error| format!("cannot read animation program {}: {error}", path.display()))?;
    let program: AnimationProgram = serde_json::from_slice(&bytes)
        .map_err(|error| format!("invalid animation program {}: {error}", path.display()))?;
    program.validate()?;
    Ok(program)
}

pub(crate) fn save_animation_program(
    path: &Path,
    program: &AnimationProgram,
    overwrite: bool,
) -> Result<(), String> {
    program.validate()?;
    let bytes = serde_json::to_vec_pretty(program)
        .map_err(|error| format!("cannot serialize animation program: {error}"))?;
    write_file(path, &bytes, overwrite)
}

#[allow(clippy::too_many_lines)]
pub(crate) fn export_animation(
    program: &AnimationProgram,
    bytes: &[u8],
) -> Result<VideoExportReport, String> {
    let frame_count = program.validate()?;
    let (projection_bytes, base_offset) = projection_source(program, bytes)?;
    let point_budget = program
        .composition
        .map_or(program.point_budget, |composition| {
            let has_bitplanes = composition.projection_a == ProjectionKind::Bitplanes
                || composition.projection_b == ProjectionKind::Bitplanes;
            let instance_budget = if has_bitplanes {
                program.point_budget.checked_div(8).unwrap_or(1).max(1)
            } else {
                program.point_budget
            };
            if composition_uses_p1(composition) {
                p1_point_budget(composition, instance_budget)
            } else {
                instance_budget
            }
        });
    let mut samples = program.composition.map_or_else(
        || {
            sample_projection_samples_at_offset(
                projection_bytes,
                base_offset,
                program.stride,
                point_budget,
            )
        },
        |composition| {
            sample_projection_samples_with_config(
                projection_bytes,
                base_offset,
                ProjectionSamplingConfig::from(composition),
                point_budget,
            )
        },
    );
    if let Some(composition) = program.composition
        && composition_uses_p1(composition)
    {
        attach_p1_video_features(&mut samples, projection_bytes, base_offset, composition)?;
    }
    if samples.is_empty() {
        return Err("source is too short for the requested projection stride".to_owned());
    }

    let output = PathBuf::from(program.output.trim());
    let manifest = manifest_path(&output)?;
    if !program.overwrite && (output.exists() || manifest.exists()) {
        return Err(format!(
            "export target already exists; enable overwrite or choose another path: {}",
            output.display()
        ));
    }
    let parent = output
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty());
    if let Some(parent) = parent {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "cannot create export directory {}: {error}",
                parent.display()
            )
        })?;
    }

    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    let frame_directory =
        std::env::temp_dir().join(format!("strata-poc-video-{}-{nonce}", std::process::id()));
    fs::create_dir(&frame_directory).map_err(|error| {
        format!(
            "cannot create temporary frame directory {}: {error}",
            frame_directory.display()
        )
    })?;

    for frame in 0..frame_count {
        let state = program.state_for_frame(frame, frame_count)?;
        let pixels = render_frame(program, &samples, state)?;
        let frame_path = frame_directory.join(format!("frame-{frame:06}.png"));
        image::save_buffer_with_format(
            &frame_path,
            &pixels,
            program.width,
            program.height,
            ColorType::Rgba8,
            ImageFormat::Png,
        )
        .map_err(|error| format!("cannot write frame {}: {error}", frame_path.display()))?;
    }

    let source_sha256 = source_sha256(bytes);
    let temporary_output = temporary_output_path(&output, nonce)?;
    encode_mp4(
        &frame_directory,
        &temporary_output,
        program.fps,
        &source_sha256,
    )?;
    fs::rename(&temporary_output, &output).map_err(|error| {
        format!(
            "cannot finalize video {} from {}: {error}",
            output.display(),
            temporary_output.display()
        )
    })?;

    let manifest_record = AnimationManifest {
        schema: "strata.animation-render/v1",
        source_sha256: &source_sha256,
        frame_count,
        program,
    };
    let manifest_bytes = serde_json::to_vec_pretty(&manifest_record)
        .map_err(|error| format!("cannot serialize animation manifest: {error}"))?;
    write_file(&manifest, &manifest_bytes, program.overwrite)?;
    let _cleanup_result = fs::remove_dir_all(&frame_directory);

    Ok(VideoExportReport {
        output,
        manifest,
        frame_count,
        source_sha256,
    })
}

pub(super) fn attach_p1_video_features(
    samples: &mut [ProjectionSample],
    bytes: &[u8],
    base_offset: usize,
    composition: ProjectionComposition,
) -> Result<(), String> {
    let base = u64::try_from(base_offset)
        .map_err(|_| "animation source offset does not fit P1 provenance".to_owned())?;
    let resident_length = u64::try_from(bytes.len())
        .map_err(|_| "animation source length does not fit P1 provenance".to_owned())?;
    let source_length = base
        .checked_add(resident_length)
        .ok_or_else(|| "animation source range overflows P1 provenance".to_owned())?;
    let ranges = samples
        .iter()
        .map(|sample| {
            let [start, end] = sample.exact_analysis_range();
            ByteRange::new(
                u64::try_from(start).map_err(|_| strata_core::DomainError::RangeOverflow)?,
                u64::try_from(end).map_err(|_| strata_core::DomainError::RangeOverflow)?,
            )
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("animation P1 ranges are invalid: {error}"))?;
    let artifact = analyze_p1_tile(
        bytes,
        base,
        source_length,
        &ranges,
        p1_analysis_config(composition.parameters),
        p1_feature_request(composition),
        false,
    )
    .map_err(|error| format!("animation P1 analysis failed: {error}"))?;
    let records = artifact
        .records
        .into_iter()
        .map(|record| (record.point_id, record))
        .collect::<std::collections::BTreeMap<_, _>>();
    for sample in samples {
        if let Some(record) = records.get(&sample.point_id).copied() {
            sample.attach_p1(record);
        }
    }
    Ok(())
}

pub(super) fn projection_source<'a>(
    program: &AnimationProgram,
    bytes: &'a [u8],
) -> Result<(&'a [u8], usize), String> {
    let source_length = u64::try_from(bytes.len()).map_or(u64::MAX, |length| length);
    if let Some(offset) = program
        .keyframes
        .iter()
        .filter_map(|keyframe| keyframe.focus_offset)
        .find(|offset| *offset >= source_length)
    {
        return Err(format!(
            "focus offset {offset} is outside the {}-byte source",
            bytes.len()
        ));
    }
    let Some(range) = program.source_range else {
        return Ok((bytes, 0));
    };
    let start = usize::try_from(range.start)
        .map_err(|_| format!("source range start {} exceeds this platform", range.start))?;
    let end = usize::try_from(range.end)
        .map_err(|_| format!("source range end {} exceeds this platform", range.end))?;
    let Some(window) = bytes.get(start..end) else {
        return Err(format!(
            "source range {}..{} is outside the {}-byte source",
            range.start,
            range.end,
            bytes.len()
        ));
    };
    Ok((window, start))
}

pub(crate) fn source_sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut output = String::with_capacity(64);
    for byte in digest {
        let _format_result = write!(output, "{byte:02x}");
    }
    output
}

fn encode_mp4(
    frame_directory: &Path,
    output: &Path,
    fps: u32,
    source_sha256: &str,
) -> Result<(), String> {
    let ffmpeg = ffmpeg_path();
    let input = frame_directory.join("frame-%06d.png");
    let status = Command::new(&ffmpeg)
        .args(["-hide_banner", "-loglevel", "error", "-y", "-framerate"])
        .arg(fps.to_string())
        .arg("-i")
        .arg(&input)
        .args([
            "-c:v",
            "libx264",
            "-preset",
            "medium",
            "-crf",
            "18",
            "-x264-params",
            "colorprim=bt709:transfer=bt709:colormatrix=bt709",
            "-pix_fmt",
            "yuv420p",
            "-colorspace",
            "bt709",
            "-color_primaries",
            "bt709",
            "-color_trc",
            "bt709",
            "-movflags",
            "+faststart",
            "-metadata",
        ])
        .arg(format!("comment=Strata source sha256 {source_sha256}"))
        .arg(output)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .map_err(|error| {
            format!(
                "cannot start FFmpeg at {}: {error}; set STRATA_FFMPEG to its executable",
                ffmpeg.display()
            )
        })?;
    if status.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&status.stderr);
    Err(format!(
        "FFmpeg failed; rendered PNG frames remain in {}: {}",
        frame_directory.display(),
        stderr.trim()
    ))
}

fn ffmpeg_path() -> PathBuf {
    if let Some(explicit) = std::env::var_os("STRATA_FFMPEG") {
        return PathBuf::from(explicit);
    }
    for candidate in ["/opt/homebrew/bin/ffmpeg", "/usr/local/bin/ffmpeg"] {
        let path = PathBuf::from(candidate);
        if path.is_file() {
            return path;
        }
    }
    PathBuf::from("ffmpeg")
}

fn manifest_path(output: &Path) -> Result<PathBuf, String> {
    let stem = output
        .file_stem()
        .and_then(|stem| stem.to_str())
        .ok_or_else(|| "video output needs a valid UTF-8 file name".to_owned())?;
    Ok(output.with_file_name(format!("{stem}.strata.json")))
}

fn temporary_output_path(output: &Path, nonce: u128) -> Result<PathBuf, String> {
    let file_name = output
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "video output needs a valid UTF-8 file name".to_owned())?;
    Ok(output.with_file_name(format!(".{file_name}.{nonce}.part.mp4")))
}

fn write_file(path: &Path, bytes: &[u8], overwrite: bool) -> Result<(), String> {
    if path.exists() && !overwrite {
        return Err(format!(
            "file already exists; enable overwrite or choose another path: {}",
            path.display()
        ));
    }
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)
            .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
    }
    fs::write(path, bytes).map_err(|error| format!("cannot write {}: {error}", path.display()))
}
