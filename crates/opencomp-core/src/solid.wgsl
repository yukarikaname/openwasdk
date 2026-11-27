// One solid quadrilateral per draw. The corners arrive in clip space: the CPU applies the tree's
// affine transform and the device scale before writing them, so the shader only forwards them.

struct Vertex {
    @location(0) position: vec2<f32>,
    @location(1) color: vec4<f32>,
};

struct Varying {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_main(vertex: Vertex) -> Varying {
    var result: Varying;
    result.position = vec4<f32>(vertex.position, 0.0, 1.0);
    result.color = vertex.color;
    return result;
}

@fragment
fn fs_main(result: Varying) -> @location(0) vec4<f32> {
    return result.color;
}
