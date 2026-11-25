// Foliage, painted: its wood as tubes and its leaves as cards, each card
// turned to the eye and each point of it lit as that point of the round mass
// it belongs to, so light falls on the mass and never on its cards. It takes
// the light the grass takes, and three tones a step apart shade it, cool in
// the shade and warm in the sun, with no hard line between them; gusts that
// cross the world sway it; its base, sunk in the grass, takes the ground's
// light. An instance crossing from one form to the next
// draws a share of its pixels in each. `CASTING` is set before this file: in
// the sun's pass a card turns to the sun, and only the wood and a round card
// a mass are drawn there.

struct Vertex {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec4<f32>,
    @location(3) card: vec4<f32>,
    @location(4) x_axis: vec3<f32>,
    @location(5) y_axis: vec3<f32>,
    @location(6) z_axis: vec3<f32>,
    @location(7) origin: vec3<f32>,
    @location(8) seed: f32,
    @location(9) mass: vec3<f32>,
    // x: how much of it is drawn, as `drawn` reads it. y: the share of its
    // size it has grown to.
    @location(10) drawn: vec2<f32>,
}

struct Varying {
    @builtin(position) clip: vec4<f32>,
    @location(0) relative: vec3<f32>,
    @location(1) normal: vec3<f32>,
    // rgb: the middle tone, as painted. a: how open to the sky.
    @location(2) color: vec4<f32>,
    // xy: where on its card, -1..1. z: the card's own seed. w: what it is.
    @location(3) card: vec4<f32>,
    // The plane a card is turned in: its right and its up.
    @location(4) right: vec3<f32>,
    @location(5) upward: vec3<f32>,
    // x: a card's half side, metres. y: how its tone leans, about 0.
    @location(6) half_lean: vec2<f32>,
    // The middle of a card's mass, as `relative` is.
    @location(7) mass: vec3<f32>,
    // xyz: the instance's origin, as `relative` is. w: its dither.
    @location(8) placed: vec4<f32>,
    // xyz: the normal of the ground it stands on. w: how high over that
    // ground this point stands, metres.
    @location(9) ground: vec4<f32>,
}

const TAU: f32 = 6.2831853;
// What a vertex is, by `card.w`.
const WOOD: f32 = 0.0;
const LEAF: f32 = 1.0;
const MASS: f32 = 2.0;
// How far the wind sways an instance at a gust's peak, metres for each metre
// of its height, and at most SWAY_M; a leaf card flutters by FLUTTER, a share
// of its half side. Both come from scene, whose bounds of foliage hold them.
const SWAY: f32 = 0.0385;
// The most a gust reaches, the sum of its waves' heights.
const GUST_PEAK: f32 = 1.1;
// How wide the leaves of a mass's rim read, metres.
const LEAF_M: f32 = 0.45;
// How much of a leaf's normal is its crown's, the rest its mass's: the crown
// shades as one body, and its masses still round.
const CROWN: f32 = 0.35;
// What wood's shade turns to, at its own value.
const VIOLET: vec3<f32> = vec3<f32>(1.0, 0.82, 1.45);
const LUMA: vec3<f32> = vec3<f32>(0.2126, 0.7152, 0.0722);
// How much of the sky a mass's underside gathers through the leaves over it,
// against a solid's; how much of the sun behind a mass comes through it.
const THIN: f32 = 0.75;
const THROUGH: f32 = 0.6;
// How far out from its mass's middle a leaf is looked up in the sun's
// shadow, against where it is: past the round card its own mass casts,
// whose plane would cut each leaf card straight into lit and shaded. Its
// own shade is its mass's, by the ball's turn from the sun.
const SKIN: f32 = 2.2;
// A leaf card whose mass turns further from the eye than this folds away:
// the mass's near side hides it, and a crown pixel is drawn half as often.
const HIDDEN: f32 = -0.25;
// How far apart gusts come, metres; whole seconds for every period, so the
// wind runs on when the scene clock wraps.
const GUST_M: f32 = 47.0;
// How much an instance's tone leans warm or cool: a low noise over the world,
// so neighbours differ a little, and its own seed.
const LEAN_M: f32 = 90.0;
// A dither's grain at least, radians: a few pixels, so it never sparkles.
const DITHER_ANGLE: f32 = 0.003;
// An instance sits in the grass, not on it: up to this height over the ground,
// metres, about the depth of the grass's shaded roots, its base turns to
// the ground and is shut in as the roots are, and where it meets the ground
// it is lit as they are.
const BASE_M: f32 = 0.4;

fn seed_of(p: vec3<f32>) -> f32 {
    return fract(sin(dot(p, vec3<f32>(12.9898, 78.233, 37.719))) * 43758.5453);
}

@vertex
fn vs(in: Vertex) -> Varying {
    // An instance at the edge of its reach grows from its root.
    let grown = in.drawn.y;
    let axes = mat3x3<f32>(in.x_axis, in.y_axis, in.z_axis) * grown;
    let block_m = length(in.z_axis) * grown;
    let up = surface_up(in.origin);
    var centre = in.origin + axes * in.position;
    let height_m = max(in.position.z * block_m, 0.0);

    // Wind: gusts travel across the world, so neighbours sway in turn, and
    // an instance sways the more the taller it stands there. From the
    // planet's centre in f32, as `up` is: a few millimetres off at its
    // radius, nothing to a wave of GUST_M or LEAN_M, where the anchor's
    // wrap would cut each wave at its seams.
    let t = view.flags.y * TAU;
    let world = view.camera.xyz + in.origin;
    let along = normalize(cross(up, vec3<f32>(0.37, 0.81, 0.45)));
    let gust = (sin(dot(world, along) * (TAU / GUST_M) - t / 6.0) * 0.65
        + sin(t / 2.0 + in.seed * TAU) * 0.2 + 0.25) / GUST_PEAK;
    centre += along * gust * min(height_m * SWAY, SWAY_M) * smoothstep(0.0, 2.0, height_m);
    let mass = centre - axes * (in.position - in.mass);

    // The cofactors of the axes carry a normal square to the sheared block,
    // as the inverse transpose would, for three cross products.
    let facing = mat3x3<f32>(
        cross(in.y_axis, in.z_axis),
        cross(in.z_axis, in.x_axis),
        cross(in.x_axis, in.y_axis),
    );
    let normal = normalize(facing * in.normal);

    let card_seed = seed_of(in.position);
    var right = vec3<f32>(0.0);
    var upward = vec3<f32>(0.0);
    var half_m = in.card.z * block_m;
    if in.card.w > WOOD + 0.5 {
        var to_eye = -normalize(centre);
        if CASTING {
            to_eye = view.sun.xyz;
        }
        if in.card.w < LEAF + 0.5 && dot(normal, to_eye) < HIDDEN {
            half_m = 0.0;
        }
        var side = cross(up, to_eye);
        if dot(side, side) < 1e-6 {
            side = cross(in.x_axis, to_eye);
        }
        let flat_right = normalize(side);
        let flat_up = cross(to_eye, flat_right);
        // Each card spins about its middle by its own seed, so the cut rims
        // of a mass never line up.
        let spin = card_seed * TAU;
        right = flat_right * cos(spin) + flat_up * sin(spin);
        upward = cross(to_eye, right);
        let flutter = sin(t / 2.0 + card_seed * TAU) * FLUTTER * half_m;
        centre += (right * in.card.x + upward * in.card.y) * half_m + up * flutter;
    }

    var out: Varying;
    out.relative = centre;
    out.clip = view.clip_from_relative * vec4<f32>(centre, 1.0);
    out.normal = normal;
    out.color = in.color;
    out.card = vec4<f32>(in.card.xy, card_seed, in.card.w);
    out.mass = mass;
    out.right = right;
    out.upward = upward;
    let lean = sin(dot(world, vec3<f32>(0.6, 0.3, 0.74)) * (TAU / LEAN_M)) * 0.5 + (in.seed - 0.5) * 0.9;
    out.half_lean = vec2<f32>(half_m, lean);
    out.placed = vec4<f32>(in.origin, in.drawn.x);
    // Its ground is the plane its axes across span: level for a shape that
    // stands on its wood, leaning for one that stands on its leaves.
    var ground = normalize(cross(in.x_axis, in.y_axis));
    if dot(ground, up) < 0.0 {
        ground = -ground;
    }
    out.ground = vec4<f32>(ground, dot(centre - in.origin, ground));
    return out;
}

// Whether a pixel of an instance is drawn: all of it at a dither of 1; at
// `d` in 0..1, the share `d` of its pixels; at `-d`, the rest. Two forms of
// an instance crossing draw each pixel once. The pattern is its own,
// leaf-sized and at least a few pixels, laid on the plane through its root
// that faces the eye, where the eye's ray meets it: one for every form, and
// it stays on the instance as the eye moves.
fn drawn(relative: vec3<f32>, placed: vec4<f32>) -> bool {
    let dither = placed.w;
    if dither >= 1.0 {
        return true;
    }
    let origin = placed.xyz;
    let far = length(origin);
    let facing = origin / far;
    let ray = normalize(relative);
    let met = ray * (far / max(dot(ray, facing), 0.01)) - origin;
    var side = cross(surface_up(origin), facing);
    if dot(side, side) < 1e-4 {
        side = cross(vec3<f32>(0.0, 0.0, 1.0), facing);
    }
    side = normalize(side);
    let rise = cross(facing, side);
    let cell_m = max(LEAF_M, far * DITHER_ANGLE);
    let at = vec2<f32>(dot(met, side), dot(met, rise)) / cell_m;
    // Stretched toward even, so a band crosses at an even pace.
    let share = clamp((value_noise(vec3<f32>(at, 3.0), 1024) - 0.5) * 1.7 + 0.5, 0.0, 0.999);
    if dither > 0.0 {
        return share < dither;
    }
    return share >= -dither;
}

// Whether a point of a card is inside its cut: a round clump whose rim is
// ragged with leaves, leaf-sized in metres whatever the card's size, so a
// big mass meets the sky as leaves and never as one scalloped lobe. Nothing
// within is cut, so a mass stays whole inside. A card standing for a whole
// mass, seen only far, is rounder.
fn inside(uv: vec2<f32>, seed: f32, what: f32, half_m: f32) -> bool {
    let d = length(uv);
    let a = atan2(uv.y, uv.x);
    let at = uv * half_m / LEAF_M;
    let leaves = value_noise(vec3<f32>(at, seed * 31.0), 1024) - 0.5;
    let twigs = value_noise(vec3<f32>(at * 2.7, seed * 17.0 + 5.0), 1024) - 0.5;
    let leafy = 0.04 * sin(a * 5.0 + seed * 40.0) + 0.2 * leaves + 0.12 * twigs;
    let ragged = select(1.0, 0.45, what > MASS - 0.5);
    // Never out to the card's square, whose side would cut it straight.
    return d < min(0.82 + leafy * ragged, 0.97);
}

// Light on foliage, painted: the grass's light, on a tone that leans a step
// cooler where the sun does not reach and a step warmer where it does. `mid`
// is the middle tone, linear; `leafy` is a mass of leaves, thin: it gathers
// the sky through the leaves over it, so its underside is deep and never
// black, and lets the sun through when the sun is behind it.
fn painted(mid: vec3<f32>, normal: vec3<f32>, relative: vec3<f32>, open: f32, leafy: bool) -> vec3<f32> {
    var reach = reach_at(relative, normal, normal, open);
    let sun = view.sun.xyz;
    let light = max(dot(normal, sun), 0.0) * reach.day * reach.sun * reach.cloud;
    var cool = mid * vec3<f32>(0.92, 1.0, 1.3);
    var warm = mid * vec3<f32>(1.14, 1.07, 0.80);
    if !leafy {
        // Wood turns by hue more than by value: its shade violet at its own
        // value, for a brown scaled bluer would only go black, and its sun
        // warmer, never lighter, for lighter wood glows.
        cool = mix(mid, dot(mid, LUMA) * VIOLET, 0.6);
        warm = mid * vec3<f32>(1.0, 0.92, 0.78);
    }
    var albedo = mix(cool, mid, smoothstep(0.0, 0.3, light));
    albedo = mix(albedo, warm, smoothstep(0.4, 0.85, light));
    let distance = length(relative);
    let dir = relative / distance;
    if leafy {
        reach.sky = mix(reach.sky, 1.0, THIN);
    }
    var color = lit_by(albedo, normal, 0.0, relative, reach);
    if leafy {
        let behind = pow(max(dot(dir, sun), 0.0), 4.0);
        color += warm * behind * (1.0 - light) * reach.day * daylight(surface_up(relative)) * reach.cloud * THROUGH;
    }
    return through_medium(color, dir, distance);
}

@fragment
fn fs(in: Varying) -> @location(0) vec4<f32> {
    if !drawn(in.relative, in.placed) {
        discard;
    }
    let what = in.card.w;
    var mid = pow(in.color.rgb, vec3<f32>(2.2));
    var normal = normalize(in.normal);
    let base = 1.0 - smoothstep(0.0, BASE_M, in.ground.w);
    if what < WOOD + 0.5 {
        normal = normalize(mix(normal, in.ground.xyz, base));
        let open = mix(in.color.a, ROOTS_OPEN, base);
        let at = in.relative - in.ground.xyz * (in.ground.w * base);
        return vec4<f32>(painted(mid, normal, at, open, false), 1.0);
    }
    let uv = in.card.xy;
    if !inside(uv, in.card.z, what, in.half_lean.x) {
        discard;
    }
    let open = in.color.a;
    var lit_at = in.relative;
    if what < LEAF + 0.5 {
        // A point of a leaf card is that point of its mass, as a ball.
        let ball = normalize(in.relative - in.mass);
        normal = normalize(ball * (1.0 - CROWN) + normal * CROWN);
        lit_at = in.mass + (in.relative - in.mass) * SKIN;
    } else {
        // A card standing for a whole mass bulges toward the eye as the
        // mass would, and is lit and shaded where the mass's skin is.
        let to_eye = -normalize(in.relative);
        let dome = in.right * uv.x + in.upward * uv.y + to_eye * sqrt(max(1.0 - dot(uv, uv), 0.0));
        lit_at = in.mass + dome * in.half_lean.x;
        normal = normalize(dome + normal * CROWN);
    }
    mid *= 1.0 + in.half_lean.y * vec3<f32>(0.10, 0.05, -0.08);
    normal = normalize(mix(normal, in.ground.xyz, base));
    lit_at -= in.ground.xyz * (in.ground.w * base);
    return vec4<f32>(painted(mid, normal, lit_at, mix(open, ROOTS_OPEN, base), true), 1.0);
}

// The sun's pass: wood whole, and a mass as a round card.
@fragment
fn fs_shadow(in: Varying) {
    if in.card.w > MASS - 0.5 && length(in.card.xy) > 0.9 {
        discard;
    }
}
