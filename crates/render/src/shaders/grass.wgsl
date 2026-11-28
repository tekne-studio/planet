// Three curved blades per tuft, roots fixed on the drawn terrain. A tuft in
// a bed of flowers is a flower: a stem, and two heads of petals turned to
// the eye, their shape cut here by the flower's form.
struct Patch { offset: vec4<f32>, anchor: vec4<f32> }
@group(1) @binding(0) var<uniform> placement: Patch;

struct Instance {
    @location(0) root_height: vec4<f32>,
    @location(1) normal_reach: vec4<f32>,
    @location(2) color_angle: vec4<f32>,
    @location(3) bloom: vec4<f32>,
}
struct Varying {
    @builtin(position) clip: vec4<f32>,
    @location(0) relative: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec3<f32>,
    @location(3) tip: f32,
    @location(4) far: f32,
    @location(5) bloom: vec4<f32>,
    // A flower's head: xy where on it, -1..1; z 1 on a head, 0 elsewhere.
    @location(6) head: vec3<f32>,
}

// A head's half side, metres, the second's share of it, and how far under
// the first it opens.
const HEAD_M: f32 = 0.055;
const SECOND: f32 = 0.75;
const SECOND_DROP: f32 = 0.16;
// A stem's width against a blade's.
const STEM: f32 = 0.18;

// The six corners of a head's card, two triangles; past them a corner
// collapses on the middle, a triangle of no area.
fn head_corner(i: u32) -> vec2<f32> {
    let corners = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, -1.0), vec2<f32>(1.0, 1.0),
        vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, 1.0), vec2<f32>(-1.0, 1.0));
    if i >= 6u {
        return vec2<f32>(0.0);
    }
    return corners[i];
}

@vertex
fn vs(in: Instance, @builtin(vertex_index) index: u32) -> Varying {
    let shape = array<vec2<f32>,9>(
        vec2<f32>(-1.0,0.0), vec2<f32>(1.0,0.0), vec2<f32>(-0.65,0.55),
        vec2<f32>(1.0,0.0), vec2<f32>(0.65,0.55), vec2<f32>(-0.65,0.55),
        vec2<f32>(-0.65,0.55), vec2<f32>(0.65,0.55), vec2<f32>(0.0,1.0));
    let p = shape[index % 9u];
    let root = in.root_height.xyz + placement.offset.xyz;
    // Grass grows on the planet only: up is away from its centre.
    let up = normalize(view.camera.xyz + root);
    let helper = select(vec3<f32>(0.0,1.0,0.0), vec3<f32>(1.0,0.0,0.0), abs(up.y) > 0.9);
    let east = normalize(cross(helper,up));
    let north = cross(up,east);
    let angle = in.color_angle.w + f32(index/9u)*2.094395;
    let side = east*cos(angle)+north*sin(angle);
    let forward = cross(side,up);
    let distance = length(root);
    let fade = 1.0-smoothstep(in.normal_reach.w*0.68,in.normal_reach.w,distance);
    // Far tufts stand for the meadow between them: taller and wider, so the
    // field keeps its cover while the count falls.
    let far = smoothstep(35.0,400.0,distance);
    let height = in.root_height.w * fade * (1.0+far*1.4);
    let anchored = placement.anchor.xyz + in.root_height.xyz;
    // Integer periods keep wind continuous when the scene clock wraps at 3600s.
    let t = view.flags.y * 6.2831853;
    let wind = sin(dot(anchored,vec3<f32>(1.0,2.0,1.0)*(6.2831853/64.0))+t/6.0)*0.16
        + sin(t/2.0+in.color_angle.w)*0.035;
    let axis = view.interaction_end.xyz-view.interaction_start.xyz;
    let along = clamp(dot(root-view.interaction_start.xyz,axis)/max(dot(axis,axis),0.001),0.0,1.0);
    let away = root-(view.interaction_start.xyz+axis*along);
    let influence = 1.0-smoothstep(view.interaction_start.w*0.3,view.interaction_start.w+0.6,length(away));
    let lateral = away-up*dot(away,up);
    let push = lateral/max(length(lateral),0.05)*influence*0.55;
    let bend = east*wind + forward*height*0.25 + push;
    // A flower blooms if the share of the flowers drawn keeps it, by a
    // roll of its own; one it does not is grass.
    let kept = fract(in.color_angle.w * 7.31) < view.foliage.y;
    let flower = in.bloom.a > 0.0 && kept;
    let blade = index / 9u;
    // A flower's stem stands a little over the grass, and is thin.
    let tall = select(1.0, 1.08, flower);
    let width = select(1.0, STEM, flower);
    let offset = up*(height*tall*p.y*(1.0-influence*0.45)) + side*(p.x*mix(0.07,1.1,far)*fade*width) + forward*(0.05*fade) + bend*(p.y*p.y)*fade;
    var out: Varying;
    out.relative = root+offset;
    // Lit as the slope it grows on, or a field on a dark hillside glitters.
    let ground = normalize(in.normal_reach.xyz);
    out.normal = normalize(mix(ground*0.8+forward*0.2, ground, far));
    out.head = vec3<f32>(0.0);
    if flower && blade > 0u {
        // A head at the stem's top, the second under it, both turned to
        // the eye and tipped toward the sky; far, a head stands for the
        // flowers about it and grows as a far tuft does.
        let second = f32(blade - 1u);
        let top = root + up*(height*tall*(1.0-influence*0.45)) + forward*(0.05*fade) + bend*fade
            - up*(height*SECOND_DROP*second) + side*(0.05*second);
        let to_eye = -normalize(top);
        var across = cross(up, to_eye);
        if dot(across, across) < 1e-6 {
            across = east;
        }
        let right = normalize(across);
        let facing = normalize(to_eye + up*0.8);
        let upward = normalize(cross(facing, right));
        let corner = head_corner(index % 9u);
        let half = HEAD_M * mix(1.0, SECOND, second) * fade * (1.0 + far*1.6);
        out.relative = top + (right*corner.x + upward*corner.y)*half;
        out.normal = normalize(up + to_eye*0.6);
        out.head = vec3<f32>(corner, 1.0);
    }
    out.clip = view.clip_from_relative * vec4<f32>(out.relative,1.0);
    out.color = in.color_angle.xyz;
    out.tip = p.y;
    out.far = far;
    out.bloom = select(vec4<f32>(0.0), in.bloom, flower);
    return out;
}

// Whether a point of a head is on a petal, by the flower's form: five round
// petals, many thin rays, a five-pointed bell, five notched petals. Its
// distance out, a share of the petal's reach there, or past 1 off it.
fn petal(uv: vec2<f32>, form: u32) -> f32 {
    let r = length(uv);
    let a = atan2(uv.y, uv.x);
    var reach = 0.62 + 0.38*sqrt(abs(cos(a*2.5)));
    if form == 2u {
        reach = 0.42 + 0.58*pow(abs(cos(a*6.5)), 1.5);
    } else if form == 3u {
        reach = 0.5 + 0.5*(1.0 - abs(sin(a*2.5)));
    } else if form == 4u {
        reach = 0.6 + 0.4*pow(abs(cos(a*2.5)), 0.4) - 0.16*pow(abs(cos(a*5.0)), 12.0);
    }
    return r / reach;
}

@fragment
fn fs(in: Varying) -> @location(0) vec4<f32> {
    if in.head.z > 0.5 {
        let form = u32(round(in.bloom.a * 255.0));
        let out = petal(in.head.xy, form);
        if out > 1.0 {
            discard;
        }
        // Petals deepen toward the heart, which is gold in a daisy and a
        // buttercup and dark in the rest.
        let heart = select(in.bloom.rgb*0.35, vec3<f32>(0.85, 0.55, 0.06), form <= 2u);
        let r = length(in.head.xy);
        var color = in.bloom.rgb*mix(0.78, 1.08, smoothstep(0.2, 0.9, out));
        color = mix(heart, color, smoothstep(0.17, 0.24, r));
        return vec4<f32>(lit(color, normalize(in.normal), 0.0, in.relative), 1.0);
    }
    // Occlusion: a blade stands among its neighbours, so light reaches its
    // tip and little of its root. From afar the field averages out, and dark
    // roots would read as dirt specks.
    let occlusion = mix(ROOTS_OPEN, 1.0, smoothstep(0.0, 0.85, in.tip));
    let blade = in.color * mix(occlusion, 1.0, in.far) * mix(1.0, 1.18, in.tip);
    return vec4<f32>(lit(blade,normalize(in.normal),0.0,in.relative), 1.0);
}
