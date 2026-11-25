// A mesh drawn with pictures placed by a part, a 3D model: its mesh is in
// its own space and one matrix puts it where the part stands, relative to
// the camera, as model.wgsl places cubes. Each run is drawn with its
// material: a picture, the colour of its corners, the alpha under which a
// pixel is not drawn, and how much it shines with its own colour, as a
// picture does.

// As model.wgsl's.
struct Instance {
    relative_from_model: mat4x4<f32>,
    normal_from_model: mat4x4<f32>,
}

@group(1) @binding(0) var<uniform> instance: Instance;

struct Material {
    // x: the alpha under which a pixel is not drawn. y: how much it shines
    // with its own colour: 0 lit, 1 a screen, more a lamp.
    params: vec4<f32>,
}

// The picture, its own or the one its draw names, bound as a picture is.
@group(2) @binding(0) var picture: texture_2d<f32>;
@group(2) @binding(1) var picture_sampler: sampler;
@group(3) @binding(0) var<uniform> material: Material;

struct Vertex {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    // rgb: sRGB albedo. a: alpha.
    @location(3) color: vec4<f32>,
}

struct Varying {
    @builtin(position) clip: vec4<f32>,
    @location(0) relative: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) color: vec4<f32>,
}

@vertex
fn vs(in: Vertex) -> Varying {
    var out: Varying;
    out.relative = (instance.relative_from_model * vec4<f32>(in.position, 1.0)).xyz;
    out.clip = view.clip_from_relative * vec4<f32>(out.relative, 1.0);
    out.normal = normalize((instance.normal_from_model * vec4<f32>(in.normal, 0.0)).xyz);
    out.uv = in.uv;
    out.color = in.color;
    return out;
}

@fragment
fn fs(in: Varying) -> @location(0) vec4<f32> {
    let texel = textureSample(picture, picture_sampler, in.uv);
    if texel.a * in.color.a < material.params.x {
        discard;
    }
    let albedo = texel.rgb * pow(in.color.rgb, vec3<f32>(2.2));
    let shine = material.params.y;
    let lit = lit(albedo, normalize(in.normal), 0.0, in.relative);
    let glow = own_light(albedo * max(shine, 1.0), in.relative);
    return vec4<f32>(mix(lit, glow, clamp(shine, 0.0, 1.0)), 1.0);
}

// The same cutout as the picture: a leaf's gaps cast no solid card.
@fragment
fn fs_shadow(in: Varying) {
    if textureSample(picture, picture_sampler, in.uv).a * in.color.a < material.params.x {
        discard;
    }
}
