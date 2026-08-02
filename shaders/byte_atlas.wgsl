struct Params {
    byte_len: u32,
    output_width: u32,
    layout_kind: u32,
    palette_mode: u32,
};

@group(0) @binding(0) var<storage, read> source_words: array<u32>;
@group(0) @binding(1) var<storage, read_write> classes: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;

fn extract_byte(index: u32) -> u32 {
    let word = source_words[index / 4u];
    let shift = (index % 4u) * 8u;
    return (word >> shift) & 0xffu;
}

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let index = gid.x;
    if (index >= params.byte_len) { return; }
    let value = extract_byte(index);
    // TODO: classify and map source offset to selected layout coordinate.
    classes[index] = value;
}
