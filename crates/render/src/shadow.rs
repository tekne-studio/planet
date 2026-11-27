//! Sun shadow cascades around the eye, stable in the reference frame of either
//! body. Matrices are built in f64 and act on camera-relative f32 positions.
//!
//! A map's grid holds still on the ground under a moving eye by its snap,
//! and under a moving sun only while its light stands still: the maps are
//! drawn from the sun in steps ([`SunSteps`]), each held for a step's life.
//! The near cascades are drawn from the step begun and the next, and the
//! shaders fade from one picture to the other; the far one, whose texels
//! are far wider than a step moves a shadow, cuts to the nearer step.

#[cfg(test)]
use glam::Vec2;
use glam::{DMat4, DVec3, Mat4, Vec3};
use scene::{Camera, Frame, SunSteps};

pub const CASCADES: usize = 3;
/// The cascades drawn from two steps of the sun and faded between, the
/// nearest first.
pub const FADED: usize = 2;
/// The maps' layers: each cascade, from the step begun where it fades and
/// from the nearer step where it cuts; then each faded cascade from the
/// next step. Shadows that cut draw the first [`CASCADES`] alone.
pub const LAYERS: usize = CASCADES + FADED;

/// The cascade a layer is drawn for, in the order of [`LAYERS`].
pub const fn cascade_of(layer: usize) -> usize {
    layer % CASCADES
}

pub const SIZE: u32 = 1024;
/// Half side of each cascade, metres: contact shadows of an avatar, the relief
/// walked through, and the mountains that shade the horizon at a low sun.
pub const RADII: [f64; CASCADES] = [40.0, 400.0, 4000.0];
/// How far towards the sun, and away from it, a cascade holds casters, metres.
/// A low sun throws a mountain's shadow several times its height.
const REACH: [f64; CASCADES] = [2400.0, 2400.0, 12000.0];

/// How far a caster's depth is pushed from the sun, so a surface never
/// shades itself.
pub const BIAS: wgpu::DepthBiasState = wgpu::DepthBiasState {
    constant: -2,
    slope_scale: -1.5,
    clamp: 0.0,
};

/// What the shaders must agree on, stated once: prepended to every module.
pub fn prelude() -> String {
    format!(
        "const SHADOW_CASCADES: u32 = {CASCADES}u;\nconst SHADOW_FADED: u32 = {FADED}u;\n\
         const SHADOW_LAYERS: u32 = {LAYERS}u;\nconst SHADOW_SIZE: f32 = {SIZE}.0;\n"
    )
}

/// Side of one texel of each cascade on the ground, metres.
pub fn texels_m() -> [f32; 4] {
    let mut texels = [0.0; 4];
    for (texel, radius) in texels.iter_mut().zip(RADII) {
        *texel = (2.0 * radius / f64::from(SIZE)) as f32;
    }
    texels
}

/// What the shaders read the maps by: the side of one texel of each
/// cascade, then how far the faded cascades have gone from the step begun
/// to the next, 0 where the shadows cut.
pub fn reading(frame: &Frame) -> [f32; 4] {
    const { assert!(CASCADES == 3, "the fade rides after the three texels") };
    let mut reading = texels_m();
    reading[CASCADES] = match frame.effects.shadows_fade {
        true => frame.sun_steps.blend.clamp(0.0, 1.0),
        false => 0.0,
    };
    reading
}

/// How many of the layers a frame draws: all of them where the shadows
/// fade, the cascades once where they cut.
pub fn drawn(frame: &Frame) -> usize {
    match frame.effects.shadows_fade {
        true => LAYERS,
        false => CASCADES,
    }
}

/// Each layer's matrix, in the order of [`LAYERS`]; past what the frame
/// draws, copies that nothing reads.
pub fn matrices(frame: &Frame, camera: &Camera) -> [Mat4; LAYERS] {
    let moon = frame.moon.position;
    let on_moon = (camera.position - moon).length() - frame.moon.radius_m
        < camera.position.length() - frame.planet_radius_m;
    let origin = if on_moon { moon } else { DVec3::ZERO };
    let eye = camera.position - origin;
    let SunSteps { from, to, blend } = frame.sun_steps;
    let [from, to] = [from, to].map(|sun| cascades(eye, sun.as_dvec3()));
    let nearer = if blend < 0.5 { &from } else { &to };
    if !frame.effects.shadows_fade {
        return core::array::from_fn(|layer| nearer[cascade_of(layer)]);
    }
    core::array::from_fn(|layer| match layer {
        _ if layer < FADED => from[layer],
        _ if layer < CASCADES => nearer[layer],
        _ => to[layer - CASCADES],
    })
}

/// The cascades around an `eye`, from its body's centre, lit from `sun`:
/// each snapped to its texel in the light's frame, so its grid holds still
/// on the ground while the eye moves.
fn cascades(eye: DVec3, sun: DVec3) -> [Mat4; CASCADES] {
    let sun = sun.normalize();
    let helper = if sun.y.abs() < 0.95 {
        DVec3::Y
    } else {
        DVec3::X
    };
    let rotation = DMat4::look_to_rh(DVec3::ZERO, -sun, helper);
    let position = rotation.transform_point3(eye);
    core::array::from_fn(|cascade| {
        let (radius, reach) = (RADII[cascade], REACH[cascade]);
        let texel = 2.0 * radius / f64::from(SIZE);
        let snap = DVec3::new(
            (position.x / texel).round() * texel,
            (position.y / texel).round() * texel,
            position.z,
        );
        // Light looks down -Z; reverse near/far to retain reversed-Z everywhere.
        let projection = DMat4::orthographic_rh(-radius, radius, -radius, radius, 2.0 * reach, 0.0);
        let offset = position - snap - DVec3::Z * reach;
        (projection * DMat4::from_translation(offset) * rotation).as_mat4()
    })
}

/// How far from where a receiver stands the map is read, texels: the
/// normal's offset in `shadow_sample`, at most 1.3, and the lookups of
/// `shadow_tent`, at most 3 from where it reads; rounded up.
const READ_TEXELS: f32 = 5.0;

/// Where a sight reads the sun's shadows: inside its frustum, four planes
/// through the eye. A caster shades it only if its shadow can fall there;
/// one past a side the shadows run away from, under every sun the maps are
/// drawn from, never does, and casts nothing. The picture is the same, for
/// a third of the casters.
pub struct Seen {
    /// The inward normals of the sides the shadows run away from under
    /// every sun, camera-relative; the rest stay zero.
    sides: [Vec3; 4],
    /// How far each cascade reads past a receiver, metres.
    margins_m: [f32; CASCADES],
}

impl Seen {
    /// The sight of `camera` at `aspect`, with the maps drawn from `suns`.
    pub fn new(camera: &Camera, aspect: f32, suns: &[Vec3]) -> Seen {
        let half_y = camera.fov_y / 2.0;
        let half_x = (half_y.tan() * aspect).atan();
        let rotation = camera.rotation.as_quat();
        // The camera looks down its -Z: each side leans out from it.
        let inward = [
            Vec3::new(half_x.cos(), 0.0, -half_x.sin()),
            Vec3::new(-half_x.cos(), 0.0, -half_x.sin()),
            Vec3::new(0.0, half_y.cos(), -half_y.sin()),
            Vec3::new(0.0, -half_y.cos(), -half_y.sin()),
        ]
        .map(|normal| rotation * normal);
        let away = |normal: Vec3| suns.iter().all(|sun| normal.dot(-sun.normalize()) <= 0.0);
        let texels = texels_m();
        Seen {
            sides: inward.map(|normal| if away(normal) { normal } else { Vec3::ZERO }),
            margins_m: core::array::from_fn(|cascade| texels[cascade] * READ_TEXELS),
        }
    }

    /// Whether the shadow of a sphere, camera-relative, can be read in view
    /// from `cascade`'s map.
    pub fn reaches(&self, cascade: usize, center: Vec3, radius: f32) -> bool {
        let reach = radius + self.margins_m[cascade];
        self.sides
            .iter()
            .all(|side| side.dot(center) + reach >= 0.0)
    }
}

/// Conservative sphere test, including casters outside the camera frustum.
pub fn intersects(matrix: Mat4, center: Vec3, radius: f32) -> bool {
    let p = matrix.transform_point3(center);
    let rows = matrix.transpose();
    let r = Vec3::new(
        rows.x_axis.truncate().length(),
        rows.y_axis.truncate().length(),
        rows.z_axis.truncate().length(),
    ) * radius;
    p.x.abs() <= 1.0 + r.x && p.y.abs() <= 1.0 + r.y && p.z + r.z >= 0.0 && p.z - r.z <= 1.0
}

pub struct Maps {
    pub sampled: wgpu::TextureView,
    pub layers: [wgpu::TextureView; LAYERS],
    pub sampler: wgpu::Sampler,
    pub uniforms: [wgpu::Buffer; LAYERS],
    pub groups: [wgpu::BindGroup; LAYERS],
}

impl Maps {
    pub fn new(device: &wgpu::Device, layout: &wgpu::BindGroupLayout) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("sun shadow maps"),
            size: wgpu::Extent3d {
                width: SIZE,
                height: SIZE,
                depth_or_array_layers: LAYERS as u32,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: super::DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let sampled = texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });
        let layers = std::array::from_fn(|i| {
            texture.create_view(&wgpu::TextureViewDescriptor {
                dimension: Some(wgpu::TextureViewDimension::D2),
                base_array_layer: i as u32,
                array_layer_count: Some(1),
                ..Default::default()
            })
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("sun PCF"),
            compare: Some(wgpu::CompareFunction::GreaterEqual),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let uniforms = std::array::from_fn(|_| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("sun view"),
                size: size_of::<super::ViewUniform>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        });
        let groups = std::array::from_fn(|i| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("sun view"),
                layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniforms[i].as_entire_binding(),
                }],
            })
        });
        Self {
            sampled,
            layers,
            sampler,
            uniforms,
            groups,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The sun of a world, `angle` radians round its day.
    fn toward(angle: f64) -> DVec3 {
        DVec3::new(angle.cos(), 0.35, angle.sin()).normalize()
    }

    /// How far the sun goes from one step to the next, radians.
    const STEP: f64 = 0.25 * core::f64::consts::PI / 180.0;

    /// The steps of the sun `angle` radians round, as a client takes them.
    fn steps(angle: f64) -> SunSteps {
        let begun = (angle / STEP).floor();
        SunSteps {
            from: toward(begun * STEP).as_vec3(),
            to: toward((begun + 1.0) * STEP).as_vec3(),
            blend: (angle / STEP - begun) as f32,
        }
    }

    /// A frame of a planet of `radius_m` with nothing in it, its eye
    /// `camera` and the sun `angle` radians round.
    fn frame(camera: Camera, radius_m: f64, angle: f64) -> Frame {
        Frame {
            camera,
            sun_direction: toward(angle).as_vec3(),
            sun_steps: steps(angle),
            moon: scene::Moon {
                position: DVec3::new(0.0, radius_m * 20.0, 0.0),
                radius_m: radius_m / 4.0,
            },
            planet_radius_m: radius_m,
            clock_s: 0.0,
            patches: Vec::new(),
            shadow_patches: Vec::new(),
            interaction: scene::InteractionCapsule {
                start: camera.position,
                end: camera.position,
                radius_m: 0.0,
            },
            effects: scene::Effects::default(),
            boxes: Vec::new(),
            skinned: Vec::new(),
            volumes: Vec::new(),
            glass: Vec::new(),
            lamps: Vec::new(),
            models: Vec::new(),
            ghost: None,
            guides: Vec::new(),
            pictures: Vec::new(),
            panes: Vec::new(),
            through: None,
            warp: None,
            overlay: None,
        }
    }

    /// A point fixed on the ground falls on the same place within its
    /// texel of every map while the clock runs inside one step of the sun
    /// and the eye walks, the shadows faded or cut: the grid holds still
    /// under it, and so do the edges it draws.
    #[test]
    fn a_point_on_the_ground_holds_its_place_in_a_texel_as_the_sun_moves() {
        // A world of 2^16 and the shortest day a world has, 24 minutes, its
        // sun the fastest; a frame at 60 Hz.
        let radius_m = 20_860.0;
        let per_frame = core::f64::consts::TAU / (24.0 * 60.0) / 60.0;
        let start = 40.05 * STEP;
        let up = DVec3::new(start.cos(), 1.0, start.sin()).normalize();
        let ground = up * radius_m;
        let walk = up.any_orthonormal_vector() * 1.4 / 60.0;
        for fade in [true, false] {
            let place = |picture: usize| {
                let eye = ground + up * 1.7 + walk * picture as f64;
                let camera = Camera {
                    position: eye,
                    rotation: glam::DQuat::IDENTITY,
                    fov_y: 1.0,
                    near: 0.1,
                };
                let mut frame = frame(camera, radius_m, start + per_frame * picture as f64);
                frame.effects.shadows_fade = fade;
                matrices(&frame, &camera).map(|matrix| {
                    let at = matrix.transform_point3((ground - eye).as_vec3());
                    let texel = (at.truncate() * 0.5 + 0.5) * SIZE as f32;
                    texel - texel.floor()
                })
            };
            let first = place(0);
            // Inside the step, short of its half, where a cut is taken: the
            // sun turns 0.4 of it.
            for picture in 1..(0.4 * STEP / per_frame) as usize {
                for (layer, (now, was)) in place(picture).iter().zip(&first).enumerate() {
                    let moved = (*now - *was).abs();
                    let moved = moved.min(Vec2::ONE - moved);
                    assert!(
                        moved.max_element() < 0.01,
                        "fade {fade}, layer {layer}, picture {picture}: {was} became {now}"
                    );
                }
            }
        }
    }

    #[test]
    fn caster_bounds_include_offscreen_and_reject_outside_light_volume() {
        let m = Mat4::orthographic_rh(-32.0, 32.0, -32.0, 32.0, 4800.0, 0.0);
        assert!(intersects(m, Vec3::new(0.0, 0.0, -1000.0), 1.0));
        assert!(intersects(m, Vec3::new(33.0, 0.0, -1000.0), 2.0));
        assert!(!intersects(m, Vec3::new(40.0, 0.0, -1000.0), 1.0));
        assert!(!intersects(m, Vec3::new(0.0, 0.0, 10.0), 1.0));
    }

    fn camera(rotation: glam::DQuat) -> Camera {
        Camera {
            position: DVec3::ZERO,
            rotation,
            fov_y: 1.0,
            near: 0.1,
        }
    }

    #[test]
    fn a_shadow_falling_away_from_the_view_is_not_cast() {
        let looking = camera(glam::DQuat::IDENTITY);
        let behind = Vec3::new(0.0, 0.0, 200.0);
        let above_behind = Vec3::new(0.0, 150.0, 100.0);
        // The sun ahead throws shadows back, away from the view.
        let ahead = Seen::new(&looking, 16.0 / 9.0, &[Vec3::new(0.0, 1.0, -1.0)]);
        assert!(!ahead.reaches(1, behind, 2.0));
        assert!(!ahead.reaches(1, above_behind, 2.0));
        assert!(ahead.reaches(1, Vec3::new(0.0, -2.0, -50.0), 2.0));
        // The sun behind throws them forward, into it from above.
        let behind_sun = Seen::new(&looking, 16.0 / 9.0, &[Vec3::new(0.0, 1.0, 1.0)]);
        assert!(behind_sun.reaches(1, above_behind, 2.0));
    }

    /// Whatever the camera's turn and the two steps of the sun the maps are
    /// drawn from, a caster whose shadow is not cast never had it fall in
    /// view: marched along either sun's rays, it never comes within the
    /// map's reading of the frustum.
    #[test]
    fn a_shadow_not_cast_never_enters_the_view() {
        let mut state = 0x9e37_79b9_7f4a_7c15_u64;
        let mut roll = move || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state >> 11) as f32 / (1u64 << 53) as f32 * 2.0 - 1.0
        };
        let aspect = 16.0 / 9.0;
        let (mut culled, mut cases) = (0, 0);
        for _ in 0..200 {
            let turn = glam::DQuat::from_euler(
                glam::EulerRot::YXZ,
                f64::from(roll()) * 3.2,
                f64::from(roll()) * 1.5,
                f64::from(roll()) * 0.3,
            );
            let looking = camera(turn);
            let sun = Vec3::new(roll(), roll().abs() + 0.05, roll()).normalize();
            let next = (sun + Vec3::new(roll(), roll(), roll()) * 0.01).normalize();
            let seen = Seen::new(&looking, aspect, &[sun, next]);
            let half_y = looking.fov_y / 2.0;
            let half_x = (half_y.tan() * aspect).atan();
            let sides = [
                Vec3::new(half_x.cos(), 0.0, -half_x.sin()),
                Vec3::new(-half_x.cos(), 0.0, -half_x.sin()),
                Vec3::new(0.0, half_y.cos(), -half_y.sin()),
                Vec3::new(0.0, -half_y.cos(), -half_y.sin()),
            ];
            let to_camera = turn.inverse().as_quat();
            for _ in 0..50 {
                let center = Vec3::new(roll(), roll(), roll()) * 400.0;
                let radius = (roll() + 1.0) * 5.0;
                for cascade in 0..CASCADES {
                    cases += 1;
                    if seen.reaches(cascade, center, radius) {
                        continue;
                    }
                    culled += 1;
                    let reach = radius + seen.margins_m[cascade];
                    for step in 0..4000 {
                        for sun in [sun, next] {
                            let at = to_camera * (center - sun * step as f32);
                            let inside = sides.iter().all(|side| side.dot(at) + reach >= 0.0);
                            assert!(
                                !inside,
                                "{center} cast nothing into {cascade}, and falls in view"
                            );
                        }
                    }
                }
            }
        }
        assert!(culled * 4 > cases, "{culled} of {cases} not cast");
    }
}
