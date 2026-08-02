struct Params {
    byte_len: u32,
    selected_plane: u32,
    packing_mode: u32,
    reserved: u32,
};

@group(0) @binding(0) var<storage, read> source_words: array<u32>;
@group(0) @binding(1) var<storage, read_write> output_words: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;

@compute @workgroup_size(256)
fn extract_plane(@builtin(global_invocation_id) gid: vec3<u32>) {
    // TODO: extract selected bit plane with explicit source-to-output mapping.
    _ = gid;
}
