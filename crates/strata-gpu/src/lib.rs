//! Backend-neutral GPU contracts and the verified WGPU P1 coordinate backend.
#![forbid(unsafe_code)]

use std::{sync::mpsc, time::Duration};

use strata_analysis::{AnalysisEnvelope, AnalysisRequest};
use strata_core::{DomainError, Priority};
use wgpu::util::DeviceExt;

const P1_SHADER: &str = include_str!("../../../shaders/p1_projection.wgsl");
const GPU_OUTPUT_BYTES: u64 = 32;
const MAX_GPU_DATUMS: usize = 1_000_000;

#[derive(Debug, Clone, PartialEq, Eq)]
/// Stable, serializable description of the GPU selected by the host.
pub struct GpuDeviceDescriptor {
    /// Native graphics backend name.
    pub backend: String,
    /// Adapter name reported by WGPU.
    pub adapter_name: String,
    /// Enabled device feature names.
    pub feature_names: Vec<String>,
    /// Canonical summary of relevant device limits.
    pub limit_json: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Host-enforced memory ceilings for analysis workloads.
pub struct GpuBudget {
    /// Maximum combined GPU-resident analysis bytes.
    pub total_bytes: u64,
    /// Maximum bytes reserved for upload staging.
    pub staging_bytes: u64,
    /// Maximum bytes reserved for result readback.
    pub readback_bytes: u64,
    /// Maximum resident bytes attributable to one plugin.
    pub per_plugin_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Versioned identity and determinism claim for one compute kernel.
pub struct KernelDescriptor {
    /// Stable kernel identifier.
    pub id: String,
    /// Version of the coordinate or analysis semantics implemented.
    pub semantics_version: String,
    /// Digest of the exact shader source or compiled module.
    pub shader_digest: String,
    /// Dispatch workgroup dimensions declared by the kernel.
    pub workgroup_size: [u32; 3],
    /// Human-readable scope of the kernel's reproducibility guarantee.
    pub deterministic_claim: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Bounded dispatch description used for scheduling and accounting.
pub struct GpuJobDescriptor {
    /// Kernel selected for the job.
    pub kernel: KernelDescriptor,
    /// Scheduler priority inherited from the analysis request.
    pub priority: Priority,
    /// Number of input bytes made resident for the dispatch.
    pub input_bytes: u64,
    /// Predicted result size used for budget admission.
    pub estimated_output_bytes: u64,
    /// Canonical JSON parameters supplied to the kernel.
    pub parameter_json: String,
}

/// Backend-neutral interface for bounded GPU analysis execution.
pub trait GpuAnalysisBackend: Send + Sync {
    /// Returns the descriptor for the currently selected device.
    fn device(&self) -> &GpuDeviceDescriptor;
    /// Returns the memory budget enforced by this backend.
    fn budget(&self) -> GpuBudget;
    /// Reports whether the backend implements a request's analyzer contract.
    fn supports(&self, request: &AnalysisRequest) -> bool;
    /// Executes one admitted request and returns provenance-bearing artifacts.
    ///
    /// # Errors
    /// Evicts or compacts resident resources until the declared budget holds.
    ///
    /// Returns an error when request validation, dispatch, or result decoding fails.
    fn execute(&self, request: AnalysisRequest) -> Result<Vec<AnalysisEnvelope>, DomainError>;
    /// Recreates the selected device and every verified compute pipeline.
    ///
    /// # Errors
    ///
    /// Returns an error when resident resources cannot be brought under the declared budget.
    fn trim_to_budget(&self) -> Result<(), DomainError>;
    ///
    /// # Errors
    ///
    /// Returns an error when the device and its verified pipelines cannot be restored.
    fn recover_device(&self) -> Result<(), DomainError>;
}

/// Exact source datum supplied to the P1 coordinate kernel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct P1GpuDatum {
    /// Stable absolute file offset.
    pub offset: u64,
    /// Exact primary byte value.
    pub byte: u8,
}

/// Coordinates returned by one verified GPU dispatch.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct P1GpuProjection {
    /// Alignment Lattice position.
    pub alignment: [f32; 3],
    /// Fixed-basis Hamming Hypercube position.
    pub hypercube: [f32; 3],
}

/// Observable result of the CPU/GPU differential acceptance gate.
#[derive(Debug, Clone, PartialEq)]
pub struct P1GpuSelfTest {
    /// Adapter selected by WGPU.
    pub adapter_name: String,
    /// Native backend name.
    pub backend: String,
    /// Largest component error across the fixed test corpus.
    pub maximum_component_error: f32,
    /// Number of records compared.
    pub compared_records: usize,
}

/// Compiled WGPU compute path for bounded P1 tile records.
#[derive(Debug, Clone)]
pub struct WgpuP1Backend {
    descriptor: GpuDeviceDescriptor,
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::ComputePipeline,
}

impl WgpuP1Backend {
    /// Compiles the kernel against an existing renderer device.
    ///
    /// # Errors
    ///
    /// Returns an error if the backend cannot establish the bounded P1 pipeline contract.
    pub fn from_device(
        adapter: &wgpu::Adapter,
        device: wgpu::Device,
        queue: wgpu::Queue,
    ) -> Result<Self, DomainError> {
        let info = adapter.get_info();
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("strata-p1-coordinate-kernel"),
            source: wgpu::ShaderSource::Wgsl(P1_SHADER.into()),
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("strata-p1-coordinate-pipeline"),
            layout: None,
            module: &shader,
            entry_point: Some("project_p1"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });
        Ok(Self {
            descriptor: GpuDeviceDescriptor {
                backend: format!("{:?}", info.backend),
                adapter_name: info.name,
                feature_names: Vec::new(),
                limit_json: "bounded P1 coordinate kernel".to_owned(),
            },
            device,
            queue,
            pipeline,
        })
    }

    /// Adapter details shown by the UI after the differential gate passes.
    #[must_use]
    pub const fn descriptor(&self) -> &GpuDeviceDescriptor {
        &self.descriptor
    }

    /// Dispatches alignment and hypercube coordinates and reads them back.
    ///
    /// # Errors
    ///
    /// Returns an error for unbounded inputs, checked-size overflow, device failure, timeout, or
    /// malformed GPU readback.
    #[allow(clippy::too_many_lines)] // Buffer lifetime and dispatch ordering stay locally auditable.
    pub fn project(
        &self,
        data: &[P1GpuDatum],
        source_length: u64,
        stride: usize,
    ) -> Result<Vec<P1GpuProjection>, DomainError> {
        if data.is_empty() {
            return Ok(Vec::new());
        }
        if data.len() > MAX_GPU_DATUMS || source_length == 0 || !(1..=4096).contains(&stride) {
            return Err(DomainError::ResourceLimit(
                "P1 GPU request exceeds its bounded domain".to_owned(),
            ));
        }
        let stride_u64 = u64::try_from(stride).map_err(|_| DomainError::RangeOverflow)?;
        let mut input_bytes = Vec::with_capacity(data.len().saturating_mul(16));
        for datum in data {
            let [offset_low, offset_high] = split_u64(datum.offset);
            append_u32(&mut input_bytes, offset_low);
            append_u32(&mut input_bytes, offset_high);
            append_u32(&mut input_bytes, u32::from(datum.byte));
            append_u32(
                &mut input_bytes,
                u32::try_from(datum.offset % stride_u64).map_err(|_| DomainError::RangeOverflow)?,
            );
        }
        let count = u32::try_from(data.len()).map_err(|_| DomainError::RangeOverflow)?;
        let stride_u32 = u32::try_from(stride).map_err(|_| DomainError::RangeOverflow)?;
        let mut parameter_bytes = Vec::with_capacity(16);
        let [source_length_low, source_length_high] = split_u64(source_length);
        append_u32(&mut parameter_bytes, source_length_low);
        append_u32(&mut parameter_bytes, source_length_high);
        append_u32(&mut parameter_bytes, stride_u32);
        append_u32(&mut parameter_bytes, count);

        let input = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("strata-p1-input"),
                contents: &input_bytes,
                usage: wgpu::BufferUsages::STORAGE,
            });
        let parameters = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("strata-p1-parameters"),
                contents: &parameter_bytes,
                usage: wgpu::BufferUsages::UNIFORM,
            });
        let output_size = GPU_OUTPUT_BYTES
            .checked_mul(u64::from(count))
            .ok_or(DomainError::RangeOverflow)?;
        let output = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("strata-p1-output"),
            size: output_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let readback = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("strata-p1-readback"),
            size: output_size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let layout = self.pipeline.get_bind_group_layout(0);
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("strata-p1-bind-group"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: input.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: output.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: parameters.as_entire_binding(),
                },
            ],
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("strata-p1-command-encoder"),
            });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("strata-p1-compute-pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.dispatch_workgroups(count.div_ceil(64), 1, 1);
        }
        encoder.copy_buffer_to_buffer(&output, 0, &readback, 0, output_size);
        let submission = self.queue.submit(Some(encoder.finish()));
        let (sender, receiver) = mpsc::channel();
        readback
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                let _ = sender.send(result);
            });
        self.device
            .poll(wgpu::PollType::Wait {
                submission_index: Some(submission),
                timeout: Some(Duration::from_secs(5)),
            })
            .map_err(|error| DomainError::Internal(format!("GPU poll failed: {error}")))?;
        receiver
            .recv_timeout(Duration::from_secs(5))
            .map_err(|error| DomainError::Internal(format!("GPU map callback failed: {error}")))?
            .map_err(|error| DomainError::Internal(format!("GPU readback failed: {error}")))?;
        let mapped = readback.slice(..).get_mapped_range();
        let projections = decode_outputs(&mapped, data.len())?;
        drop(mapped);
        readback.unmap();
        Ok(projections)
    }

    /// Runs the fixed CPU/GPU differential corpus on this renderer device.
    ///
    /// # Errors
    ///
    /// Returns an error when dispatch fails or any coordinate exceeds the fixed tolerance.
    pub fn verify(&self) -> Result<P1GpuSelfTest, DomainError> {
        verify_backend(self)
    }
}

/// Creates a native adapter, executes a real dispatch, and compares CPU coordinates.
///
/// # Errors
///
/// Returns an error when no native adapter/device is available, dispatch fails, or the CPU/GPU
/// differential exceeds tolerance.
pub fn run_p1_gpu_self_test() -> Result<P1GpuSelfTest, DomainError> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        force_fallback_adapter: false,
        compatible_surface: None,
    }))
    .map_err(|error| DomainError::UnsupportedCapability(format!("no WGPU adapter: {error}")))?;
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("strata-p1-self-test-device"),
        ..Default::default()
    }))
    .map_err(|error| DomainError::UnsupportedCapability(format!("WGPU device failed: {error}")))?;
    let backend = WgpuP1Backend::from_device(&adapter, device, queue)?;
    verify_backend(&backend)
}

fn verify_backend(backend: &WgpuP1Backend) -> Result<P1GpuSelfTest, DomainError> {
    let data = [
        P1GpuDatum { offset: 0, byte: 0 },
        P1GpuDatum {
            offset: 17,
            byte: 1,
        },
        P1GpuDatum {
            offset: 255,
            byte: 0x55,
        },
        P1GpuDatum {
            offset: 4095,
            byte: 0xaa,
        },
        P1GpuDatum {
            offset: 65_535,
            byte: 0xff,
        },
        P1GpuDatum {
            offset: (1_u64 << 32) + 17,
            byte: 0x81,
        },
        P1GpuDatum {
            offset: (1_u64 << 40) - 1,
            byte: 0x7e,
        },
    ];
    let source_length = 1_u64 << 40;
    let stride = 16;
    let actual = backend.project(&data, source_length, stride)?;
    let mut maximum_component_error = 0.0_f32;
    for (datum, projection) in data.iter().zip(&actual) {
        let sample = strata_core::ByteRange::new(datum.offset, datum.offset.saturating_add(1))?;
        let artifact = strata_analysis::projection_p1::analyze_p1_tile(
            &[datum.byte],
            datum.offset,
            source_length,
            &[sample],
            strata_analysis::projection_p1::P1AnalysisConfig {
                alignment_stride: stride,
                ..Default::default()
            },
            strata_analysis::projection_p1::P1FeatureRequest::default(),
            false,
        )?;
        let expected = artifact.records.first().ok_or_else(|| {
            DomainError::Internal("CPU differential reference returned no record".to_owned())
        })?;
        for (actual, expected) in projection
            .alignment
            .into_iter()
            .zip(expected.alignment)
            .chain(projection.hypercube.into_iter().zip(expected.hypercube))
        {
            maximum_component_error = maximum_component_error.max((actual - expected).abs());
        }
    }
    if maximum_component_error > 0.000_01 {
        return Err(DomainError::Internal(format!(
            "CPU/GPU differential exceeded tolerance: {maximum_component_error}"
        )));
    }
    Ok(P1GpuSelfTest {
        adapter_name: backend.descriptor.adapter_name.clone(),
        backend: backend.descriptor.backend.clone(),
        maximum_component_error,
        compared_records: data.len(),
    })
}

fn append_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

const fn split_u64(value: u64) -> [u32; 2] {
    let bytes = value.to_le_bytes();
    [
        u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
        u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]),
    ]
}

fn decode_outputs(bytes: &[u8], count: usize) -> Result<Vec<P1GpuProjection>, DomainError> {
    let expected = count
        .checked_mul(usize::try_from(GPU_OUTPUT_BYTES).map_err(|_| DomainError::RangeOverflow)?)
        .ok_or(DomainError::RangeOverflow)?;
    if bytes.len() != expected {
        return Err(DomainError::Internal(
            "GPU readback length does not match the dispatch".to_owned(),
        ));
    }
    bytes
        .chunks_exact(usize::try_from(GPU_OUTPUT_BYTES).map_err(|_| DomainError::RangeOverflow)?)
        .map(|chunk| {
            let component = |index: usize| -> Result<f32, DomainError> {
                let start = index.checked_mul(4).ok_or(DomainError::RangeOverflow)?;
                let encoded: [u8; 4] = chunk
                    .get(start..start.saturating_add(4))
                    .ok_or(DomainError::RangeOverflow)?
                    .try_into()
                    .map_err(|_| DomainError::RangeOverflow)?;
                Ok(f32::from_le_bytes(encoded))
            };
            Ok(P1GpuProjection {
                alignment: [component(0)?, component(1)?, component(2)?],
                hypercube: [component(4)?, component(5)?, component(6)?],
            })
        })
        .collect()
}
