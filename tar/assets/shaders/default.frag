struct VertexOutput {
    @builtin(position) position: vec4f,
    @location(0) tex_coords: vec2f,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var result: VertexOutput;

    let x = i32(vertex_index) / 2;
    let y = i32(vertex_index) & 1;

    result.tex_coords = vec2f(
        f32(x) * 2.0,
        f32(y) * 2.0
    );
    result.position = vec4f(
        result.tex_coords.x * 2.0 - 1.0,
        1.0 - result.tex_coords.y * 2.0,
        0.0, 1.0
    );

    return result;
}

@group(0)
@binding(0)
var r_color: texture_2d<f32>;

@fragment
fn fs_main(vertex: VertexOutput) -> @location(0) vec4f {
    return vec4f(vertex.tex_coords, 0.0, 1.0);
}