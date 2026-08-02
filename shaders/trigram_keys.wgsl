struct Params {
    byte_len: u32,
    stride_a: u32,
    stride_b: u32,
    output_capacity: u32,
};

@group(0) @binding(0) var<storage, read> source_words: array<u32>;
@group(0) @binding(1) var<storage, read_write> packed_keys: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;

@compute @workgroup_size(256)
fn pack_keys(@builtin(global_invocation_id) gid: vec3<u32>) {
    // TODO: pack 24-bit trigram keys for later radix sort/run reduction or bounded hash aggregation.
    _ = gid;
}
