// One quadrilateral per draw. The corners arrive in clip space: the CPU applies the tree's
// affine transform and the device scale before writing them, so the shader only forwards them.
//
// Every draw samples a texture and multiplies it by the vertex colour. A flat colour binds the
// built-in 1x1 white texture, whose texel is 1.0 in linear light, so the product is the colour
// itself: the textured and untextured cases share this one pipeline.

struct Vertex {
    @location(0) position: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) uv: vec2<f32>,
};

struct Varying {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) uv: vec2<f32>,
};

@group(0) @binding(0) var source: texture_2d<f32>;
@group(0) @binding(1) var source_sampler: sampler;

@vertex
fn vs_main(vertex: Vertex) -> Varying {
    var result: Varying;
    result.position = vec4<f32>(vertex.position, 0.0, 1.0);
    result.color = vertex.color;
    result.uv = vertex.uv;
    return result;
}

@fragment
fn fs_main(result: Varying) -> @location(0) vec4<f32> {
    // The bitmap is Rgba8UnormSrgb, so the sample is already linear, as is the vertex colour
    // the CPU decoded. The sRGB target encodes the product on the way out.
    return textureSample(source, source_sampler, result.uv) * result.color;
}
