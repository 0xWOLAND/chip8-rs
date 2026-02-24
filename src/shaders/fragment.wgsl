struct VertexOut {
    @builtin(position) position: vec4f,
    @location(0) uv: vec2f,
};

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> VertexOut {
    var positions = array<vec2f, 3>(
        vec2f(-1.0, -3.0),
        vec2f(-1.0, 1.0),
        vec2f(3.0, 1.0),
    );
    var out: VertexOut;
    let p = positions[index];
    out.position = vec4f(p, 0.0, 1.0);
    out.uv = p * vec2f(0.5, -0.5) + vec2f(0.5, 0.5);
    return out;
}

@group(0) @binding(0) var<storage, read> display: array<u32, 2048>;

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4f {
    let uv = clamp(in.uv, vec2f(0.0, 0.0), vec2f(0.999999, 0.999999));
    let x = u32(uv.x * 64.0);
    let y = u32(uv.y * 32.0);
    let on = select(0.0, 1.0, display[y * 64u + x] != 0u);
    return vec4f(on, on, on, 1.0);
}
