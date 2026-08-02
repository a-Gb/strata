struct Params {
    byte_len: u32,
    delta_mode: u32,
    reserved_a: u32,
    reserved_b: u32,
};

@group(0) @binding(0) var<storage, read> source_a: array<u32>;
@group(0) @binding(1) var<storage, read> source_b: array<u32>;
@group(0) @binding(2) var<storage, read_write> delta: array<u32>;
@group(0) @binding(3) var<uniform> params: Params;

@compute @workgroup_size(256)
fn compare(@builtin(global_invocation_id) gid: vec3<u32>) {
    // TODO: exact equality/XOR/delta over host-supplied aligned ranges.
    _ = gid;
}
