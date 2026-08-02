//! Deterministic animation programs and an offline 3D projection video renderer.
//!
//! The facade separates the versioned schema, curated presets, rasterization,
//! and encoding while retaining the original parent-module API.
#![allow(clippy::redundant_pub_crate)] // Parent-only helpers live in binary modules.

mod export;
mod presets;
mod program;
mod render;

#[cfg(test)]
mod tests;

pub(crate) use export::{export_animation, load_animation_program, save_animation_program};
pub(crate) use presets::{animation_preset, animation_presets};
pub(crate) use program::{
    AnimationEasing, AnimationKeyframe, AnimationLook, AnimationPrimitive, AnimationProgram,
    VideoExportReport,
};
