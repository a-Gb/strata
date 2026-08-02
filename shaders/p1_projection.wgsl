// Deterministic P1 coordinate kernel. Statistical and search-heavy projections
// retain bounded CPU references until their compute kernels pass the same gate.

struct Datum {
    offset_low: u32,
    offset_high: u32,
    byte_value: u32,
    residue: u32,
};

struct Projection {
    alignment: vec4<f32>,
    hypercube: vec4<f32>,
};

struct Parameters {
    source_length_low: u32,
    source_length_high: u32,
    stride: u32,
    count: u32,
};

@group(0) @binding(0) var<storage, read> input_data: array<Datum>;
@group(0) @binding(1) var<storage, read_write> output_data: array<Projection>;
@group(0) @binding(2) var<uniform> parameters: Parameters;

fn wide_float(low: u32, high: u32) -> f32 {
    return f32(high) * 4294967296.0 + f32(low);
}

fn normalized(value: f32, denominator: f32) -> f32 {
    return (value / max(denominator, 1.0)) * 2.0 - 1.0;
}

fn bit_sign(value: u32, bit: u32) -> f32 {
    return select(-1.0, 1.0, (value & (1u << bit)) != 0u);
}

@compute @workgroup_size(64)
fn project_p1(@builtin(global_invocation_id) invocation: vec3<u32>) {
    let index = invocation.x;
    if index >= parameters.count {
        return;
    }
    let datum = input_data[index];
    let stride = max(parameters.stride, 1u);
    let offset = wide_float(datum.offset_low, datum.offset_high);
    let source_length = wide_float(
        parameters.source_length_low,
        parameters.source_length_high,
    );
    let block = floor(offset / f32(stride));
    let block_count = max(ceil(source_length / f32(stride)), 1.0);
    let alignment = vec3<f32>(
        normalized(f32(datum.residue), f32(max(stride - 1u, 1u))),
        normalized(f32(datum.byte_value), 255.0),
        normalized(block, max(block_count - 1.0, 1.0)),
    );

    let s0 = bit_sign(datum.byte_value, 0u);
    let s1 = bit_sign(datum.byte_value, 1u);
    let s2 = bit_sign(datum.byte_value, 2u);
    let s3 = bit_sign(datum.byte_value, 3u);
    let s4 = bit_sign(datum.byte_value, 4u);
    let s5 = bit_sign(datum.byte_value, 5u);
    let s6 = bit_sign(datum.byte_value, 6u);
    let s7 = bit_sign(datum.byte_value, 7u);
    let hypercube = clamp(
        vec3<f32>(
            0.58 * s0 - 0.58 * s1 + 0.41 * s6 - 0.41 * s7,
            0.58 * s2 - 0.58 * s3 + 0.41 * s6 - 0.41 * s7,
            0.58 * s4 - 0.58 * s5 + 0.41 * s6 - 0.41 * s7,
        ) / 2.0,
        vec3<f32>(-1.0),
        vec3<f32>(1.0),
    );
    output_data[index].alignment = vec4<f32>(alignment, 1.0);
    output_data[index].hypercube = vec4<f32>(hypercube, 1.0);
}
