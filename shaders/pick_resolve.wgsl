struct Params {
    query_x: u32,
    query_y: u32,
    width: u32,
    height: u32,
};

@group(0) @binding(0) var pick_texture: texture_2d<u32>;
@group(0) @binding(1) var<storage, read_write> result: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;

@compute @workgroup_size(1)
fn resolve() {
    // TODO: read a bounded pixel neighborhood and return candidate integer pick IDs.
}
