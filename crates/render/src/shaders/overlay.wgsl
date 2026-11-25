// A picture laid over the finished view (`scene::Overlay`): after the
// compositor, so no exposure, bloom or tone map touches it. The viewport is
// its rectangle, a whole number of target pixels a texel, so every pixel's
// centre falls inside one texel, loaded as it is whatever sampler the
// picture keeps.

@group(1) @binding(0) var laid: texture_2d<f32>;

struct Laid {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

// One triangle over the viewport.
@vertex
fn vs(@builtin(vertex_index) index: u32) -> Laid {
    let corner = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u)) * 2.0 - 1.0;
    var out: Laid;
    out.clip = vec4<f32>(corner, 0.0, 1.0);
    out.uv = vec2<f32>(corner.x, -corner.y) * 0.5 + 0.5;
    return out;
}

// The texture is sRGB, read as linear light: put on the target as `output`
// puts its own, the bytes on the screen are the picture's.
@fragment
fn fs(in: Laid) -> @location(0) vec4<f32> {
    let size = textureDimensions(laid);
    let texel = min(vec2<u32>(in.uv * vec2<f32>(size)), size - 1u);
    return on_target(textureLoad(laid, texel, 0).rgb);
}
