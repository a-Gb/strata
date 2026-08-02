//! Versioned animation schema, validation, and deterministic timeline evaluation.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::projection::ProjectionComposition;

pub(super) const PROGRAM_VERSION: u32 = 1;
const MAX_VIDEO_PIXELS: u64 = 12_000_000;
const MAX_VIDEO_FRAMES: u32 = 3_600;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AnimationEasing {
    Linear,
    SmoothStep,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AnimationPalette {
    #[default]
    Source,
    Cividis,
    CyanAmber,
    Monochrome,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AnimationPrimitive {
    #[default]
    Disc,
    Voxel,
}

/// Display transform only; projection positions and source evidence remain unchanged.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct AnimationLook {
    pub(crate) palette: AnimationPalette,
    pub(crate) primitive: AnimationPrimitive,
    pub(crate) contrast: f32,
    pub(crate) saturation: f32,
    pub(crate) vignette: f32,
    pub(crate) guide_opacity: f32,
}

impl Default for AnimationLook {
    fn default() -> Self {
        Self {
            palette: AnimationPalette::Source,
            primitive: AnimationPrimitive::Disc,
            contrast: 1.0,
            saturation: 1.0,
            vignette: 0.0,
            guide_opacity: 1.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AnimationSourceRange {
    pub(crate) start: u64,
    pub(crate) end: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AnimationKeyframe {
    pub(crate) at: f32,
    pub(crate) morph: f32,
    pub(crate) yaw: f32,
    pub(crate) pitch: f32,
    pub(crate) zoom: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) focus_offset: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AnimationProgram {
    pub(crate) version: u32,
    pub(crate) source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) source_range: Option<AnimationSourceRange>,
    pub(crate) output: String,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) fps: u32,
    pub(crate) duration_seconds: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) composition: Option<ProjectionComposition>,
    pub(crate) stride: usize,
    pub(crate) point_budget: usize,
    pub(crate) point_size: f32,
    pub(crate) brightness: f32,
    pub(crate) perspective: f32,
    pub(crate) background: [u8; 3],
    pub(crate) show_guides: bool,
    #[serde(default)]
    pub(crate) look: AnimationLook,
    pub(crate) easing: AnimationEasing,
    pub(crate) overwrite: bool,
    pub(crate) keyframes: Vec<AnimationKeyframe>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct VideoExportReport {
    pub(crate) output: PathBuf,
    pub(crate) manifest: PathBuf,
    pub(crate) frame_count: u32,
    pub(crate) source_sha256: String,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct AnimationState {
    pub(super) morph: f32,
    pub(super) yaw: f32,
    pub(super) pitch: f32,
    pub(super) zoom: f32,
    pub(super) focus_offset: Option<f64>,
}

impl AnimationProgram {
    pub(crate) fn example() -> Self {
        super::presets::default_program()
    }

    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_precision_loss,
        clippy::cast_sign_loss,
        clippy::too_many_lines
    )]
    pub(crate) fn validate(&self) -> Result<u32, String> {
        if self.version != PROGRAM_VERSION {
            return Err(format!(
                "animation program version {} is unsupported; expected {PROGRAM_VERSION}",
                self.version
            ));
        }
        if self.source.trim().is_empty() {
            return Err("animation source cannot be empty".to_owned());
        }
        if self
            .source_range
            .is_some_and(|range| range.start >= range.end)
        {
            return Err("animation source range must have start < end".to_owned());
        }
        let output = Path::new(self.output.trim());
        if output.as_os_str().is_empty()
            || output
                .extension()
                .is_none_or(|extension| !extension.eq_ignore_ascii_case("mp4"))
        {
            return Err("animation output must be an .mp4 path".to_owned());
        }
        if !(64..=4_096).contains(&self.width)
            || !(64..=4_096).contains(&self.height)
            || self.width % 2 != 0
            || self.height % 2 != 0
        {
            return Err("video dimensions must be even values from 64 through 4096".to_owned());
        }
        if u64::from(self.width) * u64::from(self.height) > MAX_VIDEO_PIXELS {
            return Err(format!(
                "video dimensions exceed the {MAX_VIDEO_PIXELS}-pixel safety limit"
            ));
        }
        if !(1..=120).contains(&self.fps) {
            return Err("video fps must be from 1 through 120".to_owned());
        }
        if !self.duration_seconds.is_finite() || !(0.1..=120.0).contains(&self.duration_seconds) {
            return Err("video duration must be from 0.1 through 120 seconds".to_owned());
        }
        if !(1..=1_024).contains(&self.stride) {
            return Err("projection stride must be from 1 through 1024".to_owned());
        }
        if let Some(composition) = self.composition {
            composition.validate().map_err(str::to_owned)?;
        }
        if !(1..=1_000_000).contains(&self.point_budget) {
            return Err("point budget must be from 1 through 1000000".to_owned());
        }
        if !self.point_size.is_finite() || !(0.25..=16.0).contains(&self.point_size) {
            return Err("point size must be from 0.25 through 16".to_owned());
        }
        if !self.brightness.is_finite() || !(0.1..=4.0).contains(&self.brightness) {
            return Err("brightness must be from 0.1 through 4".to_owned());
        }
        if !self.perspective.is_finite() || !(0.0..=1.0).contains(&self.perspective) {
            return Err("perspective must be from 0 through 1".to_owned());
        }
        let look = self.look;
        if !look.contrast.is_finite()
            || !(0.5..=2.0).contains(&look.contrast)
            || !look.saturation.is_finite()
            || !(0.0..=1.5).contains(&look.saturation)
            || !look.vignette.is_finite()
            || !(0.0..=1.0).contains(&look.vignette)
            || !look.guide_opacity.is_finite()
            || !(0.0..=1.0).contains(&look.guide_opacity)
        {
            return Err("animation look controls are outside their bounded domains".to_owned());
        }
        if !(2..=128).contains(&self.keyframes.len()) {
            return Err("animation programs need 2 through 128 keyframes".to_owned());
        }
        let Some(first) = self.keyframes.first() else {
            return Err("animation program has no first keyframe".to_owned());
        };
        let Some(last) = self.keyframes.last() else {
            return Err("animation program has no last keyframe".to_owned());
        };
        if first.at.abs() > f32::EPSILON || (last.at - 1.0).abs() > f32::EPSILON {
            return Err("keyframes must begin at 0 and end at 1".to_owned());
        }
        for keyframe in &self.keyframes {
            validate_keyframe(*keyframe)?;
        }
        let has_focus = first.focus_offset.is_some();
        if self
            .keyframes
            .iter()
            .any(|keyframe| keyframe.focus_offset.is_some() != has_focus)
        {
            return Err(
                "focus_offset must be present on every keyframe or omitted from every keyframe"
                    .to_owned(),
            );
        }
        if let Some(range) = self.source_range
            && self.keyframes.iter().any(|keyframe| {
                keyframe
                    .focus_offset
                    .is_some_and(|offset| offset < range.start || offset >= range.end)
            })
        {
            return Err("keyframe focus_offset must fall inside source_range".to_owned());
        }
        if self
            .keyframes
            .windows(2)
            .any(|pair| pair[0].at >= pair[1].at)
        {
            return Err("keyframe positions must be strictly increasing".to_owned());
        }

        let frame_count = (self.duration_seconds * self.fps as f32).round().max(2.0) as u32;
        if frame_count > MAX_VIDEO_FRAMES {
            return Err(format!(
                "animation requires {frame_count} frames; limit is {MAX_VIDEO_FRAMES}"
            ));
        }
        Ok(frame_count)
    }

    #[allow(clippy::cast_precision_loss)]
    pub(super) fn state_for_frame(
        &self,
        frame: u32,
        frame_count: u32,
    ) -> Result<AnimationState, String> {
        if frame_count < 2 || frame >= frame_count {
            return Err("animation frame index is outside the program".to_owned());
        }
        let progress = frame as f32 / frame_count.saturating_sub(1) as f32;
        self.state_at(progress)
    }

    pub(super) fn state_at(&self, progress: f32) -> Result<AnimationState, String> {
        let progress = progress.clamp(0.0, 1.0);
        let Some(pair) = self
            .keyframes
            .windows(2)
            .find(|pair| progress >= pair[0].at && progress <= pair[1].at)
        else {
            return Err("animation progress does not map to a keyframe segment".to_owned());
        };
        let span = pair[1].at - pair[0].at;
        if span <= f32::EPSILON {
            return Err("animation keyframe segment has zero duration".to_owned());
        }
        let amount = (progress - pair[0].at) / span;
        let amount = match self.easing {
            AnimationEasing::Linear => amount,
            AnimationEasing::SmoothStep => smooth_step(amount),
        };
        Ok(interpolate_state(pair[0], pair[1], amount))
    }
}

fn validate_keyframe(keyframe: AnimationKeyframe) -> Result<(), String> {
    if !keyframe.at.is_finite() || !(0.0..=1.0).contains(&keyframe.at) {
        return Err("keyframe position must be from 0 through 1".to_owned());
    }
    if !keyframe.morph.is_finite() || !(0.0..=3.0).contains(&keyframe.morph) {
        return Err("keyframe morph must be from 0 through 3".to_owned());
    }
    if !keyframe.yaw.is_finite() {
        return Err("keyframe yaw must be finite".to_owned());
    }
    if !keyframe.pitch.is_finite() || !(-1.48..=1.48).contains(&keyframe.pitch) {
        return Err("keyframe pitch must be from -1.48 through 1.48".to_owned());
    }
    if !keyframe.zoom.is_finite() || !(0.2..=5.0).contains(&keyframe.zoom) {
        return Err("keyframe zoom must be from 0.2 through 5".to_owned());
    }
    Ok(())
}

#[allow(clippy::cast_precision_loss)] // Sources are capped at 64 MiB, below f64's integer limit.
fn interpolate_state(
    first: AnimationKeyframe,
    second: AnimationKeyframe,
    amount: f32,
) -> AnimationState {
    AnimationState {
        morph: lerp(first.morph, second.morph, amount),
        yaw: lerp(first.yaw, second.yaw, amount),
        pitch: lerp(first.pitch, second.pitch, amount),
        zoom: lerp(first.zoom, second.zoom, amount),
        focus_offset: match (first.focus_offset, second.focus_offset) {
            (Some(first), Some(second)) => {
                Some(lerp_f64(first as f64, second as f64, f64::from(amount)))
            }
            _ => None,
        },
    }
}

fn smooth_step(value: f32) -> f32 {
    let value = value.clamp(0.0, 1.0);
    value * value * 2.0f32.mul_add(-value, 3.0)
}

fn lerp(first: f32, second: f32, amount: f32) -> f32 {
    (second - first).mul_add(amount, first)
}

fn lerp_f64(first: f64, second: f64, amount: f64) -> f64 {
    (second - first).mul_add(amount, first)
}
