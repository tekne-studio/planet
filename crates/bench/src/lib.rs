//! What the client costs in WASM, the slowest place it runs and the one where
//! the generator shares a thread with the frame. `scripts/bench.ts` times the
//! exports against budgets.

use std::cell::RefCell;

use client::collision;
use client::{CUBE, Client, Event, Input, Key, Level, PartPose, Recipe, Seat, Shape};
use synth::Filter::{Band, High, Low, Open};
use synth::Source::{Noise, Wave as Tone};
use synth::Wave::{Saw, Square, Triangle};
use synth::{Envelope, Layer, Pitch, Vary, Voice};
use topology::{Sector, SurfacePoint};
use wasm_bindgen::prelude::wasm_bindgen;
use worldgen::{Field, Generator, Source};

thread_local! {
    static DESCENT: RefCell<Option<(Client, Input)>> = const { RefCell::new(None) };
}

/// Body numbers shared with the host.
const MOON: i32 = 1;
/// The planet again, shaped by the field the host loaded.
const FIELD: i32 = 2;

thread_local! {
    static GROUND: RefCell<Option<Field>> = const { RefCell::new(None) };
}

/// Hands over a baked field, so the host can time a world shaped by one.
#[wasm_bindgen]
pub fn load_field(bytes: &[u8]) {
    let field = Field::parse(bytes).expect("a baked field");
    GROUND.with(|held| *held.borrow_mut() = Some(field));
}

/// The recipe of a world over the loaded field, for the bodies that want one.
fn over_field(body: i32) -> Option<(Recipe, Field)> {
    if body != FIELD {
        return None;
    }
    GROUND.with(|held| {
        held.borrow().clone().map(|field| {
            let mut recipe = Recipe::new(1);
            recipe.params.source = Source::Field(field.id());
            (recipe, field)
        })
    })
}

/// Samples `count` directions of a body at the footprint of its finest mesh.
/// A patch is 35 x 35 of them.
#[wasm_bindgen]
pub fn samples(body: i32, count: i32) -> f64 {
    let generator = match over_field(body) {
        Some((recipe, field)) => Generator::with_field(recipe, field).expect("the loaded field"),
        None => Generator::new(Recipe::new(1)).expect("the current generator version"),
    };
    let mut sum = 0.0;
    for i in 0..count {
        let a = f64::from(i) * 1e-4;
        let direction = [a.cos(), a.sin() * 0.8, a.sin() * 0.6];
        let sample = if body == MOON {
            generator.moon_sample_at(direction, 0.8)
        } else {
            generator.sample_at(direction, 1.0)
        };
        sum += sample.height_m;
    }
    sum
}

/// One patch of volume: the columns of the finest quadtree patch, and a
/// window of cells up each of them. The heightfield patch it would replace is
/// `samples`, so the two numbers are what decide whether a volume can be
/// meshed inside a frame at all.
#[wasm_bindgen]
pub fn volume(patches: i32, columns: i32, cells: i32) -> f64 {
    let generator = Generator::new(Recipe::new(1)).expect("the current generator version");
    let mut sum = 0.0;
    for patch in 0..patches {
        for i in 0..columns {
            let a = f64::from(patch) * 1e-3 + f64::from(i) * 1e-5;
            let direction = [a.cos(), a.sin() * 0.8, a.sin() * 0.6];
            let column = generator.column(direction, 0.5);
            let ground_m = column.ground().height_m;
            for k in 0..cells {
                sum += column.density_m(ground_m - f64::from(k) * 0.5);
            }
        }
    }
    sum
}

thread_local! {
    static WALK: RefCell<Option<(Client, Input)>> = const { RefCell::new(None) };
}

/// The seed and the place the caves were found in.
const CAVE_SEED: u64 = 0x0000_0000_cafe_0007;
/// A mouth: walking forward from here goes under the ground inside five
/// seconds, so the walk below pays for the footings and for the patches the
/// body asks to be meshed deeper.
const CAVE_AT: [f64; 2] = [0.3105, 0.6585];

/// Footings across cave country: what one body costs the frame it walks in.
/// A step takes one where the body stands and up to three more for where it
/// might go, so this is multiplied by four, and again by every agent in a
/// world. The columns here are the expensive kind: a column with no cave in
/// it answers from its height and never probes.
#[wasm_bindgen]
pub fn footings(count: i32) -> f64 {
    let generator = Generator::new(Recipe::new(CAVE_SEED)).expect("the current generator version");
    let sphere = generator.sphere();
    let side = f64::from(sphere.blocks().side());
    let mut sum = 0.0;
    for i in 0..count {
        let spread = f64::from(i) * 1e-5;
        let point = SurfacePoint::new(
            Sector::ALL[0],
            (CAVE_AT[0] + spread) * side,
            (CAVE_AT[1] + spread) * side,
        );
        let direction = sphere.blocks().direction(point);
        let ground_m = generator.sample(direction).height_m;
        sum += collision::footing(&generator, direction, ground_m)
            .floor_m
            .unwrap_or_default();
    }
    sum
}

/// Stands a body in cave country and starts it running. This is the frame
/// that reads the density field under the feet: a footing where the body is,
/// and one for each step it tries before it takes one.
#[wasm_bindgen]
pub fn walk_start() {
    let mut client = Client::new(Recipe::new(CAVE_SEED)).expect("the current generator version");
    {
        let side = f64::from(client.sphere().blocks().side());
        client.teleport(topology::SurfacePoint::new(
            topology::Sector::ALL[0],
            CAVE_AT[0] * side,
            CAVE_AT[1] * side,
        ));
    }
    client.pose(0.0, -0.2, 6.0);
    let mut input = Input::default();
    input.key(Key::Forward, true);
    input.key(Key::Sprint, true);
    WALK.with(|walk| *walk.borrow_mut() = Some((client, input)));
}

/// One frame of the walk. The host times the call.
#[wasm_bindgen]
pub fn walk_frame() {
    WALK.with(|walk| {
        let mut walk = walk.borrow_mut();
        let (client, input) = walk.as_mut().expect("walk_start comes first");
        client.update(1.0 / 60.0, input);
        client.drain_events();
        client.drain_terrain_changes();
    });
}

/// Starts a descent from 3 km over a body to its ground: the streamer's
/// hardest moment, every level of the quadtree arriving at once. With
/// `life`, the world's life is on, its plants grown into every patch.
#[wasm_bindgen]
pub fn descent_start(body: i32, life: bool) {
    let mut client = match over_field(body) {
        Some((recipe, field)) => Client::with_field(recipe, field).expect("the loaded field"),
        None => Client::new(Recipe::new(1)).expect("the current generator version"),
    };
    if life {
        // A measure has no world to say life is on: it stands in for one.
        client.rehearse(Level::Admin);
    }
    if body == MOON {
        client.visit_moon(3000.0, -1.2, 6.0);
    } else {
        client.pose(3000.0, -1.2, 6.0);
    }
    let mut input = Input::default();
    input.key(Key::Down, true);
    DESCENT.with(|descent| *descent.borrow_mut() = Some((client, input)));
}

/// One frame of the descent. The host times the call.
#[wasm_bindgen]
pub fn descent_frame() {
    DESCENT.with(|descent| {
        let mut descent = descent.borrow_mut();
        let (client, input) = descent.as_mut().expect("descent_start comes first");
        client.update(1.0 / 60.0, input);
        client.drain_events();
        client.drain_terrain_changes();
    });
}

thread_local! {
    static LAY: RefCell<Option<Client>> = const { RefCell::new(None) };
}

/// The steepest plot found near the test world, on sector 4: 32 m of fall
/// across its 32 m, where a solid base is the most cells a platform lays.
const STEEP_AT: [f64; 2] = [0.312, 0.372];

/// Stands a body on the steepest plot and lets the ground settle there, so
/// what follows times the build alone.
#[wasm_bindgen]
pub fn lay_start() {
    let mut client = Client::new(Recipe::new(1)).expect("the current generator version");
    // A measure has no world to say building is on: it stands in for one.
    for plugin in plugins_client::all() {
        client.plug(plugin);
    }
    client.rehearse(Level::Admin);
    {
        let side = f64::from(client.sphere().blocks().side());
        client.teleport(SurfacePoint::new(
            Sector::ALL[4],
            STEEP_AT[0] * side,
            STEEP_AT[1] * side,
        ));
    }
    client.command_json(r#"{"type":"build.platform","side":64}"#);
    client.settled_frame();
    client.drain_events();
    client.drain_terrain_changes();
    client.drain_volume_changes();
    LAY.with(|lay| *lay.borrow_mut() = Some(client));
}

/// Lays a platform of 64 on a solid base, as a front end asks for it
/// between two frames. The host times the call.
#[wasm_bindgen]
pub fn lay_platform() {
    LAY.with(|lay| {
        let mut lay = lay.borrow_mut();
        let client = lay.as_mut().expect("lay_start comes first");
        client.command_json(r#"{"type":"build.lay","base":"solid"}"#);
    });
}

/// One frame after it. True once the platform is laid and drawn whole,
/// which the client says as it settles.
#[wasm_bindgen]
pub fn lay_frame() -> bool {
    LAY.with(|lay| {
        let mut lay = lay.borrow_mut();
        let client = lay.as_mut().expect("lay_start comes first");
        client.update(1.0 / 60.0, &mut Input::default());
        client.drain_terrain_changes();
        client.drain_volume_changes();
        let events = client.drain_events();
        events.iter().any(|event| matches!(event, Event::Settled))
    })
}

thread_local! {
    static PARTS: RefCell<Option<(Client, Input)>> = const { RefCell::new(None) };
}

/// Where the parts stand: dry land in the middle of a sector of the test
/// world, as the parts' own tests have it.
const PARTS_AT: [f64; 2] = [0.41, 0.37];

/// Stands a body among `count` cubes placed in a square around it, a block
/// apart, and looks over them. The frame then draws every one, and the
/// body's footing asks every one near it. The ground settles first: the
/// descents time its arriving.
#[wasm_bindgen]
pub fn parts_start(count: i32) {
    let mut client = Client::new(Recipe::new(1)).expect("the current generator version");
    client.rehearse(Level::Admin);
    client.set_view([1280, 720]);
    let side = f64::from(client.sphere().blocks().side());
    let (u, v) = (PARTS_AT[0] * side, PARTS_AT[1] * side);
    client.teleport(SurfacePoint::new(Sector::ALL[0], u, v));
    client.pose(0.0, -0.6, 12.0);
    let feet = client.host("bench").feet();
    let ground = client.host("bench").ground(feet.seat, feet.point);
    let across = (f64::from(count).sqrt().ceil() as i32).max(1);
    for i in 0..count {
        let (x, y) = (i % across, i / across);
        let at = [
            u + f64::from(x - across / 2) * 3.0,
            v + f64::from(y - across / 2) * 3.0,
            ground,
        ];
        let part = client::Part {
            shape: Shape::Cube,
            pose: PartPose::upright(at, CUBE),
            paint: (i % 16) as u8,
            solid: true,
            shine: 0.0,
            reach_m: 0.0,
            name: None,
            muted: false,
            volume: 0.0,
        };
        client
            .host("bench")
            .place_part(Seat::Sector(feet.seat.sector()), part)
            .expect("a builder places");
    }
    client.settled_frame();
    client.drain_terrain_changes();
    client.drain_model_changes();
    client.drain_events();
    let mut input = Input::default();
    input.key(Key::Forward, true);
    PARTS.with(|parts| *parts.borrow_mut() = Some((client, input)));
}

/// One frame among the parts, the body walking through them. The host
/// times the call.
#[wasm_bindgen]
pub fn parts_frame() {
    PARTS.with(|parts| {
        let mut parts = parts.borrow_mut();
        let (client, input) = parts.as_mut().expect("parts_start comes first");
        client.update(1.0 / 60.0, input);
        client.drain_events();
        client.drain_terrain_changes();
        client.drain_model_changes();
    });
}

/// How many parts the frame draws: what the scene holds, so a budget is
/// read against a count.
#[wasm_bindgen]
pub fn parts_count() -> i32 {
    PARTS.with(|parts| {
        let parts = parts.borrow();
        let (client, _) = parts.as_ref().expect("parts_start comes first");
        client.parts().all().count() as i32
    })
}

/// `count` footings of the body among the parts placed: what one step asks
/// of them, apart from the frame.
#[wasm_bindgen]
pub fn parts_footings(count: i32) -> f64 {
    PARTS.with(|parts| {
        let mut parts = parts.borrow_mut();
        let (client, _) = parts.as_mut().expect("parts_start comes first");
        let feet = client.host("bench").feet();
        let mut sum = 0.0;
        for i in 0..count {
            let spread = f64::from(i) * 1e-3;
            let point = SurfacePoint::new(feet.point.sector, feet.point.u + spread, feet.point.v);
            let footing =
                client
                    .parts()
                    .footing(client.generator(), feet.seat, point, feet.height_m);
            sum += footing
                .and_then(|footing| footing.floor_m)
                .unwrap_or_default();
        }
        sum
    })
}

/// The rate a voice is rendered at.
const RATE: u32 = 48_000;

/// A long voice, each layer the dearest of its kind: a second of a gliding,
/// wavering saw through a filter, a square, a triangle and noise. A shell
/// renders a voice once a seed, in the frame it is first struck.
static LONG: Voice = Voice::new(
    "bench",
    Vary {
        cents: 50.0,
        gain: 0.1,
        time: 0.0,
    },
    &[
        Layer::new(
            0.0,
            Tone(
                Saw,
                Pitch::steady(220.0).glide(440.0, 1.0).vibrato(5.0, 20.0),
            ),
            SECOND,
            Low { hz: 2000.0, q: 0.7 },
            0.25,
        ),
        Layer::new(
            0.0,
            Tone(Square, Pitch::steady(330.0).vibrato(6.0, 10.0)),
            SECOND,
            High { hz: 200.0, q: 0.7 },
            0.2,
        ),
        Layer::new(
            0.0,
            Tone(Triangle, Pitch::steady(110.0).glide(55.0, 1.0)),
            SECOND,
            Open,
            0.25,
        ),
        Layer::new(0.0, Noise, SECOND, Band { hz: 3000.0, q: 2.0 }, 0.2),
    ],
);

const SECOND: Envelope = Envelope::new(0.01, 0.79, 0.2);

/// `rounds` renders of the long voice, a seed each.
#[wasm_bindgen]
pub fn voice_render(rounds: i32) -> f64 {
    let mut samples = vec![0.0; LONG.length(0, RATE)];
    let mut sum = 0.0;
    for seed in 0..rounds.max(0) as u32 {
        LONG.render(seed, RATE, &mut samples);
        sum += f64::from(samples[RATE as usize / 2]);
    }
    sum
}

thread_local! {
    static STAGE: RefCell<Option<Stage>> = const { RefCell::new(None) };
}

/// Herd country: on sector 2 of the test world, where the herds near fill
/// the animals drawn, 24 of them.
const HERDS_AT: (usize, [f64; 2]) = (2, [0.095, 0.23]);

/// The bearing a rider crosses herd country on, degrees: dry land for the
/// ten seconds of a gallop. A walker keeps the bearing it is born with.
const ACROSS_DEG: f64 = 30.0;

/// A world as a person has it, every plugin and life on, a body in herd
/// country, and the frame's turn of what the shell does between two
/// frames: what the client asked of it drained, and the games and covers it
/// asked to show answered as opened.
struct Stage {
    client: Client,
    input: Input,
    /// The games the client keeps running, by the pictures they run into.
    games: Vec<u64>,
    /// The bearing a rider steers to, degrees.
    steer_deg: Option<f64>,
    /// How far the body turns where it stands each frame, as a pointer
    /// looks: pixels across.
    turn: f32,
    /// Where the eye stood after the last frame.
    eye: glam::DVec3,
    /// Whether the eye has moved since [`Stage::watch_the_eye`].
    eye_moved: bool,
    /// Whether the last frame found the eye where the one before left it,
    /// after it moved, and built no ground: the eye rests and the view
    /// under it is built.
    eye_rests: bool,
}

impl Stage {
    fn new() -> Stage {
        let mut client = Client::new(Recipe::new(1)).expect("the current generator version");
        // A measure has no world to say what is on: it stands in for one.
        for plugin in plugins_client::all() {
            client.plug(plugin);
        }
        client.rehearse(Level::Admin);
        client.set_view([1280, 720]);
        client.pose(0.0, -0.3, 6.0);
        let mut stage = Stage {
            client,
            input: Input::default(),
            games: Vec::new(),
            steer_deg: None,
            turn: 0.0,
            eye: glam::DVec3::ZERO,
            eye_moved: false,
            eye_rests: false,
        };
        stage.stand_in_herd_country();
        stage
    }

    /// Stands the body in herd country and lets the ground there settle
    /// before anything is timed: a descent already times it arriving.
    fn stand_in_herd_country(&mut self) {
        let side = f64::from(self.client.sphere().blocks().side());
        let (sector, [u, v]) = HERDS_AT;
        self.client
            .teleport(SurfacePoint::new(Sector::ALL[sector], u * side, v * side));
        self.client.settled_frame();
        self.frame();
    }

    /// One frame, and the shell's turn after it.
    fn frame(&mut self) -> Vec<Event> {
        if let Some(to) = self.steer_deg {
            let off = self
                .client
                .here()
                .bearing_deg
                .map_or(0.0, |at| (to - at + 540.0).rem_euclid(360.0) - 180.0);
            self.input.key(Key::Right, off > 2.0);
            self.input.key(Key::Left, off < -2.0);
        }
        self.input.look[0] += self.turn;
        let client = &mut self.client;
        let eye = client.update(1.0 / 60.0, &mut self.input).camera.position;
        let built = client
            .drain_terrain_changes()
            .iter()
            .any(|change| matches!(change, scene::TerrainChange::Add(..)));
        let still = eye == self.eye;
        self.eye_moved |= !still;
        self.eye_rests = self.eye_moved && still && !built;
        self.eye = eye;
        client.drain_volume_changes();
        client.drain_model_changes();
        client.drain_picture_changes();
        for running in client.drain_running() {
            match running {
                client::Running::Open { picture, .. } => {
                    client.running_opened(picture, Ok((160, 120)));
                    self.games.push(picture.0);
                }
                client::Running::Cover { picture, .. } => {
                    client.running_opened(picture, Ok((160, 120)));
                }
                client::Running::Close(picture) => self.games.retain(|&id| id != picture.0),
                _ => {}
            }
        }
        client.drain_events()
    }

    /// Starts watching the eye: [`stage_eye_rests`] says when it has moved
    /// and come to rest over built ground.
    fn watch_the_eye(&mut self) {
        self.eye_moved = false;
        self.eye_rests = false;
    }

    /// The animals drawn about the body, nearest first, none ridden.
    fn near(&mut self) -> Vec<client::Near> {
        let here =
            glam::DVec3::from(self.client.up()) * self.client.generator().sphere().radius_m();
        let mut near: Vec<client::Near> = self
            .client
            .host("bench")
            .animals()
            .iter()
            .copied()
            .filter(|near| !near.ridden)
            .collect();
        near.sort_by(|a, b| (a.at - here).length().total_cmp(&(b.at - here).length()));
        near
    }

    /// Stands the body where an animal stands and mounts it through its
    /// offer, held as an agent holds it; true once the body rides it.
    fn mount(&mut self, animal: &client::Near) -> bool {
        let grid = self.client.generator().sphere().blocks();
        self.client
            .teleport(grid.surface_point(animal.at.to_array()));
        self.frame();
        self.client.command_json(&format!(
            r#"{{"type":"act","plugin":"ride","id":{},"hold":true}}"#,
            animal.number
        ));
        // The hold's ring closes in its own time, a frame at a time.
        for _ in 0..240 {
            let mounted = self.frame().into_iter().any(|event| {
                matches!(
                    event,
                    Event::ModeChanged {
                        mode: client::Mode::Way(_)
                    }
                )
            });
            if mounted {
                // The animal is under its rider from the animals' next turn.
                for _ in 0..10 {
                    self.frame();
                }
                return true;
            }
        }
        false
    }

    /// Takes off on the animal ridden, as F does: true when it has wings
    /// and flies, else the body is on foot again.
    fn take_off(&mut self) -> bool {
        self.client
            .command_json(r#"{"type":"set_mode","mode":"fly"}"#);
        let off = self.frame().into_iter().any(|event| {
            matches!(
                event,
                Event::Rejected { .. }
                    | Event::ModeChanged {
                        mode: client::Mode::Fly | client::Mode::Walk
                    }
            )
        });
        if off {
            self.client
                .command_json(r#"{"type":"set_mode","mode":"walk"}"#);
            self.frame();
        }
        !off
    }

    /// Places a part standing on the ground `ahead_m` in front of the body
    /// and `aside_m` to its right.
    fn place(&mut self, shape: Shape, size_m: [f64; 3], ahead_m: f64, aside_m: f64) {
        let host = self.client.host("bench");
        let body = host.body();
        let right = body.facing.cross(body.up);
        let at = body.position + body.facing * ahead_m + right * aside_m;
        let feet = host.feet_at(at);
        let ground = host.ground(feet.seat, feet.point);
        let part = client::Part {
            shape,
            pose: PartPose::upright(
                [feet.point.u, feet.point.v, ground],
                size_m.map(|m| m / topology::BLOCK_M),
            ),
            paint: 0,
            solid: true,
            shine: 0.0,
            reach_m: 0.0,
            name: None,
            muted: false,
            volume: 0.0,
        };
        self.client
            .host("bench")
            .place_part(Seat::Sector(feet.seat.sector()), part)
            .expect("a builder places");
    }

    fn set(self) {
        STAGE.with(|stage| *stage.borrow_mut() = Some(self));
    }
}

/// A body running through herd country at a sprint, life on: the herds of
/// the regions in reach placed as they come into it, those near posed and
/// drawn, and the regions left behind let go.
#[wasm_bindgen]
pub fn herds_start() {
    let mut stage = Stage::new();
    stage.input.key(Key::Forward, true);
    stage.input.key(Key::Sprint, true);
    stage.set();
}

/// A horse of the herd nearest ridden at a gallop through the same
/// country: the animal stepped under its rider every frame, its hooves on
/// the ground and on what is built, the free ones keeping aside.
#[wasm_bindgen]
pub fn gallop_start() {
    let mut stage = Stage::new();
    let near = stage.near();
    let horse = near.first().expect("a herd near herd country");
    assert!(stage.mount(horse), "the horse nearest is mounted");
    stage.steer_deg = Some(ACROSS_DEG);
    // Sprint with forward from a halt sets off at the pace sprint names.
    stage.input.key(Key::Forward, true);
    stage.input.key(Key::Sprint, true);
    stage.set();
}

/// A pegasus of the herds near flown low over the same country, its wings
/// out, each step over the ground or the sea floor under it.
#[wasm_bindgen]
pub fn flight_start() {
    let mut stage = Stage::new();
    let near = stage.near();
    let flown = near
        .iter()
        .any(|animal| stage.mount(animal) && stage.take_off());
    assert!(flown, "a pegasus among the herds near");
    stage.steer_deg = Some(ACROSS_DEG);
    stage.input.key(Key::Forward, true);
    stage.input.key(Key::Sprint, true);
    stage.set();
}

/// One frame of the stage. The host times the call.
#[wasm_bindgen]
pub fn stage_frame() {
    STAGE.with(|stage| {
        stage
            .borrow_mut()
            .as_mut()
            .expect("a stage starts first")
            .frame();
    });
}

/// Lets every key of the stage go: the body stands where it is.
#[wasm_bindgen]
pub fn stage_stand() {
    STAGE.with(|stage| {
        let mut stage = stage.borrow_mut();
        let stage = stage.as_mut().expect("a stage starts first");
        stage.input.release_all();
        stage.steer_deg = None;
        stage.turn = 0.0;
    });
}

/// How many animals the stage draws this frame.
#[wasm_bindgen]
pub fn stage_animals() -> i32 {
    STAGE.with(|stage| {
        let mut stage = stage.borrow_mut();
        let stage = stage.as_mut().expect("a stage starts first");
        stage.client.host("bench").animals().len() as i32
    })
}

/// The cartridge every cabinet of a measure holds, by a hash no file has:
/// the bench's shell answers every game asked as running, and its host runs
/// `rocks` for each.
const ROCKS: client::ContentId = client::ContentId([7; 32]);

/// `count` cabinets with a game each, in rows of four across in front of the
/// body, a metre and a half deep: the first row at two metres, so one
/// cabinet stands before the body within the prompt's reach and four fill
/// a row.
fn cabinets(stage: &mut Stage, count: i32) {
    let cabinet = Shape::Cabinet(client::Cabinet {
        cart: Some(ROCKS),
        skin: None,
    });
    for i in 0..count {
        let (row, across) = (i / 4, i % 4);
        let aside_m = if count == 1 {
            0.0
        } else {
            (f64::from(across) - 1.5) * 1.1
        };
        stage.place(
            cabinet,
            client::cabinet::SIZE_M,
            2.0 + f64::from(row) * 1.5,
            aside_m,
        );
    }
}

/// A body standing before `count` cabinets with a game each, in view,
/// playing the one in front as its first frames come: the eye eases to its
/// screen and the ground under the eye is built finer as it comes near,
/// the game runs, and the others show their covers. [`stage_eye_rests`]
/// says when the eye has arrived; the bench's shell answers each cabinet
/// as shown ([`stage_games`] says how many run, for the host to run).
#[wasm_bindgen]
pub fn cabinets_start(count: i32) {
    let mut stage = Stage::new();
    cabinets(&mut stage, count);
    // The prompt offers it once a turn has seen it.
    stage.frame();
    stage.client.command_json(r#"{"type":"act"}"#);
    stage.watch_the_eye();
    stage.set();
}

/// Whether the eye, having moved, stood still through the last frame over
/// ground built for it.
#[wasm_bindgen]
pub fn stage_eye_rests() -> bool {
    STAGE.with(|stage| {
        let stage = stage.borrow();
        stage.as_ref().expect("a stage starts first").eye_rests
    })
}

/// How many games the stage's client keeps running.
#[wasm_bindgen]
pub fn stage_games() -> i32 {
    STAGE.with(|stage| {
        let stage = stage.borrow();
        stage.as_ref().expect("a stage starts first").games.len() as i32
    })
}

/// A body standing in herd country among `count` cabinets showing their
/// covers and every horse drawn near, turning where it stands: each frame the
/// prompt chooses among the offers of both plugins, the horses' and the
/// cabinets', and says the one in front as it changes.
#[wasm_bindgen]
pub fn offers_start(count: i32) {
    let mut stage = Stage::new();
    cabinets(&mut stage, count);
    for _ in 0..3 {
        stage.frame();
    }
    // A pointer's pixels a frame: a whole turn in ten seconds.
    stage.turn = 4.2;
    stage.set();
}

/// A world that tells `count` pets, each an account's, every one a horse
/// of a herd where it was tamed, across the planet: those of herd country
/// stand about the body, the rest far. The body then runs through herd
/// country as [`herds_start`] has it, and every frame reads every pet told.
#[wasm_bindgen]
pub fn pets_start(count: i32) {
    let mut stage = Stage::new();
    let grid = stage.client.sphere().blocks();
    let side = f64::from(grid.side());
    let mut pets: Vec<protocol::life::Pet> = Vec::new();
    // Herd country first, then a walk of the planet, sector by sector.
    let (sector, [u, v]) = HERDS_AT;
    let places = std::iter::once((sector, u, v)).chain((0..6).flat_map(|sector| {
        (0..9).flat_map(move |a| {
            (0..9).map(move |b| (sector, 0.1 + 0.1 * f64::from(a), 0.1 + 0.1 * f64::from(b)))
        })
    }));
    for (sector, u, v) in places {
        if pets.len() >= count as usize {
            break;
        }
        stage
            .client
            .teleport(SurfacePoint::new(Sector::ALL[sector], u * side, v * side));
        stage.frame();
        for near in stage.near() {
            if pets.len() >= count as usize || pets.iter().any(|pet| pet.number == near.number) {
                continue;
            }
            let point = grid.surface_point(near.at.to_array());
            let account = pets.len();
            pets.push(protocol::life::Pet {
                number: near.number,
                name: format!("pet {account}"),
                spot: Some(protocol::life::Spot {
                    sector: point.sector.index() as u32,
                    u: point.u,
                    v: point.v,
                    heading: 0.0,
                    at_ms: 0,
                }),
                from: None,
                owner: format!("account {account}"),
                mine: false,
                session: 0,
                ..Default::default()
            });
        }
    }
    assert_eq!(pets.len(), count as usize, "as many animals as pets");
    let told = protocol::ServerMessage {
        message: Some(protocol::server_message::Message::Envelope(
            protocol::Envelope {
                plugin: protocol::life::OWNER.to_owned(),
                kind: protocol::life::PETS.to_owned(),
                payload: protocol::encode(&protocol::life::Pets { pets }),
                ..protocol::Envelope::default()
            },
        )),
    };
    stage.client.receive(&protocol::encode(&told));
    stage.stand_in_herd_country();
    stage.input.key(Key::Forward, true);
    stage.input.key(Key::Sprint, true);
    stage.set();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cabinet_in_front_is_played_and_runs_alone_once_the_eye_rests() {
        for count in [1, 4] {
            cabinets_start(count);
            let frames = (1..=120)
                .find(|_| {
                    stage_frame();
                    stage_eye_rests()
                })
                .expect("the eye rests within two seconds");
            // The eye takes a third of a second to ease to the screen.
            assert!(frames > 15, "{count} cabinets: {frames} frames");
            assert_eq!(stage_games(), 1, "{count} cabinets");
        }
    }
}
