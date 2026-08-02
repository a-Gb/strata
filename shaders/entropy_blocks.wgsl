struct Params {
    byte_len: u32,
    block_bytes: u32,
    block_count: u32,
    normalization_mode: u32,
};

@group(0) @binding(0) var<storage, read> source_words: array<u32>;
@group(0) @binding(1) var<storage, read_write> block_histograms: array<atomic<u32>>;
@group(0) @binding(2) var<storage, read_write> block_entropy: array<f32>;
@group(0) @binding(3) var<uniform> params: Params;

@compute @workgroup_size(256)
fn histogram(@builtin(global_invocation_id) gid: vec3<u32>) {
    // TODO: bounded byte extraction and partitioned/block-local histogram accumulation.
    _ = gid;
}

@compute @workgroup_size(256)
fn reduce_entropy(@builtin(global_invocation_id) gid: vec3<u32>) {
    // TODO: convert integer counts into exact declared entropy normalization.
    _ = gid;
}
