struct Params {
    byte_len: u32,
    stride: u32,
    region_count: u32,
    mode: u32,
};

@group(0) @binding(0) var<storage, read> source_words: array<u32>;
@group(0) @binding(1) var<storage, read_write> bins: array<atomic<u32>>;
@group(0) @binding(2) var<uniform> params: Params;

@compute @workgroup_size(256)
fn count_pairs(@builtin(global_invocation_id) gid: vec3<u32>) {
    let index = gid.x;
    if (index + params.stride >= params.byte_len) { return; }
    // TODO: extract byte pair, select dense/partitioned region, increment 65,536-bin index.
}
