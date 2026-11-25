struct Vertex {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
}

// One box: `relative_from_box` as columns, then linear colour.
struct Instance {
    @location(2) c0: vec4<f32>,
    @location(3) c1: vec4<f32>,
    @location(4) c2: vec4<f32>,
    @location(5) c3: vec4<f32>,
    @location(6) color: vec4<f32>,
}

struct Varying {
    @builtin(position) clip: vec4<f32>,
    @location(0) relative: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec3<f32>,
    @location(3) shine: f32,
}

@vertex
fn vs(in: Vertex, box: Instance) -> Varying {
    return placed(in, box);
}

// A cube's vertex where its box puts it, camera-relative.
fn placed(in: Vertex, box: Instance) -> Varying {
    let relative_from_box = mat4x4<f32>(box.c0, box.c1, box.c2, box.c3);
    var out: Varying;
    out.relative = (relative_from_box * vec4<f32>(in.position, 1.0)).xyz;
    out.clip = view.clip_from_relative * vec4<f32>(out.relative, 1.0);
    // Boxes scale along their own axes only, so dividing the rotated normal
    // by the squared axis lengths is the inverse transpose.
    let m = mat3x3<f32>(box.c0.xyz, box.c1.xyz, box.c2.xyz);
    let scale2 = vec3<f32>(dot(m[0], m[0]), dot(m[1], m[1]), dot(m[2], m[2]));
    out.normal = m * (in.normal / scale2);
    out.color = box.color.rgb;
    out.shine = box.color.a;
    return out;
}

@fragment
fn fs(in: Varying) -> @location(0) vec4<f32> {
    let color = lit(in.color, normalize(in.normal), 0.0, in.relative);
    return vec4<f32>(charged(color, in.shine), 1.0);
}

// A box that is a layer of a sheet, drawn by a pipeline of its own: a box
// as above, with its sheet's plane, its own y.
struct Layer {
    @builtin(position) clip: vec4<f32>,
    @location(0) relative: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec3<f32>,
    @location(3) shine: f32,
    @location(4) @interpolate(flat) plane: vec3<f32>,
}

// How much of a layer's own face turns its light, the rest its sheet's: the
// sheet shades as one soft mass, and the steps of its layers still show.
const FACE: f32 = 0.4;
// How much of the sun on a sheet's far side comes through to the near one,
// in the sheet's own colour.
const TRANSMIT: f32 = 0.3;

@vertex
fn vs_sheet(in: Vertex, box: Instance) -> Layer {
    let solid = placed(in, box);
    return Layer(solid.clip, solid.relative, solid.normal, solid.color, solid.shine, normalize(box.c1.xyz));
}

@fragment
fn fs_sheet(in: Layer) -> @location(0) vec4<f32> {
    let color = sheet_lit(in.color, normalize(in.normal), in.plane, in.relative);
    return vec4<f32>(charged(color, in.shine), 1.0);
}

// A layer of a sheet, lit as a solid of its colour is, by the side of the
// sheet the eye sees, with the sun on its far side showing through: a wing
// follows its coat and the light as the body it grows from does.
fn sheet_lit(mid: vec3<f32>, face: vec3<f32>, plane: vec3<f32>, relative: vec3<f32>) -> vec3<f32> {
    let sun = view.sun.xyz;
    let distance = length(relative);
    let dir = relative / distance;
    let near = plane * select(1.0, -1.0, dot(plane, dir) > 0.0);
    // A face across the plane takes the near side; an edge keeps its own.
    let own = face * select(1.0, -1.0, dot(face, near) < 0.0);
    let normal = normalize(near * (1.0 - FACE) + own * FACE);
    var color = lit_by(mid, normal, 0.0, relative, reach_at(relative, normal, near, 1.0));
    let far = reach_at(relative, -near, -near, 1.0);
    let through = max(dot(-near, sun), 0.0) * far.day * far.sun * far.cloud;
    color += mid * SUN_COLOR * through * TRANSMIT;
    return through_medium(color, dir, distance);
}
