//! The client core: controller, terrain streaming, and the command/event seam.
//!
//! No window, no DOM, no GPU. A platform shell feeds [`Input`] and a time
//! step, forwards [`Command`]s from its UI, hands over the frames of a link to
//! a world server and sends back what is queued for it, and hands the
//! resulting [`scene::Frame`], terrain and volume changes to a renderer. An agent
//! drives the same type and simply never renders.

mod animals;
mod assets;
mod boom;
mod box_figure;
pub mod cabinet;
mod carts;
mod cells;
pub mod collision;
mod controller;
mod controls;
mod figure;
mod given;
mod grass;
mod hand;
mod history;
mod hover;
mod input;
mod life;
mod live;
mod marker;
pub mod model3d;
mod offer;
mod overlay;
mod paces;
mod parts;
mod peers;
mod pets;
mod pictures;
mod place;
mod plants;
mod plugin;
mod seam;
mod session;
mod sound;
mod speakers;
mod spring;
mod terrain;
mod trunks;
pub mod voices;
mod wardrobe;
mod way;

use std::collections::{BTreeMap, HashMap};

use gaia::flora::Flora;
use glam::{DAffine3, DMat3, DQuat, DVec3};
use scene::{ModelChange, PictureChange, SkinnedChange, TerrainChange, VolumeChange};
use topology::{Sector, SurfacePoint};
pub use worldgen::{Field, Recipe};
use worldgen::{GENERATOR_VERSION, Generator, Material, Params, Sample};

use animals::{Animals, Rider};
pub use animals::{BUILT, Keeping, Near};
pub use assets::{AssetRequest, Form};
use assets::{MANIFEST_PATH, Manifest, Purpose, Requests};
use carts::Carts;
pub use carts::{GAME_WAITS_AWAY_S, GAME_WAITS_S, Running};
pub use cells::{Aim, Cells, Guide, Met, PALETTE, REACH_M, Refusal, Seat, Sight};
pub use collision::BODY_M;
use controller::Refused;
pub use controller::{Built, Controller, Wish};
use controls::Controls;
pub use controls::{Look, Pad};
use figure::{Clips, Figure, Motion, Playing};
pub use figure::{Gait, Play};
pub use hand::Thing;
use hand::Things;
pub use history::{History, Owner};
pub use hover::Hover;
pub use input::{Chord, Input, Key, KeyAsk};
use life::Life;
pub use live::LIVE_AT_ONCE;
use live::Live;
use marker::Markers;
pub use marker::{At as MarkerAt, Choice as MarkerChoice, Marked, Marker, Said as MarkerSaid};
pub use model3d::ModelRefusal;
pub use offer::{Act, Hold, Offer, On, Prompt, Reached, Stands};
use offer::{Offered, Press, Pressed};
pub use paces::{Backing, Flight, Jump, Pace, Paces};
pub use part::{
    CARTRIDGE, Cabinet, Clip, ClipKind, ContentId, ImageKind, Media, NAME_BYTES, Name, Part,
    PartId, PictureLevel, Portal, Pose as PartPose, Shape,
};
pub use parts::is_lamp;
pub use parts::{By, CUBE, Facing, Landmarked, Mark, PartRefusal, Parts, Someone, VOXEL};
use peers::Peers;
use pets::Pets;
pub use pets::{Pet, Spot};
use pictures::Pictures;
pub use pictures::{FULL_SIDE, LOW_SIDE, Moving, Screen, THUMB_SIDE};
pub use place::Pose;
pub use plugin::{Eye, Feet, Host, InWay, PartHit, Plugin, Standing, Turn};
use plugin::{Lent, Plugins};
pub use scene::{Effects, Frame, ToneMap};
pub use seam::{
    Anchor, Animal, Command, Event, Label, Level, Mode, PeerInfo, PetOf, PluginOn, Pointed,
    SessionStatus, Waiting,
};
pub use session::Outbound;
use session::Session;
pub use sound::{
    Group, STRUCK_AT_ONCE, SUSTAINS_AT_ONCE, Sound, SoundSource, Strike, Sustain, VARIANTS,
};
use sound::{Strikes, Sustains};
use speakers::Speakers;
pub use speakers::{CLIPS_AT_ONCE, Sounding};
use terrain::{Body, Budget, Cover, Kind, Terrain, Viewpoint};
pub use trunks::Trunks;
use wardrobe::Wardrobe;
use way::Ways;
pub use way::{Held, Profile, Steering, Step, Straddle, Way};

/// Seconds for the sun to go around once.
const DAY_S: f64 = 1200.0;

/// How wide a body is for how tall it stands, roughly: what the prompt
/// keeps clear of on the screen.
const BODY_BUILD: f32 = 0.45;
/// Turns of the moon per turn of the sun.
const MOON_PACE: f64 = 0.93;
/// Centre of the planet to centre of the moon, metres: twenty of its radii,
/// so it fills as much of the sky whatever its size.
const MOON_ORBIT_M: f64 = 20.0 * MOON_RADIUS_M;
use worldgen::MOON_RADIUS_M;
const STATS_EVERY_S: f64 = 0.5;
/// Where a body's mouth is, over its feet: what it sounds comes from there.
const MOUTH_M: f64 = 1.5;
/// How far to the side of the body a pet called comes to stand, metres:
/// clear of it, as a free animal keeps from a body.
const BESIDE_M: f64 = 2.5;
/// How far the body goes from where it last called its pet, metres,
/// before the pet on its way is called again: past a body's sway, and
/// within reach of where it stands beside it.
const CHASE_M: f64 = 4.0;
/// How far the body stopped from where it last called its pet, metres,
/// for the pet on its way to be called once more, to beside where it
/// stopped: past a step to the side.
const SETTLE_M: f64 = 1.0;
/// How far a body goes in a frame and still stands, metres.
const STILL_M: f64 = 0.001;
/// How long the pet on its way runs to one place at least, seconds,
/// before it is called again: a world told twice a second at most.
const CHASE_S: f64 = 0.5;
/// How high over the ground a pet's name hangs when its kind carries no
/// rider to say how tall it stands, metres.
const PET_TALL_M: f64 = 1.5;
/// How high over a saddle a pet's name hangs while nobody rides it, metres.
const OVER_BACK_M: f64 = 0.5;

/// A pet called on its way to its keeper's body: called again as the body
/// goes on, until it stands beside it.
#[derive(Clone, Copy, Debug)]
struct Chase {
    /// Where the body stood when the pet was last called.
    body: DVec3,
    /// When it was last called, seconds of the world's clock.
    called_s: f64,
    /// Where the body stood the frame before.
    was: DVec3,
}

/// Where a mark over an animal hangs, its name or a marker: over its back,
/// and over its rider's head while it is ridden.
fn over_animal(near: &Near) -> DVec3 {
    let up = near.at.normalize_or(DVec3::Z);
    let tall_m = near
        .kind
        .carry
        .map_or(PET_TALL_M, |carry| match near.ridden {
            true => carry.height_m,
            false => carry.saddle.y + OVER_BACK_M,
        });
    near.at + up * tall_m
}

/// A marked thing found this frame: the ground under it, where its mark
/// hangs, how far along the ground, and whether its nametag hangs there.
struct Found {
    at: DVec3,
    over: DVec3,
    away_m: f64,
    tagged: bool,
}

pub struct Client {
    generator: Generator,
    /// Kept so a new recipe over the same ground needs no second download.
    field: Option<Field>,
    controller: Controller,
    terrain: Terrain,
    moon_terrain: Terrain,
    /// The cells of the world: its volumes, and what is drawn of them.
    cells: Cells,
    /// The parts of the world: what is placed that is no cell.
    parts: Parts,
    /// What grows on the planet: gaia's plants, grown for this ground the
    /// first time a world has life on it, and kept while life goes off and
    /// comes back.
    flora: Option<Flora>,
    /// Whether what grows is drawn and stands: the world's life, as
    /// [`Client::tend`] last found it.
    living: bool,
    /// The trunks of the plants about the body, which stop it.
    trunks: Trunks,
    /// Every change a hand made here, in order, of whatever owner, to take
    /// back and put back as one list.
    history: History,
    /// What the last world to welcome this client said it may do. `None`
    /// until one has: with no world, nothing is changed.
    level: Option<Level>,
    /// The plugin that has the pointer, while one does: the pointer stays
    /// free to aim and its button is that plugin's.
    pointer: Option<&'static str>,
    /// The body's controls and the eye, and who they are lent to.
    controls: Controls,
    /// The player's own body.
    figure: Figure,
    /// The clips every figure shares.
    clips: Clips,
    /// Every avatar loaded, by asset reference, worn by any number of bodies.
    wardrobe: Wardrobe,
    pictures: Pictures,
    /// The picture each cabinet's skin shows this frame, by its original's
    /// id: what `pictures` holds of it, asked for as a picture is.
    skins: HashMap<part::ContentId, scene::PictureId>,
    /// The games on the cabinets' screens.
    carts: Carts,
    /// The budget the moving pictures and the games share.
    live: Live,
    speakers: Speakers,
    /// Every source of sound this frame: the playing pictures that carry
    /// sound, and the clips within reach.
    heard: Vec<Sound>,
    /// The voices struck at a place, for the shell to play.
    strikes: Strikes,
    /// The voices sustained this frame, for the shell to keep playing.
    sustains: Sustains,
    session: Session,
    peers: Peers,
    /// What a shell plugged in, and which of it the world has on.
    plugins: Plugins,
    /// The ways of moving the plugins plugged in gave, each on while its
    /// plugin is.
    ways: Ways,
    /// The things the plugins plugged in gave a hand, each on while its
    /// plugin is, and the one this body holds, by its full name.
    things: Things,
    hand: Option<String>,
    /// Whether the world this client is in has life, at this version.
    life: Life,
    /// The animals of a world with life: the herds near the body and the
    /// animals bodies ride.
    animals: Animals,
    /// The pets of the world, as it told them, and this client's asks of
    /// its life.
    pets: Pets,
    /// The pet called, followed while it runs to the body.
    chase: Option<Chase>,
    /// What each plugin drew over the world in its last turn, by its name.
    drawn: Vec<(&'static str, Vec<scene::BoxPart>)>,
    /// What each plugin offered the body in its last turn.
    offered: Offered,
    /// What each giver pointed out in the world in its last turn.
    markers: Markers,
    /// The revision of the parts their home and favourites were last
    /// marked at.
    parts_marked: Option<u64>,
    /// Where each label drawn this frame hangs in the world: a marker there
    /// is tagged.
    labelled: Vec<DVec3>,
    /// What the prompt shows, as the front end was last told.
    told_prompt: Option<Prompt>,
    /// The key held on an offer with a hold, until it taps, holds or is
    /// let go.
    press: Option<Press>,
    /// How far the ring was told to have closed.
    told_ring: Option<f32>,
    /// How the body moves, as the front end was last told.
    told_mode: Mode,
    requests: Requests,
    manifest: Option<Manifest>,
    /// A random avatar was asked for before the manifest arrived.
    wants_random_avatar: bool,
    /// The asset reference of the avatar asked for last.
    wanted_avatar: Option<String>,
    /// The account's avatar, as the world's welcome says mundos signed it.
    /// While it holds, `V` and a random pick pass: the choice is made in
    /// mundos.
    account_avatar: Option<String>,
    /// What the front end was last told of the player's avatar: the choice,
    /// and whether the body is in it yet.
    told_avatar: (Option<String>, bool),
    /// What to be called, as the front end set it. Rides in Hello.
    wanted_name: String,
    events: Vec<Event>,
    effects: Effects,
    /// Width over height of the view, for culling.
    aspect: f32,
    /// The view's size in pixels, as the shell gave it: what a picture's
    /// size on the screen is measured in, and where a screen is laid.
    view_px: [u32; 2],
    /// The part of the view the shell leaves clear of what it lays over
    /// it, as it said, pixels `[left, top, right, bottom]`: what the eye
    /// is held on is framed in it ([`overlay::clear`]).
    clear_px: Option<[f64; 4]>,
    /// The world's clock, seconds. Online it is the server's, set by Welcome,
    /// so every client shares one sky; offline it counts from the world's
    /// making. The sun, the moon and the weather are functions of it.
    clock_s: f64,
    /// Sun angle at `clock_s = 0`: mid morning at the spawn, a fact of the
    /// world, so clients that set their clocks by one server agree on the sun.
    noon_offset: f64,
    stats_timer_s: f64,
    frames_since_stats: u32,
    /// Whether the last frame hung anchors, so the first empty frame after
    /// them still says so.
    had_anchors: bool,
    had_labels: bool,
    had_waiting: bool,
    /// A body's passage, through a portal or travelling, while it lasts.
    passage: Option<Passage>,
    /// A clip asked for over what the body moves by, while it plays.
    played: Option<Playing>,
    /// How much each portal's pane shows the world seen through it, by
    /// name: eased toward 1 for the one seen through, toward 0 for the
    /// rest, so a lens becomes a window and back over [`SEEN_S`].
    seen: BTreeMap<PartId, f32>,
    /// Whether the last frame's streaming found nothing to build.
    settled: bool,
    entropy: u64,
}

impl Client {
    /// A world that needs only its seed. A recipe that names a field is
    /// refused here: that one needs [`Client::with_field`].
    pub fn new(recipe: Recipe) -> Result<Client, worldgen::RecipeError> {
        Client::build(Generator::new(recipe)?, None)
    }

    /// A world shaped by a baked field. The field is kept, so a later
    /// `SetRecipe` onto the same ground costs nothing but the respawn.
    pub fn with_field(recipe: Recipe, field: Field) -> Result<Client, worldgen::RecipeError> {
        let generator = Generator::with_field(recipe, field.clone())?;
        Client::build(generator, Some(field))
    }

    fn build(generator: Generator, field: Option<Field>) -> Result<Client, worldgen::RecipeError> {
        let controller = Controller::spawn(spawn_point(&generator), &generator);
        // The body every streamer is printed at, the size its recipe names.
        let sphere = generator.sphere();
        let mut client = Client {
            generator,
            field,
            controller,
            terrain: Terrain::new(Body::new(Kind::Planet, sphere)),
            moon_terrain: Terrain::new(Body::new(Kind::Moon, sphere)),
            cells: Cells::default(),
            parts: Parts::default(),
            flora: None,
            living: false,
            trunks: Trunks::default(),
            history: History::default(),
            level: None,
            pointer: None,
            controls: Controls::default(),
            figure: Figure::default(),
            clips: Clips::new(),
            wardrobe: Wardrobe::default(),
            pictures: Pictures::default(),
            skins: HashMap::new(),
            carts: Carts::default(),
            live: Live::default(),
            speakers: Speakers::default(),
            heard: Vec::new(),
            strikes: Strikes::default(),
            sustains: Sustains::default(),
            session: Session::default(),
            peers: Peers::default(),
            plugins: Plugins::default(),
            ways: Ways::default(),
            things: Things::default(),
            hand: None,
            life: Life::default(),
            animals: Animals::default(),
            pets: Pets::default(),
            chase: None,
            drawn: Vec::new(),
            offered: Vec::new(),
            markers: Markers::default(),
            labelled: Vec::new(),
            parts_marked: None,
            told_prompt: None,
            press: None,
            told_ring: None,
            told_mode: Mode::Walk,
            requests: Requests::default(),
            manifest: None,
            wants_random_avatar: false,
            wanted_avatar: None,
            account_avatar: None,
            told_avatar: (None, false),
            wanted_name: String::new(),
            events: vec![
                Event::Ready {
                    generator_version: GENERATOR_VERSION,
                },
                Event::Palette {
                    colors: PALETTE
                        .iter()
                        .map(|[r, g, b]| format!("#{r:02x}{g:02x}{b:02x}"))
                        .collect(),
                },
            ],
            effects: Effects::default(),
            aspect: 16.0 / 9.0,
            view_px: [1920, 1080],
            clear_px: None,
            clock_s: 0.0,
            noon_offset: 0.0,
            stats_timer_s: 0.0,
            frames_since_stats: 0,
            had_anchors: false,
            had_labels: false,
            had_waiting: false,
            passage: None,
            played: None,
            seen: BTreeMap::new(),
            settled: false,
            entropy: 0,
        };
        client
            .requests
            .ask(MANIFEST_PATH.to_owned(), Purpose::Manifest);
        client.face_the_sun();
        client.events.push(Event::RecipeChanged {
            recipe: client.generator.recipe().clone(),
        });
        client.events.push(Event::EffectsChanged {
            effects: client.effects,
        });
        client.events.push(Event::Session {
            status: SessionStatus::Offline,
            session: None,
            level: None,
        });
        Ok(client)
    }

    /// The generator for a recipe, with the field this client holds when the
    /// recipe asks for one. A UI that wants other ground hands it over first.
    fn generator_for(&self, recipe: Recipe) -> Result<Generator, worldgen::RecipeError> {
        match (Generator::required_field(&recipe), self.field.clone()) {
            (Some(_), Some(field)) => Generator::with_field(recipe, field),
            _ => Generator::new(recipe),
        }
    }

    /// The ground a field world is shaped by, before its recipe arrives.
    /// Fails when the bytes are not a field; the world it belongs to is
    /// checked against its id when the recipe comes.
    pub fn set_field(&mut self, bytes: &[u8]) -> Result<(), worldgen::FieldError> {
        self.field = Some(Field::parse(bytes)?);
        Ok(())
    }

    /// Takes what a UI asked for, within what is sane, and says what it took.
    fn set_effects(&mut self, effects: Effects) {
        let sane = |value: f32, low: f32, high: f32, fallback: f32| {
            if value.is_finite() {
                value.clamp(low, high)
            } else {
                fallback
            }
        };
        let usual = Effects::default();
        self.effects = Effects {
            resolution: sane(effects.resolution, 0.25, 1.0, usual.resolution),
            cloud_cover: sane(effects.cloud_cover, 0.0, 1.0, usual.cloud_cover),
            cloud_density: sane(effects.cloud_density, 0.1, 3.0, usual.cloud_density),
            wind_m_s: sane(effects.wind_m_s, 0.0, 80.0, usual.wind_m_s),
            cloud_change: sane(effects.cloud_change, 0.0, 6.0, usual.cloud_change),
            exposure: sane(effects.exposure, 0.1, 4.0, usual.exposure),
            bloom: sane(effects.bloom, 0.0, 3.0, usual.bloom),
            bloom_threshold: sane(effects.bloom_threshold, 0.2, 8.0, usual.bloom_threshold),
            haze: sane(effects.haze, 0.0, 6.0, usual.haze),
            water_clarity: sane(effects.water_clarity, 0.25, 12.0, usual.water_clarity),
            video_play_m: sane(effects.video_play_m, 16.0, 1024.0, usual.video_play_m),
            picture_reach_m: sane(effects.picture_reach_m, 32.0, 2048.0, usual.picture_reach_m),
            model_reach_m: sane(effects.model_reach_m, 16.0, 1024.0, usual.model_reach_m),
            foliage_near_m: sane(
                effects.foliage_near_m,
                0.0,
                terrain::PLANTS_M[1],
                usual.foliage_near_m,
            ),
            foliage_mid_m: sane(
                effects.foliage_mid_m,
                0.0,
                terrain::PLANTS_M[1],
                usual.foliage_mid_m,
            ),
            foliage_reach_m: sane(
                effects.foliage_reach_m,
                terrain::PLANTS_M[0],
                terrain::PLANTS_M[1],
                usual.foliage_reach_m,
            ),
            undergrowth: sane(effects.undergrowth, 0.0, 1.0, usual.undergrowth),
            ..effects
        };
        self.terrain.set_reach(self.effects.foliage_reach_m);
        self.events.push(Event::EffectsChanged {
            effects: self.effects,
        });
    }

    pub fn recipe(&self) -> &Recipe {
        self.generator.recipe()
    }

    /// The size of the view in pixels: its aspect, and how tall a picture
    /// stands on it.
    pub fn set_view(&mut self, size: [u32; 2]) {
        self.view_px = size.map(|n| n.max(1));
        let [width, height] = self.view_px.map(|n| n as f32);
        self.aspect = width / height;
        self.controller.set_tall(height > width);
    }

    /// The part of the view the shell leaves clear of what it lays over
    /// it, pixels from its top left, `[left, top, right, bottom]`: a pad,
    /// a bar, a row of keys are outside it. What the eye is held on is
    /// framed inside it, a game played filling it. `None` leaves room
    /// round the middle, as for a page with keys.
    pub fn set_clear(&mut self, clear: Option<[f64; 4]>) {
        self.clear_px = clear;
    }

    /// Stirs in randomness only a platform has (a clock, the browser). Without
    /// it every random choice is reproducible, which headless shots rely on.
    pub fn add_entropy(&mut self, bits: u64) {
        self.entropy ^= mix(bits);
    }

    /// Sets the clock: a server's on Welcome, a fixed one for reproducible
    /// headless renders.
    pub fn set_clock(&mut self, seconds: f64) {
        self.clock_s = seconds;
    }

    /// The body this world is printed at: its size, and everything in metres
    /// that follows from it. Frozen with the recipe.
    pub fn sphere(&self) -> topology::QuadSphere {
        self.generator.sphere()
    }

    /// The moon of the world: its size, and the grid what is built on it is
    /// cut by.
    pub fn moon(&self) -> topology::QuadSphere {
        self.generator.moon()
    }

    /// Stands the avatar at a point of any sector. Arrival, not travel.
    pub fn teleport(&mut self, point: SurfacePoint) {
        self.controller.teleport(point, &self.generator);
    }

    /// Local up where the avatar stands: out of the body it is on.
    pub fn up(&self) -> [f64; 3] {
        self.controller.up().to_array()
    }

    /// Where the avatar is and which way it looks, as a link carries it.
    pub fn here(&self) -> place::Pose {
        let grid = self.generator.sphere().blocks();
        // Every body is parametrised on the planet's sector grid, so the moon
        // has a column too: the one its radial passes through.
        let point = if self.controller.on_moon() {
            grid.surface_point(self.controller.radial().to_array())
        } else {
            self.controller.point()
        };
        place::Pose {
            on_moon: self.controller.on_moon(),
            column: grid.column_of(point),
            // Standing on the ground is what leaves the height out: the ground
            // is a function of the recipe, so it says itself.
            h: (!self.controller.grounded())
                .then(|| (self.controller.height_m() / topology::BLOCK_M) as i16),
            bearing_deg: topology::bearing_deg(
                self.controller.up().to_array(),
                self.controller.view().to_array(),
            ),
            pitch_deg: Some(self.controller.pitch().to_degrees()),
            // How far back a person watches from is theirs; the eye is a way
            // of looking, and a link copied there opens there.
            boom_m: self.controller.first_person().then_some(0.0),
            chars: topology::CODE_MAX,
        }
    }

    /// Stands the avatar where a pose says and looks the way it says:
    /// `"4-K7M42Q"`, or `"m4-K7M42Q@40,180,-5"`. An arrival, at once: the
    /// way a page opens at an address and a body is born at home, under the
    /// front end's veil until [`Event::Settled`].
    ///
    /// A code shorter than full precision names a box, so the middle of that
    /// box is where you land: a person who quotes four characters means the
    /// neighbourhood, and the middle of it is the least surprising answer.
    pub fn go_to(&mut self, text: &str) -> Result<(), String> {
        let moon = self.moon_position();
        land(&mut self.controller, &self.generator, moon, text)?;
        // What lives near waits on what is built there, not where it was.
        self.cells.arrived();
        // A height off the ground is a flight, and a front end that showed a
        // walker has to hear it: the panel says what the body does.
        self.tell_mode();
        // A leap is a new place: it is said settled again once drawn there,
        // even where nothing was left to stream, so a front end that veils
        // a leap lifts it.
        self.settled = false;
        Ok(())
    }

    /// Travels to a place as [`Client::go_to`] reads it: through the
    /// passage a portal takes a body by, with no portal. The picture is
    /// drawn into the body, held while the ground at the place comes, and
    /// bursts out of it there. Wrong, the text says what is wrong and the
    /// body stays.
    pub fn travel(&mut self, text: &str) -> Result<(), String> {
        place::Pose::parse(self.generator.sphere().blocks(), text)?;
        Passage::lead(&mut self.passage, text.to_owned(), None, self.clock_s);
        Ok(())
    }

    /// Flies to `gap_m` metres under the moon, on the side that faces the
    /// planet, and aims the camera. For previews.
    pub fn visit_moon(&mut self, gap_m: f64, pitch: f64, boom_m: f64) {
        self.controller.pose_view(pitch, boom_m);
        let moon = self.moon_position();
        let below = moon.normalize() * (moon.length() - MOON_RADIUS_M - gap_m);
        self.controller.set_moon(controller::MoonBody {
            center: moon,
            radius_m: MOON_RADIUS_M,
        });
        self.controller.fly_to(below);
    }

    /// Poses the avatar and camera for a preview: see [`Controller::pose`].
    pub fn pose(&mut self, altitude_m: f64, pitch: f64, boom_m: f64) {
        self.controller
            .pose(altitude_m, pitch, boom_m, &self.generator);
    }

    pub fn command(&mut self, command: Command) {
        match command {
            Command::SetRecipe { recipe } => match self.generator_for(recipe) {
                Ok(generator) => self.regenerate(generator),
                Err(error) => self.events.push(Event::Rejected {
                    message: error.to_string(),
                }),
            },
            Command::SetMode { mode } => self.set_mode(mode),
            Command::SetAvatar { path } => self.wear(path),
            Command::RandomAvatar => self.random_avatar(),
            Command::NextAvatar => self.next_avatar(),
            Command::SetEffects { effects } => self.set_effects(effects),
            Command::GoTo { place } => {
                if let Err(message) = self.go_to(&place) {
                    self.events.push(Event::Rejected { message });
                }
            }
            Command::Travel { place } => {
                if let Err(message) = self.travel(&place) {
                    self.events.push(Event::Rejected { message });
                }
            }
            Command::SetName { name } => {
                self.session.rename(&name);
                self.wanted_name = name;
            }
            Command::PutDown => {
                let (plugins, lent) = self.lend();
                plugins.put_down(lent);
            }
            Command::SetChoices { home, favourites } => self.parts.kept(parts::Choices {
                home,
                favourites: favourites.into_iter().collect(),
            }),
            Command::Act { plugin, id, hold } => self.act_asked(plugin, id, hold),
            Command::Press { controls } => self.press_asked(&controls),
            Command::Leave => match self.controls_lent() {
                true => self.controls.take_back(),
                false => self.reject_unlent(),
            },
            Command::Screen { part, indices } => {
                let key = match part {
                    Some(part) => match carts::game_of(&self.parts, PartId(part)) {
                        Some(key) => Some(key),
                        None => {
                            return self.events.push(Event::Rejected {
                                message: format!("no cabinet with a game is part {part}"),
                            });
                        }
                    },
                    None => None,
                };
                if let Err(message) = self.carts.read(key, indices, &self.live) {
                    self.events.push(Event::Rejected {
                        message: message.into(),
                    });
                }
            }
            Command::Offers => {
                let offers = offer::within_reach(self.offers(), &self.standing());
                self.events.push(Event::Offers { offers });
            }
            Command::Animals => {
                let animals = self.animals_near();
                self.events.push(Event::Animals { animals });
            }
            Command::Markers => {
                let markers = self.markers_pointed();
                self.events.push(Event::Markers { markers });
            }
            Command::HideMarkers { kinds } => self.markers.hide(kinds),
            Command::Play(play) => {
                let mode = self.controller.mode();
                match Playing::new(play, self.clock_s, None, mode) {
                    Some(playing) => self.played = Some(playing),
                    None => self.events.push(Event::Rejected {
                        message: "`for_s` is no length of time a clip plays: ten seconds at most"
                            .into(),
                    }),
                }
            }
            Command::Sound { voice } => {
                if let Err(message) = self.sound(&voice) {
                    self.events.push(Event::Rejected { message });
                }
            }
            Command::SetPet { pet } => self.pets.set_kept(pet),
            Command::NamePet { name } => match (gaia::fauna::named(&name), self.pets.mine()) {
                (Some(name), Some(_)) => {
                    self.pets.name(name);
                }
                (None, _) => self.events.push(Event::Rejected {
                    message: format!(
                        "a pet's name is trimmed, no control character, at most {} bytes",
                        gaia::fauna::NAME_BYTES
                    ),
                }),
                (_, None) => self.events.push(Event::Rejected {
                    message: "this body keeps no pet".into(),
                }),
            },
        }
    }

    /// The body sounds a voice of the core's by its name, from its mouth.
    fn sound(&mut self, name: &str) -> Result<(), String> {
        let &(voice, reach_m) = voices::CALLS
            .iter()
            .find(|(voice, _)| voice.name == name)
            .ok_or_else(|| format!("no voice is called `{name}`"))?;
        let mouth = self.controller.position() + self.controller.up() * MOUTH_M;
        let seed = self.strikes.strike(voice, Group::Calls, mouth, reach_m);
        self.session.sound(voice.name, seed);
        if voice.name == voices::WHISTLE.name {
            self.call_pet();
        }
        Ok(())
    }

    /// The place of a point on the planet, as `go_to` takes it.
    fn place_of(&self, at: DVec3) -> String {
        let grid = self.generator.sphere().blocks();
        let point = grid.surface_point(at.to_array());
        let place = place::Pose {
            on_moon: false,
            column: grid.column_of(point),
            h: None,
            bearing_deg: None,
            pitch_deg: None,
            boom_m: None,
            chars: topology::CODE_MAX,
        };
        place.place(grid)
    }

    /// Every animal drawn near as an agent reads it, nearest first.
    fn animals_near(&self) -> Vec<Animal> {
        let radius_m = self.generator.sphere().radius_m();
        let body = self.controller.position();
        let mut animals: Vec<Animal> = self
            .animals
            .near()
            .iter()
            .map(|near| Animal {
                number: near.number,
                kind: near.kind.name,
                place: self.place_of(near.at),
                away_m: near.at.angle_between(body) * radius_m,
                bearing_deg: topology::bearing_deg(near.at.to_array(), near.heading.to_array()),
                ridden: near.ridden,
                pet: self.pets.of(near.number).map(|pet| PetOf {
                    name: pet.name.clone(),
                    mine: pet.mine,
                }),
            })
            .collect();
        animals.sort_by(|a, b| a.away_m.total_cmp(&b.away_m));
        animals
    }

    /// Every marker given as an agent reads it, nearest first, with where
    /// its thing is: one found nowhere is left out.
    fn markers_pointed(&self) -> Vec<Pointed> {
        let body = self.controller.position();
        let up = body.normalize_or(DVec3::Z);
        let mut pointed: Vec<Pointed> = self
            .markers_shown()
            .filter_map(|(giver, marker)| {
                let found = self.find_marked(marker.on)?;
                let toward = found.at - body;
                let along = toward - up * toward.dot(up);
                Some(Pointed {
                    marker: MarkerSaid::of(giver, marker),
                    place: self.place_of(found.at),
                    away_m: found.away_m,
                    bearing_deg: (along.length() > 1e-6)
                        .then(|| topology::bearing_deg(up.to_array(), along.to_array()))
                        .flatten(),
                })
            })
            .collect();
        pointed.sort_by(|a, b| a.away_m.total_cmp(&b.away_m));
        pointed
    }

    /// Where the thing a marker marks is now: where it stands, the point a
    /// mark hangs at over it, how far it is along the ground from the
    /// body, and whether a label drawn this frame hangs there. None while
    /// it is nowhere to be found.
    fn find_marked(&self, on: Marked) -> Option<Found> {
        let (at, over) = match on {
            Marked::Animal(number) => {
                let near = self.animals.find(number, self.clock_s, &self.generator)?;
                (near.at, over_animal(&near))
            }
            Marked::Point(point) => (point, point),
        };
        let body = self.controller.position();
        let away_m = match self.controller.on_moon() {
            true => (at - body).length(),
            false => at.angle_between(body) * self.generator.sphere().radius_m(),
        };
        let tagged = self
            .labelled
            .iter()
            .any(|label| label.distance(over) < TAGGED_M);
        Some(Found {
            at,
            over,
            away_m,
            tagged,
        })
    }

    /// The name a pet drawn near wears over it: named, within a nametag's
    /// reach of the body.
    fn wears_name(&self, near: &Near) -> Option<&str> {
        let pet = self
            .pets
            .of(near.number)
            .filter(|pet| !pet.name.is_empty())?;
        ((near.at - self.controller.position()).length() <= LABEL_M).then_some(pet.name.as_str())
    }

    /// What the givers point out, when that changed, and where each shown
    /// is for the eye this frame, when one moved. The parts give the home
    /// and favourites themselves, as a plugin gives its own, when they
    /// changed.
    fn tell_markers(&mut self, camera: &scene::Camera) {
        let revision = self.parts.revision();
        if self.parts_marked != Some(revision) {
            self.parts_marked = Some(revision);
            let marked = self.parts.marked(&self.generator);
            self.markers.give("parts", marked);
        }
        if let Some(markers) = self.markers.resaid() {
            self.events.push(Event::MarkersChanged { markers });
        }
        let projection = Projection::of(camera, self.aspect);
        let at: Vec<MarkerAt> = self
            .markers_shown()
            .filter_map(|(giver, marker)| {
                let found = self.find_marked(marker.on)?;
                let ([x, y], edge_deg) = marker::place(
                    projection.seen(found.over),
                    projection.near,
                    projection.aspect,
                );
                Some(MarkerAt {
                    giver,
                    id: marker.id,
                    x,
                    y,
                    edge_deg,
                    distance_m: found.away_m as f32,
                    tagged: found.tagged && edge_deg.is_none(),
                })
            })
            .collect();
        let [width_px, height_px] = self.view_px.map(|n| n as f32);
        if let Some(at) = self.markers.retold(at, width_px, height_px) {
            self.events.push(Event::MarkersAt { at });
        }
    }

    /// A peer sounded a voice of the core's: struck at its mouth with its
    /// seed, within the voice's reach; a jump's spring at the horse it
    /// rides, as that horse's. A voice this client does not hold, or a peer
    /// not yet seen, is not heard.
    fn sounded(&mut self, sounded: &protocol::Sounded) {
        let seed = sounded.seed % sound::VARIANTS;
        if sounded.voice == voices::SPRING.name {
            if let Some(animal) = self.animals.under(Some(sounded.session)) {
                let spring = voices::leap(&voices::SPRING, animal.traits(), animal.leap_force());
                let (at, reach_m) = (animal.withers(), voices::LEAP_REACH_M);
                self.strikes
                    .struck_with(&spring, Group::Steps, seed, at, reach_m);
            }
            return;
        }
        let Some(&(voice, reach_m)) = voices::CALLS
            .iter()
            .find(|(voice, _)| voice.name == sounded.voice)
        else {
            return;
        };
        let moon = self.moon_position();
        let Some(mouth) = self
            .peers
            .over_feet(sounded.session, MOUTH_M, moon, &self.ways)
        else {
            return;
        };
        self.strikes
            .struck_with(voice, Group::Calls, seed, mouth, reach_m);
    }

    /// The world's clock, milliseconds since the Unix epoch.
    fn clock_ms(&self) -> u64 {
        (self.clock_s * 1000.0).max(0.0) as u64
    }

    /// The pet a whistle would call now: the body has one, is not on it,
    /// and stands where animals live, on the planet with life on. What a
    /// front end shows the whistle by.
    fn callable(&self) -> Option<&Pet> {
        let pet = self.pets.mine()?;
        let here = self.life.on() && !self.controller.on_moon();
        (here && self.controller.carrier() != pet.number).then_some(pet)
    }

    /// A whistle calls the body's pet to stand beside the body, facing its
    /// way. Within [`gaia::fauna::HEARS_M`] it runs from where it is seen.
    /// From past it, it is out of sight a while, by how far it was, then
    /// comes into sight behind the eye and gallops the rest. How far it was,
    /// and how long it is out of sight, is said. Answered, it follows the
    /// body on its way ([`Client::chase_pet`]).
    fn call_pet(&mut self) {
        self.chase = None;
        let Some(pet) = self.callable() else {
            return;
        };
        let (number, spot) = (pet.number, pet.spot);
        let radius_m = self.generator.sphere().radius_m();
        let body = self.controller.position();
        let seen = self
            .animals
            .near()
            .iter()
            .find(|near| near.number == number);
        // On its way, it goes on from where it runs, at its speed; else
        // from where it is seen, from a standstill.
        let afoot = (self.animals)
            .afoot(number, self.clock_s, &self.generator)
            .filter(|afoot| afoot.on_its_way);
        let (at, heading, from_mps) = match (afoot, seen) {
            (Some(afoot), _) => (afoot.at, afoot.heading, afoot.speed_mps),
            (None, Some(near)) => (near.at, near.heading, 0.0),
            (None, None) => match animals::spot_at(&spot, &self.generator) {
                Some((up, heading)) => (up * radius_m, heading, 0.0),
                None => return,
            },
        };
        let away_m = at.angle_between(body) * radius_m;
        let (beside, facing) = self.beside();
        let now_ms = self.clock_ms();
        let mut coming_s = 0.0;
        if away_m > gaia::fauna::ANSWERS_M {
            self.events.push(Event::Whistled {
                away_m,
                coming_s,
                answered: false,
            });
            return;
        }
        if away_m <= gaia::fauna::HEARS_M {
            let from = animals::spot_of(at, heading, now_ms, &self.generator);
            let to = animals::spot_of(beside, facing, now_ms, &self.generator);
            self.pets.call(from, to, from_mps);
        } else {
            coming_s = gaia::fauna::coming_s(away_m);
            let comes_ms = now_ms + (coming_s * 1000.0).round() as u64;
            // Behind the eye, wherever the body faces, on ground it
            // stands on; with none about, beside the body.
            let ahead = self.controller.camera(&self.generator).rotation * DVec3::NEG_Z;
            let enter = Animals::kind_of(number)
                .and_then(|kind| animals::entrance(kind, beside, ahead, &self.generator))
                .unwrap_or(beside);
            let toward = (beside - enter).normalize_or(facing);
            let enter = animals::spot_of(enter, toward, now_ms, &self.generator);
            let to = animals::spot_of(beside, facing, comes_ms, &self.generator);
            // Out of sight it ran: it comes into sight at its gallop.
            let gallop_mps = self.animals.called_mps(number).unwrap_or(0.0);
            self.pets.call_from_afar(enter, to, gallop_mps);
        }
        self.chase = Some(Chase {
            body,
            called_s: self.clock_s,
            was: body,
        });
        self.events.push(Event::Whistled {
            away_m,
            coming_s,
            answered: true,
        });
    }

    /// Where a pet called comes to stand: [`BESIDE_M`] to the body's right,
    /// facing its way.
    fn beside(&self) -> (DVec3, DVec3) {
        let (up, facing) = (self.controller.up(), self.controller.facing());
        let body = self.controller.position();
        (
            body + up.cross(facing).normalize_or_zero() * BESIDE_M,
            facing,
        )
    }

    /// The pet called follows its keeper, as the keeper may be on the move:
    /// once the body is [`CHASE_M`] from where it last called, or stopped
    /// [`SETTLE_M`] from there, and the pet has run [`CHASE_S`] since, it
    /// is called again, from where the world's record has it, to beside
    /// the body. Every client draws each run from the record, as one call.
    /// It ends once the pet stands beside the body, a whistle and not a
    /// leash, or once the body is past earshot of it: the pet runs on to
    /// where it was last called and waits there. Out of sight, called from
    /// afar, it is followed once it comes into sight.
    fn chase_pet(&mut self) {
        let Some(mut chase) = self.chase else {
            return;
        };
        let body = self.controller.position();
        let still = (body - chase.was).length() < STILL_M;
        chase.was = body;
        self.chase = Some(chase);
        let number = self.pets.mine().map(|pet| pet.number);
        let Some(number) = number.filter(|&number| {
            self.controller.carrier() != number && self.life.on() && !self.controller.on_moon()
        }) else {
            self.chase = None;
            return;
        };
        let Some(run) = self.animals.afoot(number, self.clock_s, &self.generator) else {
            return;
        };
        let moved_m = (body - chase.body).length();
        if moved_m < CHASE_M && !(still && moved_m >= SETTLE_M) {
            if !run.on_its_way && moved_m < SETTLE_M {
                self.chase = None;
            }
            return;
        }
        if self.clock_s - chase.called_s < CHASE_S {
            return;
        }
        let radius_m = self.generator.sphere().radius_m();
        if run.at.angle_between(body) * radius_m > gaia::fauna::HEARS_M {
            self.chase = None;
            return;
        }
        let (beside, facing) = self.beside();
        let now_ms = self.clock_ms();
        let from = animals::spot_of(run.at, run.heading, now_ms, &self.generator);
        let to = animals::spot_of(beside, facing, now_ms, &self.generator);
        self.chase = self.pets.call(from, to, run.speed_mps).then_some(Chase {
            body,
            called_s: self.clock_s,
            was: body,
        });
    }

    /// Whether a plugin has the pointer, a tool in hand: a shell hands the
    /// pointer's button over as [`Key::Use`] then, and keeps the pointer free
    /// to aim with.
    pub fn pointing(&self) -> bool {
        self.pointer.is_some()
    }

    /// Whether the body's controls are lent: its keys press the holder's
    /// controls, Enter and Escape among them, and the eye is the holder's,
    /// so a shell lets a captured pointer go.
    pub fn controls_lent(&self) -> bool {
        self.controls.holder().is_some()
    }

    /// Whether a plugin that is on in this world asks for a key of the
    /// keyboard, by the name the web gives it, with what is held with it. A
    /// shell hands such a key to [`Input::code`] and keeps it from whatever
    /// else would take it.
    pub fn asks(&self, code: &str, chord: Chord) -> bool {
        // While the controls are lent every key is theirs.
        !self.controls_lent() && self.plugins.asks(code, chord)
    }

    /// The cells of the world, to read.
    pub fn cells(&self) -> &Cells {
        &self.cells
    }

    /// The plugins, and what of the core their host lends them for a call.
    fn lend(&mut self) -> (&mut Plugins, Lent<'_>) {
        let moon = self.moon_position();
        let lent = Lent {
            session: &mut self.session,
            events: &mut self.events,
            generator: &self.generator,
            cells: &mut self.cells,
            parts: &mut self.parts,
            trunks: &self.trunks,
            history: &mut self.history,
            controller: &mut self.controller,
            level: self.level,
            pointer: &mut self.pointer,
            controls: &mut self.controls,
            carts: &mut self.carts,
            live: &self.live,
            passage: &mut self.passage,
            played: &mut self.played,
            clock_s: self.clock_s,
            ways: &self.ways,
            things: &self.things,
            hand: &mut self.hand,
            peers: &self.peers,
            animals: &self.animals,
            pets: &mut self.pets,
            drawn: &mut self.drawn,
            offered: &mut self.offered,
            markers: &mut self.markers,
            moon,
        };
        (&mut self.plugins, lent)
    }

    /// A host for a plugin held outside this client: what a plugin's own
    /// tests stand on.
    pub fn host(&mut self, plugin: &'static str) -> Host<'_> {
        self.lend().1.host(plugin)
    }

    /// Stands in for a world's word where there is no world to say it: this
    /// session has `level`, and every plugin plugged in is on at the version
    /// it holds. For a headless picture, a bench and a test. A person's
    /// client hears both from a world alone.
    pub fn rehearse(&mut self, level: Level) {
        self.level = Some(level);
        let plugins = self.plugins.rehearse();
        self.life.rehearse();
        self.tend();
        self.switch_ways(&plugins);
        self.state(plugins);
    }

    /// Draws the plants when life came on, grown the first time, and stops
    /// drawing them when it went off, every patch built again to say so.
    fn tend(&mut self) {
        let on = self.life.on();
        if on == self.living {
            return;
        }
        self.living = on;
        if on && self.flora.is_none() {
            self.flora = Some(Flora::grow(&self.generator));
        }
        self.trunks = Trunks::default();
        self.terrain.replant();
    }

    /// Commands arriving as JSON over the seam. Bad JSON becomes a
    /// [`Event::Rejected`], never a panic: the other side is a UI. A `type`
    /// with a dot in it is a plugin's, `chat.say`, and goes to that plugin.
    pub fn command_json(&mut self, json: &str) {
        let mut body = match serde_json::from_str::<serde_json::Value>(json) {
            Ok(body) => body,
            Err(error) => {
                return self.events.push(Event::Rejected {
                    message: error.to_string(),
                });
            }
        };
        let named = body
            .get("type")
            .and_then(serde_json::Value::as_str)
            .and_then(|name| name.split_once('.'))
            .map(|(plugin, kind)| (plugin.to_owned(), kind.to_owned()));
        if let Some((plugin, kind)) = named {
            if let Some(fields) = body.as_object_mut() {
                fields.remove("type");
            }
            return self.plugin_command(&plugin, &kind, body);
        }
        match serde_json::from_value::<Command>(body) {
            Ok(command) => self.command(command),
            Err(error) => self.events.push(Event::Rejected {
                message: error.to_string(),
            }),
        }
    }

    /// Plugs in a plugin's client half. A shell does it once, before the
    /// link opens, for each plugin its version carries. It stays off until a
    /// world says it is on. A way or a thing it gives whose name a stance
    /// cannot carry is refused.
    pub fn plug(&mut self, plugin: Box<dyn Plugin>) {
        let ways = self.ways.give(plugin.name(), plugin.ways());
        let things = self.things.give(plugin.name(), plugin.things());
        for message in ways.into_iter().chain(things) {
            self.events.push(Event::Rejected { message });
        }
        self.plugins.plug(plugin);
    }

    /// A command for a plugin: `kind` is its name without the plugin's, and
    /// `body` the rest of its JSON. Refused when no plugin of that name is on
    /// in this world.
    pub fn plugin_command(&mut self, plugin: &str, kind: &str, body: serde_json::Value) {
        let (plugins, lent) = self.lend();
        if !plugins.command(plugin, kind, body, lent) {
            self.events.push(Event::Rejected {
                message: format!("`{plugin}.{kind}`: no plugin `{plugin}` is on in this world"),
            });
        }
        self.tell_mode();
    }

    /// Takes the world's statement of which plugins are on, and says it to
    /// the front end.
    fn spoken(&mut self, spoken: &[protocol::Plugin]) {
        let (plugins, lent) = self.lend();
        let (plugins, apart) = plugins.speak(spoken, lent);
        let lifeless = self.life.hear(spoken);
        self.pets.life(self.life.on());
        self.tend();
        for message in apart.into_iter().chain(lifeless) {
            self.events.push(Event::Rejected { message });
        }
        self.switch_ways(&plugins);
        self.tell_prompt();
        self.state(plugins);
    }

    /// Says to the front end what is on: the plugins, and life while it is.
    fn state(&mut self, mut plugins: Vec<PluginOn>) {
        plugins.extend(self.life.line());
        self.events.push(Event::Statement { plugins });
    }

    /// The plugins on now: their ways of moving and their things are, a
    /// body in a way of one that went off steps off it, and a hand holding
    /// one of its things lets it go.
    fn switch_ways(&mut self, on: &[PluginOn]) {
        self.ways.switch(on);
        self.things.switch(on);
        if let Some(hand) = &self.hand
            && self.things.get(hand).is_none()
        {
            self.hand = None;
        }
        // A clip a plugin asked for ends as the plugin rests.
        self.played = self.played.take().filter(|playing| {
            playing
                .by
                .is_none_or(|by| on.iter().any(|plugin| plugin.name == by))
        });
        if let Some(way) = self.controller.way()
            && self.ways.get(way).is_none()
        {
            self.step_off();
        }
        self.tell_mode();
    }

    /// This client's own body, as an offer is measured from.
    fn standing(&self) -> Standing {
        Standing {
            position: self.controller.position(),
            up: self.controller.up(),
            facing: self.controller.facing(),
        }
    }

    /// The offer the prompt shows now. Nothing is measured while nothing is
    /// offered.
    fn prompt(&self) -> Option<Prompt> {
        if self.offered.is_empty() {
            return None;
        }
        offer::choose(self.offers(), &self.standing())
    }

    /// What is pointed out: while the controls are lent, the holder's
    /// markers alone, so nothing to press lies around a game being played.
    fn markers_shown(&self) -> impl Iterator<Item = (&'static str, &Marker)> {
        let holder = self.controls.holder();
        self.markers
            .every()
            .filter(move |(giver, _)| holder.is_none_or(|holder| holder == *giver))
    }

    /// What is offered to act on: while the controls are lent, the holder's
    /// offers alone, so the key that acts reaches nothing else.
    fn offers(&self) -> impl Iterator<Item = (&'static str, &Offer)> + Clone {
        let holder = self.controls.holder();
        offer::every(&self.offered)
            .filter(move |(plugin, _)| holder.is_none_or(|holder| holder == *plugin))
    }

    /// Says what the prompt shows when the front end was told otherwise: a
    /// turn, a move or a plugin switched off may have changed it.
    fn tell_prompt(&mut self) {
        let prompt = self.prompt();
        if prompt != self.told_prompt {
            self.told_prompt = prompt;
            self.events.push(Event::Prompt { prompt });
        }
    }

    /// Hands an act on an offer to the plugin that offered it.
    fn act(&mut self, plugin: &'static str, id: u64, act: Act) {
        let (plugins, lent) = self.lend();
        if plugins.act(plugin, id, act, lent) {
            self.events.push(Event::Acted { plugin, id, act });
        }
        self.tell_mode();
    }

    /// The key goes down on an offer: a tap at once when it has no hold,
    /// else a press, timed by [`Client::hold_on`] into a tap or a hold.
    fn press(&mut self, prompt: Prompt, by_key: bool, named: bool) {
        match (prompt.hold, prompt.tap) {
            (None, Some(_)) => self.act(prompt.plugin, prompt.id, Act::Tap),
            (None, None) => {}
            (Some(_), _) => self.press = Some(Press::new(prompt, by_key, named)),
        }
    }

    /// The press `dt` seconds on, while one is held: the ring said as it
    /// closes, and the act once it is over. Nothing while nothing is held.
    fn hold_on(&mut self, dt: f64, down: bool) {
        let Some(mut press) = self.press.take() else {
            return;
        };
        let there = press.there(self.offers(), &self.standing());
        let ring = match press.turn(dt, down, there) {
            Pressed::Held(ring) => {
                self.press = Some(press);
                // Shown once it is more than a tap: a tap never flashes it.
                (ring > 0.0).then_some(ring)
            }
            Pressed::Over(act) => {
                // Said whole before it ends, so a front end tells a ring
                // that closed from one let go.
                if act == Some(Act::Hold) {
                    self.tell_ring(Some(1.0));
                }
                if let Some(act) = act {
                    self.act(press.target.plugin, press.target.id, act);
                }
                None
            }
        };
        self.tell_ring(ring);
    }

    fn tell_ring(&mut self, closed: Option<f32>) {
        if closed != self.told_ring {
            self.told_ring = closed;
            self.events.push(Event::Ring { closed });
        }
    }

    /// Where the prompt's point is on the screen, every frame while the
    /// prompt is a point in view, and where the body stands there while it
    /// is seen: a front end hangs the prompt by the thing, clear of the body.
    fn prompt_at(&mut self, camera: &scene::Camera) {
        let Some(prompt) = self.told_prompt.filter(|prompt| prompt.on == Stands::Point) else {
            return;
        };
        let Some(point) = offer::point_of(&self.offered, &prompt) else {
            return;
        };
        let projection = Projection::of(camera, self.aspect);
        let Some((x, y, distance_m)) = projection.project(point) else {
            return;
        };
        let body = self
            .controller
            .sees_own_body()
            .then(|| {
                let feet = self.controller.drawn_position();
                let head = feet + self.controller.body_basis().y_axis * self.figure.label_m();
                let (head_x, top, _) = projection.project(head)?;
                let (feet_x, bottom, _) = projection.project(feet)?;
                // As wide as it is tall times its build, in fractions of the
                // view's width.
                let half = (bottom - top) * BODY_BUILD / self.aspect / 2.0;
                let middle = (head_x + feet_x) / 2.0;
                Some([middle - half, top, middle + half, bottom])
            })
            .flatten();
        self.events.push(Event::PromptAt {
            x,
            y,
            distance_m,
            body,
        });
    }

    /// An act asked at the seam: on the prompt, or on one offer within
    /// reach by its plugin and id; a tap, or with `hold` a hold, timed as
    /// the key's. Refused when nothing answers, or the offer has no such
    /// act.
    fn act_asked(&mut self, plugin: Option<String>, id: Option<u64>, hold: bool) {
        let named = plugin.is_some();
        let found = match (plugin, id) {
            (None, None) => self
                .prompt()
                .ok_or_else(|| "nothing is offered within reach of the body".to_string()),
            (Some(plugin), Some(id)) => offer::within_reach(self.offers(), &self.standing())
                .into_iter()
                .map(|reached| reached.prompt)
                .find(|prompt| prompt.is(&plugin, id))
                .ok_or_else(|| format!("`{plugin}` offers nothing within reach as {id}")),
            _ => Err("`act` names an offer by `plugin` and `id` together, or neither".into()),
        };
        let found = found.and_then(|prompt| match (hold, prompt.tap, prompt.hold) {
            (false, Some(_), _) | (true, _, Some(_)) => Ok(prompt),
            (false, None, _) => Err(format!(
                "`{}` {} is acted on by a hold",
                prompt.plugin, prompt.id
            )),
            (true, _, None) => Err(format!("`{}` {} has no hold", prompt.plugin, prompt.id)),
        });
        match found {
            Ok(prompt) if hold => self.press = Some(Press::new(prompt, false, named)),
            Ok(prompt) => self.act(prompt.plugin, prompt.id, Act::Tap),
            Err(message) => self.events.push(Event::Rejected { message }),
        }
    }

    /// Controls held at the seam by name, while they are lent, until the
    /// next press.
    fn press_asked(&mut self, names: &[String]) {
        if !self.controls_lent() {
            return self.reject_unlent();
        }
        let mut pad = Pad::NONE;
        for name in names {
            match Pad::named(name) {
                Some(control) => pad = pad | control,
                None => {
                    return self.events.push(Event::Rejected {
                        message: format!("`{name}` is no control"),
                    });
                }
            }
        }
        self.controls.command(pad);
    }

    fn reject_unlent(&mut self) {
        self.events.push(Event::Rejected {
            message: "the controls are lent to nobody".into(),
        });
    }

    /// Says who holds the controls when the front end was told otherwise.
    fn tell_controls(&mut self) {
        if let Some(holder) = self.controls.tell() {
            self.events.push(Event::ControlsChanged {
                plugin: holder.map(str::to_owned),
            });
        }
    }

    /// Says how the body moves when the front end was told otherwise: a
    /// key, a command, a plugin or a place may have changed it.
    fn tell_mode(&mut self) {
        if !self.controller.moves_as(&self.told_mode) {
            let mode = self.controller.mode();
            self.told_mode = mode.clone();
            self.events.push(Event::ModeChanged { mode });
        }
    }

    pub fn drain_events(&mut self) -> Vec<Event> {
        core::mem::take(&mut self.events)
    }

    /// Files the client wants. The shell fetches each one and answers with
    /// [`Client::asset_loaded`].
    pub fn drain_asset_requests(&mut self) -> Vec<AssetRequest> {
        for shape in self.parts.wanted() {
            if let Some(name) = shape.asset() {
                self.requests.ask(name, Purpose::Model(shape));
            }
        }
        self.requests.drain()
    }

    /// The answer to an [`AssetRequest`] for a picture: its pixels, or why
    /// there are none. A level that fails is logged and asked for again
    /// when wanted.
    pub fn picture_loaded(&mut self, id: u64, image: Result<scene::Image, String>) {
        let (name, outcome) = match self.requests.answer(id) {
            Some(Purpose::Picture(content)) => {
                (content.hex(), self.pictures.arrived(content, image))
            }
            // A 3D model is drawn whatever its pictures do: one that fails
            // leaves its triangles their corners' colour.
            Some(Purpose::ModelPicture(hash, index)) => {
                let (image, outcome) = match image {
                    Ok(image) => (Some(image), Ok(())),
                    Err(message) => (None, Err(message)),
                };
                self.parts.picture_decoded(hash, index, image);
                (format!("{} of a 3D model", index), outcome)
            }
            _ => return,
        };
        if let Err(message) = outcome {
            self.events.push(Event::Rejected {
                message: format!("picture {name}: {message}"),
            });
        }
    }

    /// The answer to an [`AssetRequest`]: the bytes, or why there are none.
    /// A bad or missing file is logged as a rejection; the box figure and the
    /// rest pose are the fallbacks, so nothing breaks.
    pub fn asset_loaded(&mut self, id: u64, bytes: Result<Vec<u8>, String>) {
        let Some(purpose) = self.requests.answer(id) else {
            return;
        };
        let outcome = match &purpose {
            Purpose::Manifest => bytes.and_then(|bytes| {
                serde_json::from_slice::<Manifest>(&bytes)
                    .map(|manifest| self.adopt(manifest))
                    .map_err(|e| e.to_string())
            }),
            // The wardrobe hears a file that never came too, so wearing it
            // again asks again.
            Purpose::Avatar(path) => {
                let avatar = bytes
                    .and_then(|bytes| avatar::Avatar::from_vrm(&bytes).map_err(|e| e.to_string()));
                self.wardrobe.arrived(path, avatar).map(|_| ())
            }
            Purpose::Clip(gait) => bytes.and_then(|bytes| {
                avatar::Clip::from_glb(&bytes)
                    .map(|clip| {
                        self.clips.insert(*gait, clip);
                    })
                    .map_err(|e| e.to_string())
            }),
            Purpose::Model(shape) => {
                bytes.and_then(|bytes| self.parts.file_arrived(*shape, &bytes, &mut self.requests))
            }
            Purpose::Picture(id) => Err(format!("{}: a picture is answered as pixels", id.hex())),
            Purpose::ModelPicture(..) => Err("a picture is answered as pixels".into()),
        };
        if let Err(message) = outcome {
            self.events.push(Event::Rejected {
                message: format!("{purpose:?}: {message}"),
            });
            if let Purpose::Avatar(path) = &purpose {
                self.fall_back_from(&path.clone());
            }
        }
        self.dress_all();
    }

    /// Asks for the avatar at an asset reference. Every way of choosing an
    /// avatar ends here, whether the reference came from the manifest, the
    /// front end's choice or the account's, as the welcome says it.
    fn wear(&mut self, path: String) {
        self.wanted_avatar = Some(path.clone());
        self.session.wear(&path);
        self.dress_all();
    }

    /// The default avatar the manifest names, if it names one.
    fn default_avatar(&self) -> Option<String> {
        self.manifest
            .as_ref()
            .and_then(|m| m.default_avatar.clone())
    }

    /// A file at `path` is not an avatar: every body that wanted it wears the
    /// default instead. The default itself failing, or there being none,
    /// leaves the box figure.
    fn fall_back_from(&mut self, path: &str) {
        let default = self.default_avatar().filter(|d| d != path);
        if self.wanted_avatar.as_deref() == Some(path) {
            match &default {
                Some(default) => self.wear(default.clone()),
                None => self.wanted_avatar = None,
            }
        }
        for peer in self.peers.iter_mut().filter(|p| p.avatar == path) {
            peer.avatar = default.clone().unwrap_or_default();
        }
    }

    /// Puts every body in what it wants, from what has landed, and asks for
    /// what has not. A body whose file is still on its way stays as it is:
    /// the box figure, or what it wore before. The front end hears of the
    /// player's avatar whenever the choice or its being worn changes.
    fn dress_all(&mut self) {
        let default = self.default_avatar();
        let choose = |wanted: &str| -> Option<String> {
            if wanted.is_empty() {
                default.clone()
            } else {
                Some(wanted.to_owned())
            }
        };
        let wanted = self.wanted_avatar.clone().and_then(|w| choose(&w));
        if let Some(wanted) = &wanted {
            dress(
                &mut self.figure,
                wanted,
                &mut self.wardrobe,
                &mut self.requests,
            );
        }
        for peer in self.peers.iter_mut() {
            if let Some(wanted) = choose(&peer.avatar) {
                dress(
                    &mut peer.figure,
                    &wanted,
                    &mut self.wardrobe,
                    &mut self.requests,
                );
            }
        }
        let worn = self
            .figure
            .mesh()
            .into_iter()
            .chain(self.peers.iter().filter_map(|p| p.figure.mesh()));
        self.wardrobe.prune(worn);
        self.wardrobe.advance(&mut self.requests);
        let worn = wanted.as_deref().is_some_and(|wanted| {
            self.wardrobe
                .get(wanted)
                .is_some_and(|worn| Some(worn.mesh) == self.figure.mesh())
        });
        if self.told_avatar != (wanted.clone(), worn) {
            self.events.push(Event::AvatarChanged {
                path: wanted.clone(),
                worn,
            });
            self.told_avatar = (wanted, worn);
        }
    }

    /// The avatar on offer after the one worn, wrapping around. The first
    /// press holds the whole offer, fetched one by one in the order of
    /// pressing once the next has landed: browsing is one wait, and every
    /// press after lands at once. Nothing while the account's avatar holds.
    fn next_avatar(&mut self) {
        if self.account_avatar.is_some() {
            return;
        }
        let Some(manifest) = &self.manifest else {
            return;
        };
        let offer = manifest.avatars.clone();
        if offer.is_empty() {
            return;
        }
        let current = self.wanted_avatar.as_ref();
        let index = offer.iter().position(|path| Some(path) == current);
        let next = index.map_or(0, |i| (i + 1) % offer.len());
        let after_next = (1..=offer.len()).map(|k| offer[(next + k) % offer.len()].clone());
        self.wardrobe.hold(after_next);
        self.wear(offer[next].clone());
    }

    /// The manifest arrived: fetch the clips it names, and the avatar if one
    /// was waiting for it.
    fn adopt(&mut self, manifest: Manifest) {
        for (gait, path) in &manifest.clips {
            self.requests.ask(path.clone(), Purpose::Clip(*gait));
        }
        self.manifest = Some(manifest);
        if core::mem::take(&mut self.wants_random_avatar) {
            self.random_avatar();
        }
    }

    /// Any avatar on offer, once the manifest says the offer. Nothing while
    /// the account's avatar holds.
    fn random_avatar(&mut self) {
        if self.account_avatar.is_some() {
            return;
        }
        let Some(manifest) = &self.manifest else {
            self.wants_random_avatar = true;
            return;
        };
        if manifest.avatars.is_empty() {
            return;
        }
        let pick = mix(self.entropy) as usize % manifest.avatars.len();
        self.wear(manifest.avatars[pick].clone());
    }

    pub fn drain_skinned_changes(&mut self) -> Vec<SkinnedChange> {
        self.wardrobe.drain_changes()
    }

    /// Pictures to upload or drop, in the order the client produced them:
    /// pictures placed, then the cabinets' screens.
    pub fn drain_picture_changes(&mut self) -> Vec<PictureChange> {
        let mut changes = self.pictures.drain_changes();
        changes.extend(self.carts.drain_changes());
        changes
    }

    /// Every source of sound this frame, for the shell to mix at the eye:
    /// the pictures playing that carry sound, and the clips and the games
    /// running within reach.
    /// Read after [`Client::update`].
    pub fn sounds(&self) -> &[Sound] {
        &self.heard
    }

    /// The voices sustained this frame, within reach of the eye, the
    /// nearest first, at most [`SUSTAINS_AT_ONCE`]: the shell keeps each
    /// playing, by its key, until a frame does not name it. Read after
    /// [`Client::update`].
    pub fn sustains(&self) -> &[Sustain] {
        self.sustains.heard()
    }

    /// The voices struck at a place since the shell last asked, within
    /// reach of the eye, the nearest first: the shell renders each for its
    /// seed and plays it where it was struck. What is not taken by the next
    /// [`Client::update`] is let go.
    pub fn drain_strikes(&mut self) -> Vec<Strike> {
        self.strikes.drain()
    }

    /// Clips for the shell to open and close, in the order the client
    /// decided: a sound placed in the world comes within reach of the eye
    /// or leaves it. The shell plays an open clip where
    /// [`Client::sounds`] says each frame.
    pub fn drain_sounding(&mut self) -> Vec<Sounding> {
        self.speakers.drain()
    }

    /// Pictures that move, for the shell to open and close, in the order the
    /// client decided. The shell answers an open with
    /// [`Client::moving_opened`] and writes every frame into the renderer.
    pub fn drain_moving(&mut self) -> Vec<Moving> {
        self.pictures.drain_moving()
    }

    /// The shell's answer to a [`Moving::Open`]: the size of its frames, or
    /// why it does not play, which is logged; its poster shows meanwhile.
    pub fn moving_opened(&mut self, picture: scene::PictureId, size: Result<(u32, u32), String>) {
        if let Err(message) = self.pictures.opened(picture, size, &mut self.live) {
            self.events.push(Event::Rejected {
                message: format!("picture {picture:?}: {message}"),
            });
        }
    }

    /// Games for the shell to run and stop, in the order the client
    /// decided. The shell answers a run with [`Client::running_opened`],
    /// writes every frame into the renderer, and drops what it plays.
    pub fn drain_running(&mut self) -> Vec<Running> {
        self.carts.drain_running()
    }

    /// The shell's answer to a [`Running::Open`]: the size of the game's
    /// screen, or why it does not run, which is logged, said too of a game
    /// running when the runner it ran in died. The cabinet's glass keeps
    /// what it showed.
    pub fn running_opened(&mut self, picture: scene::PictureId, size: Result<(u32, u32), String>) {
        if let Err(message) = self.carts.opened(picture, size, &mut self.live) {
            self.events.push(Event::Rejected {
                message: format!("cartridge {picture:?}: {message}"),
            });
        }
    }

    /// The shell says the game in `picture` stopped for good, its screen
    /// saying why: whoever plays it is let go.
    pub fn running_stopped(&mut self, picture: scene::PictureId) {
        self.carts.stopped(picture);
    }

    /// The shell read the screen of the game in `picture`, as a
    /// [`Running::Read`] asked: its pixels, an index into `palette` each,
    /// row by row. Said over the seam as [`Event::Screen`].
    pub fn running_read(&mut self, picture: scene::PictureId, pixels: &[u8], palette: &[[u8; 4]]) {
        for read in self.carts.was_read(picture) {
            let said = read.said(pixels, palette);
            self.events.push(said);
        }
    }

    /// The shell opened the link to the world server: the client says hello.
    /// Which world is in the socket's address; who this is was settled by
    /// the key that rode along with it.
    pub fn link_opened(&mut self) {
        self.session.opened(
            self.wanted_avatar.as_deref().unwrap_or(""),
            &self.wanted_name,
        );
        self.events.push(Event::Session {
            status: SessionStatus::Connecting,
            session: None,
            level: self.level,
        });
    }

    /// The link is gone, from either side.
    pub fn link_closed(&mut self) {
        self.session.closed();
        self.went_offline();
    }

    /// One frame from the server.
    pub fn receive(&mut self, frame: &[u8]) {
        use protocol::server_message::Message;
        let message = match protocol::decode::<protocol::ServerMessage>(frame) {
            Ok(message) => message,
            Err(error) => {
                self.events.push(Event::Rejected {
                    message: format!("frame from the server: {error}"),
                });
                return;
            }
        };
        self.session.heard();
        match message.message {
            Some(Message::Welcome(welcome)) => {
                if !self.same_world(welcome.recipe.as_ref()) {
                    self.events.push(Event::Rejected {
                        message: "the server holds another recipe for this world".into(),
                    });
                    self.session.close();
                    self.went_offline();
                    return;
                }
                self.session.welcomed(welcome.session);
                // The body wears the account's avatar. The client is the one
                // place that decides what a body wears, so it takes it from
                // the welcome itself: every front end and every agent is
                // dressed by the same rule. Through `wear`, so the others hear
                // it as any change; a page that read it on entering started
                // the body in it, and then nothing goes out.
                self.account_avatar = Some(welcome.avatar.clone()).filter(|a| !a.is_empty());
                if let Some(avatar) = self.account_avatar.clone()
                    && self.wanted_avatar.as_ref() != Some(&avatar)
                {
                    self.wear(avatar);
                }
                // The world's clock is the server's from here on.
                self.set_clock(welcome.clock_ms as f64 / 1000.0);
                self.level = Some(Level::from_wire(welcome.level));
                // The world keeps the cells from here on.
                self.cells.link(welcome.session);
                self.parts.link(welcome.session, !welcome.visitor);
                self.pets.link(welcome.session, !welcome.visitor);
                self.history.clear();
                for peer in &welcome.peers {
                    self.peers.add(peer, &self.generator);
                }
                self.events.push(Event::Session {
                    status: SessionStatus::Online,
                    session: Some(welcome.session),
                    level: self.level,
                });
                self.spoken(&welcome.plugins);
                self.peers_changed();
            }
            Some(Message::Joined(joined)) => {
                if let Some(peer) = &joined.peer {
                    self.peers.add(peer, &self.generator);
                    self.peers_changed();
                }
            }
            Some(Message::Left(left)) => {
                self.peers.remove(left.session);
                self.pets.left(left.session);
                self.peers_changed();
            }
            Some(Message::Stances(stances)) => {
                let own = match self.session.link() {
                    session::Link::Online { session } => session,
                    _ => 0,
                };
                for moved in &stances.moved {
                    if let Some(stance) = &moved.stance
                        && moved.session != own
                    {
                        self.peers.moved(moved.session, stance, &self.generator);
                    }
                }
            }
            Some(Message::Wearing(wearing)) => {
                self.peers.wearing(wearing.session, &wearing.avatar);
                self.dress_all();
            }
            Some(Message::Renamed(renamed)) => {
                self.peers.renamed(renamed.session, &renamed.name);
                self.peers_changed();
            }
            Some(Message::Sounded(sounded)) => self.sounded(&sounded),
            // The cells are the core's, and their events come under their
            // name as a plugin's do under its own.
            Some(Message::Envelope(envelope)) if envelope.plugin == protocol::cells::OWNER => {
                self.cells
                    .receive(&self.generator, &envelope.kind, &envelope.payload);
            }
            Some(Message::Envelope(envelope)) if envelope.plugin == protocol::parts::OWNER => {
                self.parts
                    .receive(&self.generator, &envelope.kind, &envelope.payload);
            }
            Some(Message::Envelope(envelope)) if envelope.plugin == protocol::life::OWNER => {
                self.pets.receive(&envelope.kind, &envelope.payload);
            }
            Some(Message::Envelope(envelope)) => {
                let (plugins, lent) = self.lend();
                plugins.receive(&envelope, lent);
            }
            Some(Message::Answer(answer)) => {
                if !answer.code.is_empty() {
                    self.history.refused(answer.id);
                }
                let cells = self
                    .cells
                    .answered(&self.generator, answer.id, &answer.code);
                let parts = !cells && self.parts.answered(answer.id, &answer.code);
                let pet = self.pets.answered(answer.id, &answer.code);
                if let Some(kind) = pet.filter(|_| !answer.code.is_empty()) {
                    self.events.push(Event::Rejected {
                        message: format!("the world refused {kind} of a pet: {}", answer.code),
                    });
                }
                if (cells || parts) && !answer.code.is_empty() {
                    let what = if cells { "cells" } else { "parts" };
                    self.events.push(Event::Rejected {
                        message: format!(
                            "the world refused a change to the {what}: {}",
                            answer.code
                        ),
                    });
                }
            }
            Some(Message::Plugins(plugins)) => self.spoken(&plugins.plugins),
            Some(Message::Refused(refused)) => {
                self.events.push(Event::Rejected {
                    message: format!("refused by the server: {}", refused.reason),
                });
                self.session.closed();
                self.went_offline();
            }
            None => {}
        }
    }

    /// Whether the server's recipe is the one this client stands in. A page
    /// sets the recipe from the same row the server reads, so a difference
    /// is a stale page, and peers placed by another recipe would stand on
    /// the wrong ground.
    fn same_world(&self, recipe: Option<&protocol::Recipe>) -> bool {
        let Some(recipe) = recipe else {
            return false;
        };
        let own = self.recipe();
        let params = if recipe.params_json.trim().is_empty() {
            Ok(Params::default())
        } else {
            serde_json::from_str::<Params>(&recipe.params_json)
        };
        recipe.seed == worldgen::format_seed(own.seed)
            && recipe.generator_version == own.generator_version
            && params.is_ok_and(|params| params == own.params)
    }

    fn peers_changed(&mut self) {
        self.dress_all();
        self.events.push(Event::Peers {
            peers: self.peers.infos(),
        });
    }

    fn went_offline(&mut self) {
        // What the world kept went with it.
        self.cells.unlink();
        self.parts.unlink();
        self.history.clear();
        self.peers.clear();
        self.dress_all();
        self.events.push(Event::Session {
            status: SessionStatus::Offline,
            session: None,
            level: self.level,
        });
        let (plugins, lent) = self.lend();
        let plugins = plugins.hush(lent);
        self.life.hush();
        self.pets.unlink();
        self.tend();
        self.switch_ways(&plugins);
        self.state(plugins);
        self.events.push(Event::Peers { peers: Vec::new() });
    }

    /// What the shell sends on the link, in order. Empty while offline.
    pub fn drain_outbound(&mut self) -> Vec<Outbound> {
        for ask in self.cells.drain_asks() {
            self.session
                .ask(protocol::cells::OWNER, ask.kind, ask.payload, ask.id);
        }
        for ask in self.parts.drain_asks() {
            self.session
                .ask(protocol::parts::OWNER, ask.kind, ask.payload, ask.id);
        }
        for ask in self.pets.drain_asks() {
            self.session
                .ask(protocol::life::OWNER, ask.kind, ask.payload, ask.id);
        }
        self.session.drain_outbound()
    }

    /// Where this body is and how it moves, for the wire.
    fn stance(&self) -> protocol::Stance {
        let controller = &self.controller;
        let grid = self.generator.sphere().blocks();
        let point = if controller.on_moon() {
            grid.surface_point(controller.radial().to_array())
        } else {
            controller.point()
        };
        let facing = controller.facing();
        // What peers play is what this body plays: one motion for both.
        let motion = self.motion();
        let body = if controller.on_moon() {
            protocol::Body::Moon
        } else {
            protocol::Body::Planet
        };
        protocol::Stance {
            body: body.into(),
            sector: point.sector.index() as u32,
            u: point.u as f32,
            v: point.v as f32,
            height_m: controller.height_m() as f32,
            facing_x: facing.x as f32,
            facing_y: facing.y as f32,
            facing_z: facing.z as f32,
            gait: motion.gait.wire().into(),
            speed_mps: motion.speed_mps as f32,
            sprint: motion.sprint,
            way: controller.way().unwrap_or_default().to_owned(),
            carrier: controller.carrier(),
            hand: self.hand.clone().unwrap_or_default(),
        }
    }

    /// Volume mesh uploads and removals since the last call, in order.
    pub fn drain_volume_changes(&mut self) -> Vec<VolumeChange> {
        self.cells.drain_changes()
    }

    /// The meshes of models to upload and drop, in order.
    pub fn drain_model_changes(&mut self) -> Vec<ModelChange> {
        self.parts.drain_changes()
    }

    /// The parts of the world, to read: what stands where.
    pub fn parts(&self) -> &Parts {
        &self.parts
    }

    /// The ground of the world, to read.
    pub fn generator(&self) -> &Generator {
        &self.generator
    }

    /// What a hand changed, to read: whether there is a change to take
    /// back, and one to put back.
    pub fn history(&self) -> &History {
        &self.history
    }

    /// Takes back the last change a hand made here, whatever owner it was
    /// of: the owner takes its own back, here or in the world.
    pub fn take_back(&mut self) {
        match self.history.take_back() {
            Some(Owner::Cells) => self.cells.take_back(&self.generator),
            Some(Owner::Parts) => self.parts.restore(&self.generator, true),
            None => {}
        }
    }

    /// Puts back the last change taken back.
    pub fn put_back(&mut self) {
        match self.history.put_back() {
            Some(Owner::Cells) => self.cells.put_back(&self.generator),
            Some(Owner::Parts) => self.parts.restore(&self.generator, false),
            None => {}
        }
    }

    pub fn drain_terrain_changes(&mut self) -> Vec<TerrainChange> {
        let mut changes = self.terrain.drain_changes();
        changes.extend(self.moon_terrain.drain_changes());
        changes
    }

    /// Advances the simulation by `dt` seconds and returns the frame to draw.
    pub fn update(&mut self, dt: f64, input: &mut Input) -> Frame {
        self.strikes.begin_frame();
        self.sustains.begin_frame();
        let taken = input.take_frame();
        let (look, zoom) = (taken.look, taken.zoom);
        self.entropy = self.entropy.wrapping_add(dt.to_bits()).rotate_left(7);
        let struck = [Key::Forward, Key::Back].map(|key| taken.pressed.contains(&key));
        // The shell letting every key go is the person gone: a hidden tab
        // presses nothing.
        if taken.interrupted {
            self.away();
        }
        let lent = self.controls_lent();
        let tapped = Pad::tapped(&taken.pressed);
        for key in taken.pressed {
            // While the controls are lent the keys are theirs: the key that
            // acts reaches the holder's offers, and one key leaves.
            if lent && !matches!(key, Key::Act | Key::Leave) {
                continue;
            }
            match key {
                // The key leaves a way of moving first, on foot.
                Key::ToggleMode => self.toggle_mode(),
                Key::NewSeed => {
                    // A new seed over the same ground: a field world keeps its
                    // coastlines and gets new mountains under them.
                    let recipe = Recipe {
                        params: self.recipe().params,
                        ..Recipe::new(mix(self.recipe().seed ^ self.entropy))
                    };
                    self.command(Command::SetRecipe { recipe });
                }
                Key::NextAvatar => self.next_avatar(),
                Key::FirstPerson => self.controller.toggle_first_person(),
                Key::Undo => self.take_back(),
                Key::Redo => self.put_back(),
                // A key with nothing offered does nothing: no refusal.
                Key::Act => {
                    if let Some(prompt) = self.prompt() {
                        self.press(prompt, true, false);
                    }
                }
                Key::Whistle => self.command(Command::Sound {
                    voice: voices::WHISTLE.name.to_owned(),
                }),
                Key::Leave => self.controls.take_back(),
                _ => {}
            }
        }
        // A plugin hears the keys it asked for by name.
        let (plugins, lent) = self.lend();
        plugins.keys(&taken.codes, lent);

        // The clock takes every second, a stalled tab's too, so it keeps
        // pace with the server's. Long frames must not tunnel the avatar
        // anywhere: the body takes a tenth at most.
        self.clock_s += dt.max(0.0);
        let elapsed_s = dt.max(0.0);
        let dt = dt.clamp(0.0, 0.1);
        let wish = Wish {
            movement: input.movement(),
            up: input.held(Key::Up),
            down: input.held(Key::Down),
            sprint: input.sprinting(),
            struck,
            reach: input.reach(),
            look,
            zoom,
        };
        // A clip asked for plays its time, or until the body moves when
        // moving ends it, while the body moves as it did when asked and no
        // passage takes it.
        let moved = wish.movement != [0.0, 0.0] || wish.up || wish.down;
        let (now_s, clips, controller) = (self.clock_s, &self.clips, &self.controller);
        let passing = self.passage.is_some();
        self.played = self.played.take().filter(|playing| {
            !passing && controller.moves_as(&playing.mode) && playing.plays(now_s, clips, moved)
        });
        // The moon has a gravity field of its own: fly into it and it becomes
        // down; turn flight off there and you fall to it, walk and jump high.
        let moon = self.moon_position();
        self.controller.set_moon(controller::MoonBody {
            center: moon,
            radius_m: MOON_RADIUS_M,
        });
        let streamer = if self.controller.on_moon() {
            &self.moon_terrain
        } else {
            &self.terrain
        };
        let footprint_m = streamer.drawn_footprint_m(self.controller.radial());
        self.controller.set_drawn_footprint(footprint_m);
        self.cells.orbit(moon);
        self.parts.orbit(moon);
        // A body in a passage is the passage's: it stands where it fell in,
        // and never walks out the far side of a ring it crossed. A body whose
        // controls are lent stands as it does there.
        if self.passage.is_none() && !self.controls_lent() {
            let (seat, point) = self.controller.over(&self.generator);
            let flora = self.flora.as_ref().filter(|_| self.living);
            self.trunks.around(&self.generator, flora, seat, point);
            self.controller.update(
                dt,
                wish,
                &self.generator,
                Built {
                    cells: &self.cells,
                    parts: &self.parts,
                    trunks: &self.trunks,
                },
            );
        }
        self.passage_turn();
        // Nothing holds a body a passage takes.
        if self.passage.is_some() {
            self.controls.take_back();
        }
        let motion = self.motion();
        let held = self
            .controller
            .way()
            .and_then(|way| self.ways.get(way))
            .map(|(_, way)| &way.held);
        self.figure.update(dt, &motion, &self.clips, held);
        let hand = self.hand.as_deref().and_then(|hand| self.things.get(hand));
        let arm = hand.map(|(_, thing)| thing.arm.as_slice());
        self.figure.grasp(dt, arm);
        self.peers
            .update(dt, moon, &self.clips, &self.ways, &self.things);
        // A body leapt off what it rode is heard landing on its feet, this
        // one's and each peer's as this client draws it.
        let peers = self.peers.iter().filter_map(|peer| peer.figure.alighted());
        for at in self.figure.alighted().into_iter().chain(peers) {
            self.strikes
                .strike(&voices::ALIGHT, Group::Steps, at, voices::ALIGHT_REACH_M);
        }
        self.animals_turn(dt, moon);
        if self.session.tick(dt) {
            self.events.push(Event::Rejected {
                message: "the server went silent".into(),
            });
            self.session.close();
            self.went_offline();
        } else if self.session.stance_due() {
            let stance = self.stance();
            self.session.stance(stance);
        }
        self.stats(dt);

        self.controls.ease(dt);
        let camera = self.controls.eye(self.controller.camera(&self.generator));
        self.anchors(&camera);
        self.labels(&camera);
        let eye = Eye {
            position: camera.position,
            rotation: camera.rotation,
            fov_y: f64::from(camera.fov_y),
            aspect: f64::from(self.aspect),
            clear: self.clear(),
            // A captured pointer aims through the middle of the view.
            pointer: input.pointer.unwrap_or([0.5, 0.5]).map(f64::from),
        };
        // Each plugin's turn, then the cells drawn as the turns left them,
        // and kept in step with what the world holds near the body.
        let pad = match self.controls_lent() {
            true => self.controls.pad(input, tapped),
            false => Pad::NONE,
        };
        let (plugins, lent) = self.lend();
        plugins.turn(&eye, input, pad, (taken.interrupted, taken.inspected), lent);
        self.tell_mode();
        self.tell_controls();
        self.tell_prompt();
        self.hold_on(dt, input.held(Key::Act) && !taken.interrupted);
        self.prompt_at(&camera);
        self.tell_markers(&camera);
        // Heard last, once everything that may strike in a frame has.
        self.strikes.hear(camera.position);
        self.sustains.hear(camera.position);
        self.say_choices();
        self.cells.update(eye.position);
        let through = self.through(&camera);
        // The cells near the star a portal leads to are looked at too, so
        // what is built there shows through the pane.
        let eye = through.as_ref().map(|seen| self.body_of(seen.star));
        self.cells
            .look(&self.generator, self.controller.held_by(), eye, dt);
        self.parts
            .look(&self.generator, self.controller.held_by(), dt);
        self.ease_seen(dt, through.as_ref().map(|seen| seen.portal));
        let (patches, seen) = self.stream(&camera, through.as_ref(), Some(Budget::frame()));
        // Said on the way in, never while it holds: a front end lifts its
        // veil on it, and hears it again after a leap or a new recipe.
        let settled = self.terrain.settled()
            && self.moon_terrain.settled()
            && self.cells.settled()
            && !self.plugins.busy();
        if settled && !self.settled {
            self.events.push(Event::Settled);
        }
        self.settled = settled;
        // What the game played here was told, before the games' frame
        // closes: its controls are drawn as it holds them.
        let played = self.carts.pads_played();
        // A game waits as long on a slow screen as on a fast one, and the
        // time a hidden page drew nothing counts at its first frame back.
        let pictures = self.pictures_drawn(&camera, elapsed_s);
        self.waiting(&camera);
        self.frame(camera, patches, pictures, through.zip(seen), played)
    }

    /// Says where the body is born once the first chart in a world answers,
    /// and a visitor's choices whenever they change, for its front end to
    /// keep.
    fn say_choices(&mut self) {
        if let Some(home) = self.parts.take_home() {
            let place = home
                .map(|(seat, part)| self.host(protocol::parts::OWNER).place_at(seat, &part.pose));
            self.events.push(Event::Home { place });
        }
        if let Some(choices) = self.parts.take_rechosen() {
            self.events.push(Event::Choices {
                home: choices.home,
                favourites: choices.favourites.into_iter().collect(),
            });
        }
    }

    /// Where every head in view lands on the screen, own body first unless
    /// the camera is in it, with the renderer's own projection: reversed
    /// infinite depth has no far plane, so only what is behind the camera or
    /// off the sides is left out.
    fn anchors(&mut self, camera: &scene::Camera) {
        let own = match self.session.link() {
            session::Link::Online { session } => Some(session),
            _ => None,
        };
        let moon = self.moon_position();
        // The own head is the eye in first person: no tag hangs over it, nor
        // while the eye is lent away from the body, which is not drawn.
        let own_head = own
            .filter(|_| self.controller.sees_own_body() && !self.controls.eye_away())
            .map(|session| {
                let up = self.controller.body_basis().y_axis;
                (
                    session,
                    self.controller.drawn_position() + up * self.figure.label_m(),
                )
            });
        let projection = Projection::of(camera, self.aspect);
        let anchors: Vec<Anchor> = own_head
            .into_iter()
            .chain(
                own.is_some()
                    .then(|| self.peers.heads(moon, &self.ways))
                    .into_iter()
                    .flatten(),
            )
            .filter_map(|(session, head)| {
                let (x, y, distance_m) = projection.project(head)?;
                Some(Anchor {
                    session,
                    x,
                    y,
                    distance_m,
                })
            })
            .collect();
        if anchors.is_empty() && !self.had_anchors {
            return;
        }
        self.had_anchors = !anchors.is_empty();
        self.events.push(Event::Anchors { anchors });
    }

    /// Where every landmark and portal in view and within reach of a label
    /// lands on the screen, the point over its top, as heads do: a star
    /// bears its own name, a ring the name of the star it leads to. A ring
    /// whose star is not known here asks the chart.
    fn labels(&mut self, camera: &scene::Camera) {
        let projection = Projection::of(camera, self.aspect);
        let body = self.controller.held_by();
        let mut unknown = false;
        let mut labels: Vec<(Label, DVec3)> = self
            .parts
            .all()
            .filter_map(|(id, seat, part)| {
                if !matches!(
                    part.shape,
                    Shape::Landmark(_) | Shape::Portal(Portal { to: Some(_), .. })
                ) {
                    return None;
                }
                if parts::away_m(&self.generator, seat, &part.pose, body) > LABEL_M {
                    return None;
                }
                let name = match part.shape {
                    Shape::Portal(Portal { to: Some(to), .. }) => match self.parts.landmark(to) {
                        Some((
                            _,
                            Part {
                                shape: Shape::Landmark(name),
                                ..
                            },
                        )) => name,
                        _ => {
                            unknown = true;
                            return None;
                        }
                    },
                    Shape::Landmark(name) => name,
                    _ => return None,
                };
                let world = self.parts.transform(&self.generator, id)?;
                let top = world.transform_point3(DVec3::new(0.0, 0.0, 1.0));
                let (x, y, distance_m) = projection.project(top)?;
                let label = Label {
                    id: id.0,
                    text: name.to_string(),
                    x,
                    y,
                    distance_m,
                };
                Some((label, top))
            })
            .collect();
        // A pet bears its name over its head.
        for near in self.animals.near() {
            let Some(name) = self.wears_name(near) else {
                continue;
            };
            let over = over_animal(near);
            if let Some((x, y, distance_m)) = projection.project(over) {
                let label = Label {
                    id: near.number,
                    text: name.to_owned(),
                    x,
                    y,
                    distance_m,
                };
                labels.push((label, over));
            }
        }
        if unknown {
            self.parts.chart_for(self.clock_s);
        }
        self.labelled.clear();
        self.labelled.extend(labels.iter().map(|&(_, over)| over));
        let labels: Vec<Label> = labels.into_iter().map(|(label, _)| label).collect();
        if labels.is_empty() && !self.had_labels {
            return;
        }
        self.had_labels = !labels.is_empty();
        self.events.push(Event::Labels { labels });
    }

    /// Where every cabinet in view whose game waits for its player is on
    /// the screen, the middle of its screen, and how long it still waits.
    fn waiting(&mut self, camera: &scene::Camera) {
        let projection = Projection::of(camera, self.aspect);
        let waiting: Vec<Waiting> = self
            .carts
            .waiting()
            .filter_map(|(id, at, remains_s, for_s)| {
                let (x, y, distance_m) = projection.project(at)?;
                Some(Waiting {
                    id,
                    x,
                    y,
                    distance_m,
                    remains: (remains_s / for_s) as f32,
                    remains_s: remains_s as f32,
                })
            })
            .collect();
        if waiting.is_empty() && !self.had_waiting {
            return;
        }
        self.had_waiting = !waiting.is_empty();
        self.events.push(Event::Waiting { waiting });
    }

    /// The person went away from the world, the page hidden or its window
    /// left: the controls come back, and every game played is left at
    /// once, waiting for them [`GAME_WAITS_AWAY_S`]. A shell calls it as it
    /// happens, and drains what the games run at once: a hidden page may
    /// draw no frame for long.
    pub fn away(&mut self) {
        self.controls.take_back();
        let parts = &self.parts;
        self.carts
            .away(&mut self.live, |key| carts::runs(parts, key));
    }

    /// The frame as it would look once streaming caught up. Blocks until every
    /// patch is built: for headless renders only.
    pub fn settled_frame(&mut self) -> Frame {
        // The moon is where the clock says now, for whoever stands on it and
        // for what is built there, as for its ground.
        let moon = self.moon_position();
        self.controller.set_moon(controller::MoonBody {
            center: moon,
            radius_m: MOON_RADIUS_M,
        });
        self.cells.orbit(moon);
        self.parts.orbit(moon);
        let eye = self.controller.camera(&self.generator).position;
        let (plugins, lent) = self.lend();
        plugins.settle(lent);
        self.cells.settle(eye);
        // As the person sees it: a holder of the controls holds the eye.
        let camera = self.controls.eye(self.controller.camera(&self.generator));
        let through = self.through(&camera);
        self.ease_seen(f64::INFINITY, through.as_ref().map(|seen| seen.portal));
        let (patches, seen) = self.stream(&camera, through.as_ref(), None);
        let played = self.carts.pads_played();
        let pictures = self.pictures_drawn(&camera, 0.0);
        self.frame(camera, patches, pictures, through.zip(seen), played)
    }

    /// Eases how much each portal's pane shows the world seen through it:
    /// toward 1 for the portal seen through this frame, toward 0 for every
    /// other, a step of `dt` over [`SEEN_S`]. A portal no longer held is
    /// forgotten.
    fn ease_seen(&mut self, dt: f64, through: Option<PartId>) {
        let step = (dt / SEEN_S).min(1.0) as f32;
        let portals: Vec<PartId> = self
            .parts
            .all()
            .filter(|(_, _, part)| matches!(part.shape, Shape::Portal(_)))
            .map(|(id, _, _)| id)
            .collect();
        self.seen.retain(|id, _| portals.contains(id));
        for id in portals {
            let target = f32::from(u8::from(through == Some(id)));
            let amount = self.seen.entry(id).or_insert(0.0);
            *amount += (target - *amount).clamp(-step, step);
        }
    }

    /// The thing this body's hand grasps, while its plugin is on.
    fn grasped_thing(&self) -> Option<&Thing> {
        let hand = self.hand.as_deref()?;
        self.things.get(hand).map(|(_, thing)| thing)
    }

    /// The body's motion as the figure shows it, in its gait.
    fn motion(&self) -> Motion {
        Motion::of(&self.controller, self.gait())
    }

    /// Where a figure riding an animal is drawn: its feet hung under the
    /// saddle of the animal under it, by the hips of the pose it holds, its
    /// seat on the saddle, turned as the saddle is. By who rides: `None`
    /// for this client's body, a peer by its session. None for one on its
    /// own feet, or on an animal not drawn here.
    fn seat(&self, figure: &Figure, who: Option<u32>, carrier: u64) -> Option<DAffine3> {
        if carrier == 0 {
            return None;
        }
        Some(figure.seat(self.animals.saddle(who)?))
    }

    /// The animals' turn in a frame, after the bodies moved: those ridden
    /// step under their riders, the free herds near the body where the
    /// world's clock has them, and the eye rises into the saddle.
    fn animals_turn(&mut self, dt: f64, moon: DVec3) {
        if !self.life.on() {
            // A world that lost its life drops whoever rode on its ground.
            if self.controller.carrier() != 0 {
                self.step_off();
            }
            self.animals.clear();
            self.controller.ride_at(0.0, dt);
            self.controller.bank_at(0.0, dt);
            return;
        }
        let own = (self.controller.carrier() != 0).then(|| Rider {
            who: None,
            number: self.controller.carrier(),
            at: self.controller.drawn_position(),
            facing: self.controller.facing(),
            speed_mps: self.controller.ground_mps(),
            balk_s: self.controller.balked_s(),
            gathering: self.controller.gathering(),
            aloft: Some(!self.controller.grounded()),
            // As a peer reads it from the stance.
            wings: self.gait() == Gait::Fly,
        });
        let peers = self
            .peers
            .in_ways(moon, &self.ways)
            .filter(|(.., carrier, _)| *carrier != 0)
            .map(|(session, .., carrier, motion)| Rider {
                who: Some(session),
                number: carrier,
                at: motion.position,
                facing: -motion.basis.z_axis,
                speed_mps: motion.speed_mps,
                balk_s: None,
                gathering: None,
                aloft: None,
                wings: motion.gait == Gait::Fly,
            });
        let riders: Vec<Rider> = own.into_iter().chain(peers).collect();
        if self.pets.take_changed() {
            self.animals.set_pets(self.pets.all(), &self.generator);
        }
        if let Some(pet) = self.pets.resaid() {
            self.events.push(Event::Pet { pet });
        }
        let near = (!self.controller.on_moon()).then(|| self.controller.point());
        let built = Built {
            cells: &self.cells,
            parts: &self.parts,
            trunks: &self.trunks,
        };
        // Bodies on foot a horse left waits for: this one, and each
        // peer's.
        let own = (self.controller.carrier() == 0).then(|| self.controller.drawn_position());
        let on_foot: Vec<DVec3> = own
            .into_iter()
            .chain(self.peers.on_foot(moon, &self.ways))
            .collect();
        let bodies = animals::Bodies {
            riders: &riders,
            on_foot: &on_foot,
        };
        self.animals
            .update(dt, self.clock_s, near, bodies, &self.generator, built);
        // Each rider's legs go round the back of what it rides.
        let (straddle, saddle) = (self.animals.straddle(None), self.animals.saddle(None));
        self.figure.straddle(straddle, saddle);
        for peer in self.peers.iter_mut() {
            let who = Some(peer.info.session);
            let (straddle, saddle) = (self.animals.straddle(who), self.animals.saddle(who));
            peer.figure.straddle(straddle, saddle);
        }
        // Stepped down from its pet, the body says where it stands.
        if let Some((number, at, heading)) = self.animals.stepped_off()
            && self.pets.mine().is_some_and(|pet| pet.number == number)
        {
            let spot = animals::spot_of(at, heading, self.clock_ms(), &self.generator);
            self.pets.leave(spot);
        }
        self.chase_pet();
        // Each beat of a gait heard sounds under the animal, by the ground
        // there, as hard as its gait: the one ridden carries further, a
        // free herd is quieter.
        for &(fall, ridden) in self.animals.struck() {
            if let Some(voice) = voices::hoof(fall.surface, fall.force) {
                let (ridden_m, free_m) = voices::HOOF_REACH_M;
                let reach_m = if ridden { ridden_m } else { free_m };
                self.strikes.strike(&voice, Group::Steps, fall.at, reach_m);
            }
        }
        for &at in self.animals.wingbeats() {
            let reach_m = voices::WINGBEAT_REACH_M;
            self.strikes
                .strike(&voices::WINGBEAT, Group::Steps, at, reach_m);
        }
        // A horse ridden here drives off the ground as it springs into a
        // jump, heard by its rider and sent for peers to strike under it.
        for &(who, at) in self.animals.sprang() {
            let Some(animal) = self.animals.under(who) else {
                continue;
            };
            let spring = voices::leap(&voices::SPRING, animal.traits(), animal.leap_force());
            let seed = self
                .strikes
                .strike(&spring, Group::Steps, at, voices::LEAP_REACH_M);
            if who.is_none() {
                self.session.sound(voices::SPRING.name, seed);
            }
        }
        // Every horse ridden lands as this client draws it, a peer's too: from
        // a jump as hard as its pace, set down from the wing as hard as it
        // comes down.
        for &(who, landing) in self.animals.landed() {
            let Some(animal) = self.animals.under(who) else {
                continue;
            };
            let sound = match landing.winged {
                false => Some(voices::leap(
                    &voices::LANDING,
                    animal.traits(),
                    animal.leap_force(),
                )),
                true => voices::touchdown(animal.traits(), &landing),
            };
            if let Some(sound) = sound {
                (self.strikes).strike(&sound, Group::Steps, landing.at, voices::LEAP_REACH_M);
            }
        }
        // Wings held out to glide sound the wind past them, as long as they
        // are, one voice a rider.
        for (who, glide) in self.animals.glides() {
            let (level, tone) = voices::wind(glide);
            let rider = who.map_or(0, |session| u64::from(session) + 1);
            self.sustains.sustain(Sustain {
                key: voices::WIND_KEY ^ rider.wrapping_mul(0x9E37_79B9_7F4A_7C15),
                voice: voices::WIND.clone(),
                group: Group::Steps,
                position: glide.at,
                reach_m: voices::WIND_REACH_M,
                level,
                tone,
            });
        }
        let riding_m = self
            .seat(&self.figure, None, self.controller.carrier())
            .map_or(0.0, |seat| {
                let up = self.controller.up();
                (seat.translation - self.controller.drawn_position()).dot(up)
            });
        self.controller.ride_at(riding_m, dt);
        self.controller.bank_at(self.animals.bank(None), dt);
    }

    /// How the body moves as it is drawn and sent: falling while it is in a
    /// passage, playing a clip asked for while it plays, otherwise as it
    /// moves.
    fn gait(&self) -> Gait {
        // On a mount the gait says to every eye whether it flies, its wings
        // out: aloft it flies whatever clip plays, and a clip never makes it.
        let mounted = self.controller.carrier() != 0;
        match &self.played {
            _ if self.passage.is_some() => Gait::Fall,
            _ if self.controller.aloft() => Gait::Fly,
            Some(playing) if !(mounted && playing.clip() == Gait::Fly) => playing.clip(),
            _ => Gait::of(&self.controller),
        }
    }

    /// A passage's turn. The body falls in for [`WARP_IN_S`] and is held
    /// there until the place is known; it goes through, and is held again
    /// until the ground there has come, [`HELD_S`] at most each; then the
    /// picture bursts out for [`WARP_OUT_S`]. A place that cannot be
    /// reached, or never said, lets the body out where it fell in.
    fn passage_turn(&mut self) {
        let Some(passage) = &mut self.passage else {
            return;
        };
        let now = self.clock_s;
        match passage.stage {
            Stage::In => {
                let fallen_s = now - passage.started_s - WARP_IN_S;
                if fallen_s < 0.0 {
                    return;
                }
                match passage.place.clone() {
                    Some(place) => match self.go_to(&place) {
                        Ok(()) => passage_stage(&mut self.passage, Stage::Through(now)),
                        Err(message) => {
                            self.events.push(Event::Rejected { message });
                            passage_stage(&mut self.passage, Stage::Out(now));
                        }
                    },
                    None if fallen_s >= HELD_S => passage.stage = Stage::Out(now),
                    None => {}
                }
            }
            // The ground at the place, streamed after the leap: the frame of
            // the leap is never settled there, so one frame at least.
            Stage::Through(at) => {
                if (now > at && self.settled) || now - at >= HELD_S {
                    passage.stage = Stage::Out(now);
                }
            }
            Stage::Out(at) => {
                if now - at >= WARP_OUT_S {
                    self.passage = None;
                }
            }
        }
    }

    /// How charged the body is by its passage, 0 to 1: white as it goes
    /// through, its own colour again as it lands.
    fn charge(&self) -> f32 {
        self.passage
            .as_ref()
            .map_or(0.0, |passage| passage.depth(self.clock_s) as f32)
    }

    /// The warp of the picture this frame: into the portal the body is
    /// falling into, or into the body travelling with none, and out of the
    /// body where it landed. The waist is one point all the way for a
    /// travel, so the hole never hops as the body goes through, and the
    /// body sits in it; the middle of the view, tried, hops down to it.
    fn warp(&self, camera: &scene::Camera) -> Option<scene::Warp> {
        let passage = self.passage.as_ref()?;
        let projection = Projection::of(camera, self.aspect);
        let on_screen = |point: DVec3| {
            projection
                .project(point)
                .map_or([0.5, 0.5], |(x, y, _)| [x, y])
        };
        let waist = || on_screen(self.controller.drawn_position() + self.controller.up() * 0.9);
        let depth = passage.depth(self.clock_s) as f32;
        Some(match passage.stage {
            Stage::In => {
                let pane = passage.portal.and_then(|portal| {
                    self.parts
                        .transform(&self.generator, portal)
                        .map(|ring| ring.transform_point3(DVec3::new(0.0, 0.0, 0.5)))
                });
                scene::Warp {
                    centre: pane.map_or_else(waist, on_screen),
                    phase: depth - 1.0,
                }
            }
            // Held in, past the portal: the picture stays drawn into the body.
            Stage::Through(_) => scene::Warp {
                centre: waist(),
                phase: 0.0,
            },
            Stage::Out(_) => scene::Warp {
                centre: waist(),
                phase: 1.0 - depth,
            },
        })
    }

    /// The portal the view is seen through this frame, and the lens past it:
    /// the nearest portal within [`THROUGH_M`] that leads to a landmark
    /// whose place is known here, and a wide lens carried from the pane's
    /// middle to the landmark, lifted over it, looking the way the eye looks
    /// through the pane. A landmark this client does not hold is asked of
    /// the chart.
    fn through(&mut self, camera: &scene::Camera) -> Option<Seen> {
        let body = self.controller.held_by();
        let (_, id, to) = self
            .parts
            .all()
            .filter_map(|(id, seat, part)| {
                let Shape::Portal(Portal { to: Some(to), .. }) = part.shape else {
                    return None;
                };
                let away_m = parts::away_m(&self.generator, seat, &part.pose, body);
                (away_m <= THROUGH_M).then_some((away_m, id, to))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))?;
        let ring = self.parts.transform(&self.generator, id)?;
        let Some(star) = self.parts.landmark_transform(&self.generator, to) else {
            self.parts.chart_for(self.clock_s);
            return None;
        };
        // The pane is the ring's `y = 0` plane and its heading `+y`; the star
        // lies on the ground with its heading `+y` too: frame to frame, the
        // rigid part alone.
        let star = rigid(&star);
        let carried = star * rigid(&ring).inverse();
        let turn = DQuat::from_mat3(&carried.matrix3);
        // A wide lens in the portal, carried to the star and lifted high
        // over it, so the far side is seen whole and from above: it looks
        // the way the eye looks through the pane's middle.
        let middle = ring.transform_point3(DVec3::new(0.0, 0.0, 0.5));
        let line = (middle - camera.position).normalize_or(camera.rotation * DVec3::NEG_Z);
        let rotation = looking(turn * line, turn * (camera.rotation * DVec3::Y))
            .unwrap_or(turn * camera.rotation);
        Some(Seen {
            portal: id,
            camera: scene::Camera {
                position: carried.transform_point3(middle) + star.matrix3.z_axis * LENS_LIFT_M,
                rotation,
                fov_y: LENS_FOV_Y,
                ..*camera
            },
            turn,
            star: star.translation,
            size: self.through_size(camera, id),
        })
    }

    /// How big the world past a portal is drawn, of the view's size: as big
    /// as the pane is seen, in steps, a quarter at least and whole at most,
    /// so a pane that fills the view is as sharp as the view and a far one
    /// costs a quarter.
    fn through_size(&self, camera: &scene::Camera, portal: PartId) -> f32 {
        let seen = self
            .parts
            .transform(&self.generator, portal)
            .and_then(|ring| {
                let centre = ring.transform_point3(DVec3::new(0.0, 0.0, 0.5));
                let radius_m = ring.matrix3.x_axis.length() / 2.0;
                Projection::of(camera, self.aspect).height_of(centre, radius_m)
            })
            .unwrap_or(0.0);
        // The disc's height on the screen, of the view's: a step up at a
        // quarter and at a half.
        match seen * 2.0 {
            h if h >= 0.5 => 1.0,
            h if h >= 0.25 => 0.5,
            _ => 0.25,
        }
    }

    /// The body a point of the world is on, and where from its centre: the
    /// moon within its reach, else the planet.
    fn body_of(&self, point: DVec3) -> (protocol::Body, DVec3) {
        let moon = self.cells.centre(protocol::Body::Moon);
        match (point - moon).length() < plugin::MOON_REACH_M {
            true => (protocol::Body::Moon, point - moon),
            false => (protocol::Body::Planet, point),
        }
    }

    /// Where the moon is now: an orbit on rails, a function of the clock. It
    /// laps the sky a little slower than the sun on a tilted path, so its
    /// phase changes from night to night.
    fn moon_position(&self) -> DVec3 {
        let angle = self.noon_offset + self.clock_s / DAY_S * core::f64::consts::TAU;
        let lunar = angle * MOON_PACE + 2.4;
        let tilt = -0.25 + 0.3 * (lunar * 0.37).sin();
        DVec3::new(lunar.cos(), tilt, lunar.sin()).normalize() * MOON_ORBIT_M
    }

    /// Streams the terrain of every body and lists what to draw. Each
    /// streamer works around its own body's centre, so the moon's sees the
    /// camera from where the moon is now. They build within one `budget`,
    /// the body the eye is held by first and the other with what it left,
    /// so the ground under the feet never waits on the sky; with none, until
    /// nothing is missing.
    fn stream(
        &mut self,
        camera: &scene::Camera,
        through: Option<&Seen>,
        mut budget: Option<Budget>,
    ) -> (Vec<scene::PatchDraw>, Option<Vec<scene::PatchDraw>>) {
        let moon = self.moon_position();
        // The view's eye first, then the eye past a portal, which sees at a
        // quarter of the view's height and asks for a quarter of its detail.
        let eyes: Vec<Viewpoint> = core::iter::once(Viewpoint {
            camera: *camera,
            coarseness: 1.0,
        })
        .chain(through.map(|seen| Viewpoint {
            camera: seen.camera,
            // As coarse as the sight is small: a quarter height sight asks
            // for a quarter of the detail.
            coarseness: f64::from(1.0 / seen.size),
        }))
        .collect();
        let from_moon: Vec<Viewpoint> = eyes
            .iter()
            .map(|eye| Viewpoint {
                camera: scene::Camera {
                    position: eye.camera.position - moon,
                    ..eye.camera
                },
                coarseness: eye.coarseness,
            })
            .collect();
        // Grass grows again where what is built over it changed.
        for (middle, angle) in self.cells.drain_touched() {
            self.terrain.regrow(middle, angle);
        }
        let (generator, aspect) = (&self.generator, self.aspect);
        let build = &self.cells;
        let covered = |sector, column, heights| build.covered(sector, column, heights);
        let flora = self.flora.as_ref().filter(|_| self.living);
        let order = match self.controller.on_moon() {
            true => [Kind::Moon, Kind::Planet],
            false => [Kind::Planet, Kind::Moon],
        };
        let (mut on_planet, mut on_moon) = (Vec::new(), Vec::new());
        for kind in order {
            let (terrain, flora, cover, eyes, drawn): (_, _, Cover, _, _) = match kind {
                Kind::Planet => (&mut self.terrain, flora, &covered, &eyes, &mut on_planet),
                // No grass grows on the moon for what is built there to cover.
                Kind::Moon => (
                    &mut self.moon_terrain,
                    None,
                    &|_, _, _| false,
                    &from_moon,
                    &mut on_moon,
                ),
            };
            *drawn = match budget.as_mut() {
                Some(budget) => terrain.update(generator, flora, cover, eyes, aspect, budget),
                None => terrain.settle(generator, flora, cover, eyes, aspect),
            };
        }
        let at = |body_center: DVec3| move |id| scene::PatchDraw { id, body_center };
        let mut lists = on_planet.into_iter().zip(on_moon).map(|(planet, on_moon)| {
            let planet = planet.into_iter().map(at(DVec3::ZERO));
            planet
                .chain(on_moon.into_iter().map(at(moon)))
                .collect::<Vec<_>>()
        });
        let view = lists.next().expect("the view's eye");
        (view, lists.next())
    }

    /// Every picture placed, at the level it is seen at from the camera,
    /// and every cabinet's game, and the frame closed for them, `dt`
    /// seconds after the last: what went unseen is dropped, and the live
    /// pictures opened and closed.
    fn pictures_drawn(&mut self, camera: &scene::Camera, dt: f64) -> Vec<scene::PictureDraw> {
        let screen = Screen {
            camera: *camera,
            aspect: f64::from(self.aspect),
            height_px: f64::from(self.view_px[1]),
            play_m: f64::from(self.effects.video_play_m),
        };
        let body = self.controller.held_by();
        let reach_m = f64::from(self.effects.picture_reach_m);
        self.pictures.begin_frame();
        let mut drawn: Vec<scene::PictureDraw> = self
            .parts
            .pictures(&self.generator, body, reach_m)
            .map(|(media, placed)| {
                self.pictures
                    .draw(&media, placed, &screen, &mut self.requests, &mut self.live)
            })
            .collect();
        // A skin is a picture on a cabinet's body: as tall on the screen as
        // the body, the body's whole height its picture's.
        let model_reach = f64::from(self.effects.model_reach_m);
        self.skins.clear();
        for (skin, world_from_box) in self.parts.skins(&self.generator, body, model_reach) {
            let axes = world_from_box.matrix3;
            let height_m = axes.z_axis.length();
            let radius_m = 0.5
                * (axes.x_axis.length_squared()
                    + axes.y_axis.length_squared()
                    + axes.z_axis.length_squared())
                .sqrt();
            let centre = world_from_box.transform_point3(DVec3::new(0.0, 0.0, 0.5));
            let seen = screen.measure(centre, radius_m, height_m);
            if let Some(picture) = self.pictures.dress(&skin, seen, &mut self.requests) {
                self.skins.insert(skin.full, picture);
            }
        }
        drawn.extend(self.parts.games(&self.generator, body, reach_m).filter_map(
            |(names, cart, world_from_quad)| {
                self.carts
                    .draw(&mut self.live, names, cart, world_from_quad, &screen)
            },
        ));
        let orders = self.live.end_frame();
        self.pictures.end_frame(&orders, &mut self.live);
        let parts = &self.parts;
        self.carts
            .end_frame(&orders, &mut self.live, dt, |key| carts::runs(parts, key));
        self.speakers.begin_frame();
        self.parts
            .speakers(&self.generator, camera.position, &mut self.speakers);
        self.speakers.end_frame();
        self.heard.clear();
        self.heard.extend_from_slice(self.pictures.heard());
        self.heard.extend_from_slice(self.speakers.heard());
        self.heard
            .extend(self.carts.heard(&self.live, camera.position));
        drawn
    }

    /// The frame drawn: `played`, the cabinet played here and what each of
    /// its stations holds ([`carts::Carts::pads_played`]).
    fn frame(
        &self,
        camera: scene::Camera,
        patches: Vec<scene::PatchDraw>,
        pictures: Vec<scene::PictureDraw>,
        through: Option<(Seen, Vec<scene::PatchDraw>)>,
        played: Option<(PartId, [Pad; arcade::PADS])>,
    ) -> Frame {
        let overlay = self.overlay(&camera, &pictures);
        let angle = self.noon_offset + self.clock_s / DAY_S * core::f64::consts::TAU;
        let sun = DVec3::new(angle.cos(), 0.35, angle.sin()).normalize();

        // Every body, the player's first: an avatar when its file has
        // landed, the box figure until then. With the camera in its head, or
        // the eye lent away from the body, the player's casts its shadow and
        // is not seen; the world past a portal draws no body at all, so this
        // is the main view's alone.
        let mut boxes = Vec::new();
        let mut skinned = Vec::new();
        let hidden = !self.controller.sees_own_body() || self.controls.eye_away();
        let own = (self.charge(), hidden);
        let carrier = self.controller.carrier();
        let own_thing = self.grasped_thing();
        let bodies = core::iter::once((None, &self.figure, self.motion(), carrier, own_thing, own))
            .chain(
                self.peers
                    .bodies(self.moon_position(), &self.ways, &self.things)
                    .map(|(session, figure, motion, carrier, thing)| {
                        (Some(session), figure, motion, carrier, thing, (0.0, false))
                    }),
            );
        // The lamps of things held: this body's, then the others by how
        // far the eye is past where their light ends.
        let mut held_lamps = Vec::new();
        for (who, figure, mut motion, carrier, thing, (shine, shadow_only)) in bodies {
            let feet = DAffine3::from_mat3_translation(motion.basis, motion.position);
            let drawn = figure.drawn(self.seat(figure, who, carrier), feet);
            (motion.position, motion.basis) = (drawn.translation, drawn.matrix3);
            let as_drawn = |part: scene::BoxPart| scene::BoxPart {
                shine,
                shadow_only,
                ..part
            };
            let hand = match figure.instance(&motion, &self.clips) {
                Some((instance, hand)) => {
                    skinned.push(scene::SkinnedInstance {
                        shine,
                        shadow_only,
                        ..instance
                    });
                    hand
                }
                None => {
                    let parts = box_figure::parts(&motion, figure.held());
                    boxes.extend(parts.into_iter().map(as_drawn));
                    hand::box_hand(&motion, figure.hand_out_m())
                }
            };
            if let Some(thing) = thing.filter(|_| figure.grasped()) {
                // A part of the thing that shines of itself, a lens, keeps
                // its light on a body drawn darker.
                let lamp = thing.placed(hand, &mut boxes, |part| scene::BoxPart {
                    shine: part.shine.max(shine),
                    shadow_only: shadow_only || part.shadow_only,
                    ..part
                });
                if let Some(lamp) = lamp {
                    let beyond = lamp.position.distance(camera.position) - f64::from(lamp.reach_m);
                    held_lamps.push((who.is_some(), beyond, lamp));
                }
            }
        }
        held_lamps.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)));
        held_lamps.truncate(hand::LAMPS);
        self.animals.boxes(&mut boxes);
        // What the plugins drew over the world, as each turn left it.
        boxes.extend(
            self.drawn
                .iter()
                .flat_map(|(_, parts)| parts.iter().copied()),
        );
        Frame {
            camera,
            sun_direction: sun.as_vec3(),
            moon: scene::Moon {
                position: self.moon_position(),
                radius_m: MOON_RADIUS_M,
            },
            planet_radius_m: self.generator.sphere().radius_m(),
            clock_s: self.clock_s,
            patches,
            shadow_patches: self
                .terrain
                .shadow_patches()
                .iter()
                .map(|&id| scene::PatchDraw {
                    id,
                    body_center: DVec3::ZERO,
                })
                .chain(
                    self.moon_terrain
                        .shadow_patches()
                        .iter()
                        .map(|&id| scene::PatchDraw {
                            id,
                            body_center: self.moon_position(),
                        }),
                )
                .collect(),
            effects: self.effects,
            interaction: scene::InteractionCapsule {
                start: self.controller.drawn_position() + self.controller.up() * 0.2,
                end: self.controller.drawn_position() + self.controller.up() * 1.5,
                radius_m: 0.65,
            },
            boxes,
            skinned,
            volumes: self.cells.drawn(),
            glass: self.cells.glazed(),
            lamps: {
                let mut lamps = self.cells.lamps();
                lamps.extend(self.parts.lamps(&self.generator));
                lamps.extend(held_lamps.into_iter().map(|(.., lamp)| lamp));
                lamps
            },
            models: self.parts.drawn(
                &self.generator,
                self.controller.held_by(),
                f64::from(self.effects.model_reach_m),
                self.clock_s,
                &self.skins,
                played,
            ),
            ghost: self.cells.ghost(),
            guides: self.cells.guides_drawn(),
            pictures,
            panes: self.parts.panes(
                &self.generator,
                self.controller.held_by(),
                f64::from(self.effects.model_reach_m),
                &self.seen,
            ),
            through: through.map(|(seen, patches)| scene::Through {
                camera: seen.camera,
                turn: seen.turn,
                lens: LENS_RIM,
                patches,
                size: seen.size,
            }),
            warp: self.warp(&camera),
            overlay,
        }
    }

    /// The screen the lent eye is held on, laid over the view as drawn,
    /// once the eye is nearly there: where the world draws it this frame.
    fn overlay(
        &self,
        camera: &scene::Camera,
        pictures: &[scene::PictureDraw],
    ) -> Option<scene::Overlay> {
        let (part, opacity) = self.controls.shown()?;
        let key = carts::game_of(&self.parts, part)?;
        let (picture, texels) = self.carts.picture_of(key, &self.live)?;
        let drawn = pictures.iter().find(|draw| draw.picture == Some(picture))?;
        overlay::laid(
            camera,
            self.view_px,
            overlay::clear(self.view_px, self.clear_px),
            drawn.transform,
            picture,
            texels,
            opacity,
        )
    }

    /// The part of the view left clear, as fractions of it from its top
    /// left: what a plugin frames a look in.
    fn clear(&self) -> [f64; 4] {
        let [wide, tall] = self.view_px.map(f64::from);
        let [left, top, right, bottom] = overlay::clear(self.view_px, self.clear_px);
        [left / wide, top / tall, right / wide, bottom / tall]
    }

    /// The walk and fly key: on foot the body flies, in flight or in a way
    /// it walks. On an animal it is the animal's key alone: one with wings
    /// takes off, and aloft lands; one without says so and keeps its rider.
    fn toggle_mode(&mut self) {
        let mode = match self.controller.mode() {
            Mode::Walk => Mode::Fly,
            Mode::Way(name) if self.controller.carrier() != 0 => match self.controller.aloft() {
                true => Mode::Way(name),
                false => Mode::Fly,
            },
            Mode::Fly | Mode::Way(_) => Mode::Walk,
        };
        self.set_mode(mode);
    }

    fn set_mode(&mut self, mode: Mode) {
        if let Err(message) = self.put_in(&mode) {
            self.events.push(Event::Rejected { message });
        }
        self.tell_mode();
    }

    /// Has the body move as `mode` says, stepped down first off whatever it
    /// rode. A way of moving is one a plugin on in this world gave, taken on
    /// the planet alone; a way that carries is mounted on an animal through
    /// the plugin's offer, never put in. On an animal, flying takes off on
    /// its wings, refused on one without, and the way it rides lands;
    /// aloft, nothing else is taken until it lands.
    fn put_in(&mut self, mode: &Mode) -> Result<(), String> {
        if self.controller.carrier() != 0 {
            let built = Built {
                cells: &self.cells,
                parts: &self.parts,
                trunks: &self.trunks,
            };
            let said = |why: Refused| match why {
                Refused::Flightless => "what the body rides does not fly here".to_string(),
                Refused::NoRoom => "no room over the body to take off".to_string(),
                Refused::NoGround => "no ground under the body to land on".to_string(),
            };
            let riding = self.controller.way().map(str::to_owned);
            match mode {
                Mode::Fly => {
                    return self
                        .controller
                        .take_off(self.animals.winged(None), &self.generator, built)
                        .map_err(said);
                }
                Mode::Way(name) if riding.as_deref() == Some(name.as_str()) => {
                    return self.controller.land(&self.generator, built).map_err(said);
                }
                _ if self.controller.aloft() => {
                    return Err("aloft, the body lands before it leaves what it rides".into());
                }
                _ => {}
            }
        }
        if let Mode::Way(name) = mode {
            if self.controller.way() == Some(name) {
                return Ok(());
            }
            match self.ways.get(name) {
                None => {
                    return Err(format!(
                        "`{name}`: no plugin on in this world gave this way"
                    ));
                }
                Some((_, way)) if way.profile.carried() => {
                    return Err(format!(
                        "`{name}`: a way that carries is mounted on an animal, through its offer"
                    ));
                }
                Some(_) => {}
            }
        }
        self.step_off();
        match mode {
            Mode::Walk => {}
            Mode::Fly => self.controller.fly(),
            Mode::Way(name) => {
                let (_, way) = self.ways.get(name).expect("a way given, asked above");
                if !self
                    .controller
                    .take_way(name, &way.profile, way.steering, 0, &self.generator)
                {
                    return Err(format!(
                        "`{name}`: no way of moving is taken off the planet or under its ground"
                    ));
                }
            }
        }
        Ok(())
    }

    /// On foot, stepped down off whatever the body rode.
    fn step_off(&mut self) {
        let built = Built {
            cells: &self.cells,
            parts: &self.parts,
            trunks: &self.trunks,
        };
        self.controller.step_off(&self.generator, built);
    }

    fn regenerate(&mut self, generator: Generator) {
        // Another recipe is another world, and the link was to this one. A
        // front end that changes worlds opens a new link afterwards.
        if self.session.link() != session::Link::Offline {
            self.session.close();
            self.went_offline();
        }
        let mode = self.controller.mode();
        // Turning a knob should leave you where you stood. But the ground
        // under a place belongs to the recipe, so the same address on new
        // ground can be open sea, and over a field of the Earth it usually
        // is: the planet is 71% water. Keep the place only while it is still
        // a place to stand, and let the world choose otherwise.
        let held = self.controller.point();
        let keep = !self.controller.on_moon()
            && self.controller.sphere() == generator.sphere()
            && standable(&generator, held);
        self.generator = generator;
        // What grew, and its trunks, grew on the old ground: it grows for
        // the new one when it is drawn.
        self.flora = self.living.then(|| Flora::grow(&self.generator));
        self.trunks = Trunks::default();
        // What a plugin had in hand was for the old ground, and so were the
        // volumes that stood on it.
        let (plugins, lent) = self.lend();
        plugins.rest(lent);
        self.played = None;
        self.cells.clear();
        self.pictures.clear();
        self.carts.clear(&self.live);
        self.live.clear();
        self.speakers.clear();
        if keep {
            // A flyer keeps its altitude, which `stand_at` lifts if the new
            // ground rose through it; a walker lands on whatever is there now.
            let height_m = (mode == Mode::Fly).then(|| self.controller.height_m());
            self.controller.stand_at(height_m, &self.generator);
        } else {
            self.controller = Controller::spawn(spawn_point(&self.generator), &self.generator);
        }
        // The body moves as it did, on foot off an animal of the old
        // ground; a way it may no longer take, it leaves.
        let mode = match mode {
            Mode::Way(_) if self.controller.carrier() != 0 => Mode::Walk,
            mode => mode,
        };
        if self.put_in(&mode).is_err() {
            self.step_off();
        }
        self.terrain.clear();
        self.moon_terrain.clear();
        self.animals.clear();
        // A new world starts its own clock.
        self.clock_s = 0.0;
        self.face_the_sun();
        self.events.push(Event::RecipeChanged {
            recipe: self.generator.recipe().clone(),
        });
    }

    /// Mid morning at the spawn when the clock reads zero. The spawn is the
    /// recipe's, so the offset is the world's too.
    fn face_the_sun(&mut self) {
        let up = self.controller.up();
        self.noon_offset = up.z.atan2(up.x) - 0.7;
    }

    /// The name of the pace the body goes at, in a way that goes in paces.
    fn pace(&self) -> Option<&'static str> {
        let (_, way) = self.ways.get(self.controller.way()?)?;
        match &way.profile {
            Profile::Paces(paces) => {
                // A take-off or a landing refused says why for a moment,
                // over the flight or the pace.
                let flight = paces.flight.as_ref();
                match (self.controller.refused(), flight) {
                    (Some(Refused::NoRoom), Some(flight)) => return Some(flight.no_room),
                    (Some(Refused::Flightless), Some(flight)) => return Some(flight.wingless),
                    (Some(Refused::NoGround), Some(flight)) => return Some(flight.no_ground),
                    _ => {}
                }
                if self.controller.aloft() {
                    return flight.map(|flight| {
                        match self.controller.at_ceiling(&self.generator) {
                            true => flight.ceiling,
                            false => flight.name,
                        }
                    });
                }
                match (self.controller.backs(), &paces.backing) {
                    (true, Some(backing)) => Some(backing.name),
                    _ => Some(paces.paces.get(self.controller.pace()?)?.name),
                }
            }
            Profile::Hover(_) => None,
        }
    }

    fn stats(&mut self, dt: f64) {
        self.stats_timer_s += dt;
        self.frames_since_stats += 1;
        if self.stats_timer_s >= STATS_EVERY_S {
            let grid = self.generator.sphere().blocks();
            let pose = self.here();
            self.events.push(Event::Stats {
                fps: (f64::from(self.frames_since_stats) / self.stats_timer_s) as f32,
                altitude_m: self.controller.altitude_m(&self.generator),
                speed_mps: self.controller.speed_mps(),
                place: pose.place(grid),
                pose: pose.text(grid),
                bearing_deg: pose.bearing_deg,
                pace: self.pace(),
                calls: self.callable().is_some(),
                flying: self.controller.flying(),
            });
            self.stats_timer_s = 0.0;
            self.frames_since_stats = 0;
        }
    }
}

/// Whether a body could stand at `point`: dry ground, clear of the waves.
///
/// This is what a place carried from one recipe to another has to answer. An
/// address on its own says nothing about the ground, because the ground is
/// the recipe's, so the same characters can name a hill under one and the
/// middle of an ocean under the next.
fn standable(generator: &Generator, point: SurfacePoint) -> bool {
    let direction = generator.sphere().blocks().direction(point);
    dry(generator.sample(direction), generator.scale())
}

/// Ground clear of the waves. Heights are in reference metres, so the margin
/// scales with the body: a metre of clearance on the reference planet, less
/// on a smaller one.
fn dry(sample: Sample, scale: f64) -> bool {
    sample.height_m > scale
}

/// A place to stand: the first gentle, dry ground along a fixed search path
/// over the whole body. Deterministic per recipe, so a world always opens the
/// same.
///
/// The search is every sector, not just the first. A face of a world can be
/// all ocean, and on a field of the Earth one of them is most of the Pacific,
/// so a search that gave up after sector 0 spawned people hundreds of metres
/// under water. Sector 0 is still walked first, so a world that had an answer
/// there opens where it always did.
pub(crate) fn spawn_point(generator: &Generator) -> SurfacePoint {
    let grid = generator.sphere().blocks();
    let side = f64::from(grid.side());
    // Heights are written in reference metres and printed at the body's own
    // size, so what counts as dry ground scales with the world.
    let scale = generator.scale();
    // A coarse lattice ordered from a sector's centre outward.
    let mut cells: Vec<(i32, i32)> = (-12..=12)
        .flat_map(|i| (-12..=12).map(move |j| (i, j)))
        .collect();
    cells.sort_by_key(|(i, j)| i * i + j * j);
    // The highest ground seen anywhere, for a world with no gentle spot at
    // all: the shallowest water is the likeliest place to find a shoal, and
    // is a kinder answer than the middle of a sector.
    let mut highest: Option<(f64, SurfacePoint)> = None;
    for sector in Sector::ALL {
        for &(i, j) in &cells {
            let point = SurfacePoint::new(
                sector,
                side * (0.5 + f64::from(i) / 26.0),
                side * (0.5 + f64::from(j) / 26.0),
            );
            let sample = generator.sample(grid.direction(point));
            let gentle = dry(sample, scale) && sample.material != Material::Snow;
            if gentle && sample.height_m < 300.0 * scale {
                return point;
            }
            match highest {
                Some((height_m, _)) if height_m >= sample.height_m => {}
                _ => highest = Some((sample.height_m, point)),
            }
        }
    }
    // A water world, or one frozen solid: stand on the best there is.
    let (_, point) = highest.expect("the lattice has at least one cell");
    point
}

/// splitmix64 finalizer: spreads a counter into a seed.
fn mix(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9e37_79b9_7f4a_7c15);
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    x ^ (x >> 31)
}

/// Puts one body in the avatar it wants, when the file has landed, and asks
/// for it otherwise.
fn dress(figure: &mut Figure, wanted: &str, wardrobe: &mut Wardrobe, requests: &mut Requests) {
    match wardrobe.get(wanted) {
        Some(worn) if figure.mesh() != Some(worn.mesh) => figure.wear(worn),
        Some(_) => {}
        None => wardrobe.want(wanted, requests),
    }
}

/// Puts the body at a place, as [`Client::go_to`] does and a passage does
/// as it goes through: the point first, because it is what the body stands
/// on and what the moon's radial is read from; then the body, then the
/// height, then the way of looking. Each step reads the one before it.
fn land(
    controller: &mut Controller,
    generator: &Generator,
    moon: DVec3,
    text: &str,
) -> Result<(), String> {
    let grid = generator.sphere().blocks();
    let pose = place::Pose::parse(grid, text)?;
    let found = topology::Place {
        column: pose.column,
        h: pose.h,
        chars: pose.chars,
    };
    let [su, sv] = found.span(grid).map(f64::from);
    let point = SurfacePoint::new(
        pose.column.sector,
        f64::from(pose.column.u) + su / 2.0,
        f64::from(pose.column.v) + sv / 2.0,
    );
    controller.teleport(point, generator);
    if pose.on_moon {
        controller.set_moon(controller::MoonBody {
            center: moon,
            radius_m: MOON_RADIUS_M,
        });
        let direction = DVec3::from(grid.direction(point));
        controller.stand_on_moon(direction, generator);
    }
    controller.stand_at(pose.h.map(|h| f64::from(h) * topology::BLOCK_M), generator);
    if let Some(bearing) = pose.bearing_deg {
        controller.face(bearing);
    }
    if let Some(pitch) = pose.pitch_deg {
        controller.set_pitch(pitch.to_radians());
    }
    if let Some(boom_m) = pose.boom_m {
        controller.set_boom(boom_m);
    }
    Ok(())
}

/// How far from the body a landmark's name is hung over it, metres: as far
/// as a nametag is read.
const LABEL_M: f64 = 120.0;

/// How near a label drawn this frame hangs to where a marker hangs for the
/// marker to be its thing's, metres: the two are the same point, give or
/// take the thing's own motion within the frame.
const TAGGED_M: f64 = 0.25;

/// How far from the body a portal is seen through, metres: three plots.
/// One at a time, the nearest; further, a pane is a lens.
const THROUGH_M: f64 = 96.0;

/// What is seen through a portal this frame: which, the eye past it, the
/// star it leads to, and how big the world past it is drawn, of the view.
struct Seen {
    portal: PartId,
    camera: scene::Camera,
    /// What carries a direction on this side of the portal to the far side.
    turn: DQuat,
    star: DVec3,
    size: f32,
}

/// The lens in a portal: how high over the star it is carried, metres; the
/// height of its picture, radians; and how far off its line the pane's rim
/// sees, radians, inside that height so the rim never reads past the
/// picture's edge.
const LENS_LIFT_M: f64 = 12.0;
const LENS_FOV_Y: f32 = 140.0 * core::f32::consts::PI / 180.0;
const LENS_RIM: f32 = 65.0 * core::f32::consts::PI / 180.0;
/// How long a pane takes to become a window from a lens, and back, seconds.
const SEEN_S: f64 = 0.6;

/// How long a body falls in before it is through, and how long the
/// picture takes to burst out past it, seconds: a passage whose place is
/// known and whose ground has come is under a second, a blink of the world.
const WARP_IN_S: f64 = 0.35;
const WARP_OUT_S: f64 = 0.65;
/// How long a body is held in a passage at most, seconds: for the place it
/// leads to, then for the ground there.
const HELD_S: f64 = 8.0;

/// A body's passage to a place, through a portal or travelling with none:
/// it falls in, the picture drawn into the portal or into the body, and is
/// held there until the place is said; it goes through and lands at the
/// place, held until the ground there has come; then the picture bursts
/// out of it for [`WARP_OUT_S`].
pub(crate) struct Passage {
    /// Where it leads, once said.
    place: Option<String>,
    /// The portal it falls into; none for a travel, which falls into the
    /// body.
    portal: Option<PartId>,
    started_s: f64,
    stage: Stage,
}

/// Where a passage is.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Stage {
    /// Falling in, then held there.
    In,
    /// Through since this second of the clock, held until the ground comes.
    Through(f64),
    /// Bursting out since this second of the clock.
    Out(f64),
}

impl Passage {
    pub(crate) fn new(place: Option<String>, portal: Option<PartId>, started_s: f64) -> Passage {
        Passage {
            place,
            portal,
            started_s,
            stage: Stage::In,
        }
    }

    /// Takes the body through to `place`, by `portal` or by none. A
    /// passage still falling in, or held for its place, is led there; one
    /// through already, or bursting out, falls in again from as deep as
    /// the picture is, so it never jumps.
    pub(crate) fn lead(
        passage: &mut Option<Passage>,
        place: String,
        portal: Option<PartId>,
        now_s: f64,
    ) {
        match passage {
            Some(held) if held.stage == Stage::In => held.place = Some(place),
            _ => {
                let deep = passage.as_ref().map_or(0.0, |was| was.depth(now_s));
                *passage = Some(Passage::new(Some(place), portal, now_s - deep * WARP_IN_S));
            }
        }
    }

    /// How deep in the picture is drawn, 0 whole to 1 gone through: rising
    /// as the body falls in, held through, falling as it bursts out.
    fn depth(&self, now_s: f64) -> f64 {
        match self.stage {
            Stage::In => ((now_s - self.started_s) / WARP_IN_S).clamp(0.0, 1.0),
            Stage::Through(_) => 1.0,
            Stage::Out(at) => 1.0 - ((now_s - at) / WARP_OUT_S).clamp(0.0, 1.0),
        }
    }

    /// Lets the body out where it fell in: a portal that leads nowhere.
    pub(crate) fn let_out(&mut self, now_s: f64) {
        if self.stage == Stage::In {
            self.stage = Stage::Out(now_s);
        }
    }
}

fn passage_stage(passage: &mut Option<Passage>, stage: Stage) {
    if let Some(passage) = passage {
        passage.stage = stage;
    }
}

/// The rotation of a camera that looks along `forward` with `up` above it;
/// none when the two are one line.
fn looking(forward: DVec3, up: DVec3) -> Option<DQuat> {
    let right = forward.cross(up).try_normalize()?;
    let forward = forward.normalize();
    let up = right.cross(forward);
    Some(DQuat::from_mat3(&glam::DMat3::from_cols(
        right, up, -forward,
    )))
}

/// A frame's rigid part: its rotation and translation, its scale taken out,
/// each axis made unit.
fn rigid(frame: &DAffine3) -> DAffine3 {
    let m = frame.matrix3;
    let axes = DMat3::from_cols(
        m.x_axis.normalize(),
        m.y_axis.normalize(),
        m.z_axis.normalize(),
    );
    DAffine3::from_mat3_translation(axes, frame.translation)
}

/// The renderer's own projection of a point of the world onto the screen:
/// reversed infinite depth has no far plane, so only what is behind the
/// camera or off the sides is left out.
struct Projection {
    inverse: DQuat,
    position: DVec3,
    focal: f64,
    aspect: f64,
    near: f64,
}

impl Projection {
    fn of(camera: &scene::Camera, aspect: f32) -> Projection {
        Projection {
            inverse: camera.rotation.inverse(),
            position: camera.position,
            focal: 1.0 / (f64::from(camera.fov_y) / 2.0).tan(),
            aspect: f64::from(aspect),
            near: f64::from(camera.near),
        }
    }

    /// A point as the eye sees it, across and up scaled so the view's edge
    /// is at its depth.
    fn seen(&self, point: DVec3) -> marker::Seen {
        let local = self.inverse * (point - self.position);
        marker::Seen {
            across: self.focal / self.aspect * local.x,
            up: self.focal * local.y,
            depth: -local.z,
        }
    }

    /// How tall a sphere round a point is on the screen, of the viewport's
    /// height. `None` behind the camera.
    fn height_of(&self, point: DVec3, radius_m: f64) -> Option<f32> {
        let local = self.inverse * (point - self.position);
        if local.z >= -self.near {
            return None;
        }
        Some((self.focal * radius_m / -local.z / 2.0) as f32)
    }

    /// Where a point lands: fractions of the viewport from the top left,
    /// and how far the point is. `None` behind the camera or off the sides,
    /// a little past the edge, so a label slides off instead of popping.
    fn project(&self, point: DVec3) -> Option<(f32, f32, f32)> {
        let local = self.inverse * (point - self.position);
        if local.z >= -self.near {
            return None;
        }
        let x = self.focal / self.aspect * local.x / -local.z;
        let y = self.focal * local.y / -local.z;
        if x.abs() > 1.2 || y.abs() > 1.2 {
            return None;
        }
        Some((
            ((x + 1.0) / 2.0) as f32,
            ((1.0 - y) / 2.0) as f32,
            local.length() as f32,
        ))
    }
}
