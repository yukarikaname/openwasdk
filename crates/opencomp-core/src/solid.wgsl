// One quadrilateral per draw. The corners arrive in clip space: the CPU applies the tree's
// affine transform and the device scale before writing them, so the shader only forwards them.
//
// Every draw samples a texture and multiplies it by the vertex colour. A flat colour binds the
// built-in 1x1 white texture, whose texel is 1.0 in linear light, so the product is the colour
// itself: the textured and untextured cases share this one pipeline.
//
// A vertex also carries its place in the node's own coordinates, half the node's size and the
// four corner radii. Interpolating the local position across the transformed quad inverts the
// affine transform for free, so the fragment stage can measure the rounded outline in local
// units and the radii follow any rotation or scale.

struct Vertex {
    // Where the vertex sits on the target, clip space already applied.
    @location(0) position: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) uv: vec2<f32>,
    // The same vertex in the node's own coordinates.
    @location(3) local: vec2<f32>,
    // Half the node's size, equal on every vertex.
    @location(4) half_size: vec2<f32>,
    // Top-left, top-right, bottom-right, bottom-left, in local units.
    @location(5) radius: vec4<f32>,
    // -1 for a flat paint, 0 for a linear ramp, 1 for a radial one.
    @location(6) gradient_kind: f32,
    // The ramp's start or centre, in local units.
    @location(7) gradient_a: vec2<f32>,
    // The ramp's end or radius, in local units.
    @location(8) gradient_b: vec2<f32>,
    // -1 for a rectangle or rounded rectangle, 0 for an ellipse, 1 for a line.
    @location(9) shape_kind: f32,
    // The ellipse centre or the line's start, in local units.
    @location(10) shape_a: vec2<f32>,
    // The ellipse radii or the line's end, in local units.
    @location(11) shape_b: vec2<f32>,
    // The line's stroke width, in local units; zero elsewhere.
    @location(12) shape_width: f32,
};

struct Varying {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) local: vec2<f32>,
    // Constant across the quad, so interpolation would only add rounding noise.
    @location(3) @interpolate(flat) half_size: vec2<f32>,
    @location(4) @interpolate(flat) radius: vec4<f32>,
    @location(5) @interpolate(flat) gradient_kind: f32,
    @location(6) @interpolate(flat) gradient_a: vec2<f32>,
    @location(7) @interpolate(flat) gradient_b: vec2<f32>,
    @location(8) @interpolate(flat) shape_kind: f32,
    @location(9) @interpolate(flat) shape_a: vec2<f32>,
    @location(10) @interpolate(flat) shape_b: vec2<f32>,
    @location(11) @interpolate(flat) shape_width: f32,
};

@group(0) @binding(0) var source: texture_2d<f32>;
@group(0) @binding(1) var source_sampler: sampler;

@vertex
fn vs_main(vertex: Vertex) -> Varying {
    var result: Varying;
    result.position = vec4<f32>(vertex.position, 0.0, 1.0);
    result.color = vertex.color;
    result.uv = vertex.uv;
    result.local = vertex.local;
    result.half_size = vertex.half_size;
    result.radius = vertex.radius;
    result.gradient_kind = vertex.gradient_kind;
    result.gradient_a = vertex.gradient_a;
    result.gradient_b = vertex.gradient_b;
    result.shape_kind = vertex.shape_kind;
    result.shape_a = vertex.shape_a;
    result.shape_b = vertex.shape_b;
    result.shape_width = vertex.shape_width;
    return result;
}

// The signed distance from `p` to a rounded box centred on the origin, in local units, negative
// inside. Each quadrant takes its own corner radius; the CPU has clamped every radius to half the
// shorter side, so the outline never folds.
fn rounded_box(p: vec2<f32>, half_size: vec2<f32>, radius: vec4<f32>) -> f32 {
    // Local y grows downwards, so the top row is the negative side.
    var r: f32;
    if (p.x < 0.0) {
        r = select(radius.x, radius.w, p.y >= 0.0);
    } else {
        r = select(radius.y, radius.z, p.y >= 0.0);
    }
    let q = abs(p) - (half_size - vec2<f32>(r));
    return length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - r;
}

// The signed distance from `p` to an axis-aligned ellipse. The first-order approximation is
// exact on the outline, which is the only place the coverage band looks.
fn ellipse(p: vec2<f32>, center: vec2<f32>, radius: vec2<f32>) -> f32 {
    let r = max(radius, vec2<f32>(0.0001, 0.0001));
    let d = p - center;
    let k0 = length(d / r);
    let k1 = length(d / (r * r));
    return k0 * (k0 - 1.0) / max(k1, 0.0001);
}

// The signed distance from `p` to a segment stroked `width` wide, with flat ends.
fn segment(p: vec2<f32>, start: vec2<f32>, end: vec2<f32>, width: f32) -> f32 {
    let span = end - start;
    let h = clamp(dot(p - start, span) / max(dot(span, span), 0.0001), 0.0, 1.0);
    return length(p - start - span * h) - width * 0.5;
}

@fragment
fn fs_main(v: Varying) -> @location(0) vec4<f32> {
    // A gradient draw binds the 256-entry ramp instead of a bitmap. The ramp maps t to texel
    // centre `t * 255 + 0.5`, so the ends sit under the first and last entries. The condition
    // is flat across the quad, and the single sample stays outside it so derivatives are
    // uniform.
    var uv = v.uv;
    if (v.gradient_kind > 0.5) {
        // Radial: the ellipse is an axis-aligned one in local units; t is its normalised
        // distance. A zero radius divides by a small epsilon and clamps to the last stop.
        let radius = max(v.gradient_b, vec2<f32>(0.0001, 0.0001));
        let t = clamp(length((v.local - v.gradient_a) / radius), 0.0, 1.0);
        uv = vec2<f32>(t * 255.0 / 256.0 + 0.5 / 256.0, 0.5);
    } else if (v.gradient_kind > -0.5) {
        // Linear: the projection of the local point onto the start-to-end span.
        let span = v.gradient_b - v.gradient_a;
        let squared = dot(span, span);
        var t = 1.0;
        if (squared > 0.0) {
            t = clamp(dot(v.local - v.gradient_a, span) / squared, 0.0, 1.0);
        }
        uv = vec2<f32>(t * 255.0 / 256.0 + 0.5 / 256.0, 0.5);
    }

    // The ramp is Rgba8UnormSrgb, so the sample is already linear, as is the vertex colour
    // the CPU decoded. Sample before any discard: implicit derivatives need uniform control
    // flow, and a square corner still samples exactly as it did before rounding existed.
    let texel = textureSample(source, source_sampler, uv);
    let color = texel * v.color;

    // A square keeps the hard edge this pipeline always had; the distance would otherwise
    // soften the four corners a square does not have. A shape always measures its own outline;
    // otherwise only a rounded rectangle needs one.
    var d = 0.0;
    if (v.shape_kind > 0.5) {
        d = segment(v.local, v.shape_a, v.shape_b, v.shape_width);
    } else if (v.shape_kind > -0.5) {
        d = ellipse(v.local, v.shape_a, v.shape_b);
    } else if (any(v.radius != vec4<f32>(0.0))) {
        d = rounded_box(v.local - v.half_size, v.half_size, v.radius);
    } else {
        return color;
    }

    // The distance moves by `fwidth(d)` per device pixel, so half of that either side of the
    // outline is one pixel of coverage. No extension is needed: fwidth is core WGSL.
    let band = max(0.5 * fwidth(d), 0.0001);
    let coverage = 1.0 - smoothstep(-band, band, d);
    if (coverage <= 0.0) {
        discard;
    }

    return vec4<f32>(color.rgb, color.a * coverage);
}
