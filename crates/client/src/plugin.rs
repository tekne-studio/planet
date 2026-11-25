//! The host of plugins on the client: who is plugged in, which of them the
//! world has on, and what each is offered.
//!
//! A plugin's client half is a crate of its own that imports this one. The
//! core names no plugin: a shell plugs in what its version carries, the
//! world's statement says which are on, and a command, a frame from the
//! server, a key or a turn reaches a plugin by the name it gave.
//!
//! A plugin speaks to the core's systems through its [`Host`]: the ground,
//! the cells, the parts, the body. What a system holds is read freely. What
//! changes it is an op, and the host asks whether this session may before
//! the system sees it.

use glam::{DAffine3, DQuat, DVec3};
use part::{Part, PartId, Pose as PartPose, Shape};
use protocol::Body;
use serde::Serialize;
use serde_json::Value;
use topology::{QuadSphere, SurfacePoint};
use voxel::{Gesture, Span};
use worldgen::Generator;

use crate::Passage;
use crate::Trunks;
use crate::animals::{self, Animals, Near};
use crate::carts::Carts;
use crate::cells::{self, Cells, Guide, Refusal, Seat, Sight};
use crate::collision;
use crate::controller::{Built, Controller};
use crate::controls::{Controls, Look, Pad};
use crate::figure::{Play, Playing};
use crate::given;
use crate::hand::{Thing, Things};
use crate::history::{History, Owner};
use crate::input::{Chord, Input, Key, KeyAsk};
use crate::live::Live;
use crate::marker::{Marker, Markers};
use crate::model3d::ModelRefusal;
use crate::offer::{Act, Offer, Offered};
use crate::parts::{Facing, Landmarked, Mark, PartRefusal, Parts};
use crate::peers::Peers;
use crate::pets::{Pet, Pets};
use crate::place::Pose;
use crate::seam::{Event, Level, PluginOn};
use crate::session::Session;
use crate::way::{Way, Ways};

/// How far from the moon's centre a point of the world is the moon's,
/// metres: its radius and the band over it where anything stands.
pub(crate) const MOON_REACH_M: f64 = worldgen::MOON_RADIUS_M + 2000.0;

/// The most bytes a plugin relays at once ([`Host::relay`]): the wire's.
pub const RELAY_BYTES: usize = protocol::Limit::RelayBytes as usize;

/// Bytes relayed about a plugin's thing, as each holder hears them.
#[derive(Clone, Copy, Debug)]
pub struct Relayed<'a> {
    pub thing: u64,
    /// The session that sent them.
    pub session: u32,
    pub payload: &'a [u8],
    /// The world's clock when it relayed them, milliseconds: one clock for
    /// every holder.
    pub clock_ms: u64,
}

/// A plugin's client half.
pub trait Plugin {
    /// The name the wire's envelope and the world's statement say, and the
    /// one before the dot in its commands and events: `chat.say`.
    fn name(&self) -> &'static str;

    /// The version of its wire and of its seam. A world that speaks another
    /// leaves it off here.
    fn version(&self) -> u32;

    /// A command from a front end or an agent, its name without the
    /// plugin's: `say` for `chat.say`. `body` is the rest of its JSON.
    fn command(&mut self, kind: &str, body: Value, host: &mut Host<'_>);

    /// A message of this plugin from the world server.
    fn receive(&mut self, _kind: &str, _payload: &[u8], _host: &mut Host<'_>) {}

    /// Bytes relayed about a thing of the plugin's, held by this client
    /// together with it ([`Host::relay`]): this client's own come back too,
    /// and every holder hears them all in one order, the plugin's events
    /// among them.
    fn relayed(&mut self, _relayed: Relayed<'_>, _host: &mut Host<'_>) {}

    /// The session `session` left the world, heard in its place among the
    /// plugin's messages: what it held of the plugin's is let go.
    fn left(&mut self, _session: u32, _host: &mut Host<'_>) {}

    /// The keys it asks for, by name: data, which each shell binds. Asked
    /// once, as it is plugged in.
    fn keys(&self) -> Vec<KeyAsk> {
        Vec::new()
    }

    /// The ways of moving it gives a body, as data: the core moves a body
    /// in one when the plugin asks ([`Host::set_way`]). Asked once, as it is
    /// plugged in.
    fn ways(&self) -> Vec<Way> {
        Vec::new()
    }

    /// The things it gives the body's hand, as data: the hand grasps one
    /// when the plugin asks ([`Host::grasp`]). Asked once, as it is plugged
    /// in.
    fn things(&self) -> Vec<Thing> {
        Vec::new()
    }

    /// A key it asked for went down, by the name it gave it.
    fn key(&mut self, _name: &str, _host: &mut Host<'_>) {}

    /// One of its offers was acted on, by the id it gave it and with what
    /// the offer has, a tap or a hold: the prompt's key, a finger on it, or
    /// an agent naming it ([`Host::offer`]).
    fn act(&mut self, _id: u64, _act: Act, _host: &mut Host<'_>) {}

    /// Its turn in a frame, after the body moved and before the picture is
    /// made.
    fn turn(&mut self, _turn: &Turn<'_>, _host: &mut Host<'_>) {}

    /// Whether it owes the world work it spreads over turns. The world is
    /// not settled while a plugin is busy.
    fn busy(&self) -> bool {
        false
    }

    /// Does at once all the work it owes: for a picture that must show it.
    fn settle(&mut self, _host: &mut Host<'_>) {}

    /// The world it worked in switched it off, or is gone: it puts down what
    /// it had in hand.
    fn rest(&mut self, _host: &mut Host<'_>) {}

    /// The world switched it off: what it gave the body is taken back after
    /// this, the thing in the body's hand too. It says what it must.
    fn off(&mut self, _host: &mut Host<'_>) {}
}

/// The eye a hand aims from: where the camera is, how it looks, and where
/// the pointer is on the view. What a plugin is handed in its turn: the
/// cells take a line of sight, and know no eye.
#[derive(Clone, Copy, Debug)]
pub struct Eye {
    pub position: DVec3,
    /// World from camera. The camera looks down its `-Z`, `+Y` is up.
    pub rotation: DQuat,
    /// Vertical field of view, radians.
    pub fov_y: f64,
    /// Width over height.
    pub aspect: f64,
    /// The part of the view the shell leaves clear of what it lays over
    /// it, fractions of the view from the top left, `[left, top, right,
    /// bottom]`: a look at a screen frames it here.
    pub clear: [f64; 4],
    /// Fractions of the view from the top left.
    pub pointer: [f64; 2],
}

impl Eye {
    /// The pointer on the view in units of half its height, `y` up: a
    /// distance there is the same across and down.
    fn at(&self) -> [f64; 2] {
        let [x, y] = self.pointer;
        [(2.0 * x - 1.0) * self.aspect, 1.0 - 2.0 * y]
    }

    /// Where the eye is and which way the pointer looks from it, unit.
    pub fn sight(&self) -> (DVec3, DVec3) {
        let [x, y] = self.at();
        let tan = (self.fov_y / 2.0).tan();
        let look = DVec3::new(x * tan, y * tan, -1.0).normalize();
        (self.position, self.rotation * look)
    }
}

/// A plugin's turn in a frame: where the eye and the pointer are, and what
/// of the plugin's is held down.
pub struct Turn<'a> {
    /// The eye, and where the pointer is on its view.
    pub eye: Eye,
    /// Whether the pointer's button is down. False for a plugin that has
    /// not the pointer.
    pub using: bool,
    /// What the controls press, for the plugin they are lent to; `None`
    /// for every other, and for it while the person is away from the
    /// world, the page hidden or its window left ([`crate::Client::away`]).
    pub controls: Option<Pad>,
    /// What is held right now, command and shift. The core's keys, so a
    /// plugin reads them here and never asks for them: a shift asked for
    /// would take the sprint from the body.
    pub chord: Chord,
    /// The keys this plugin asked to read held that are down, by name.
    pub keys: &'a [&'static str],
    /// Whether the shell let everything go since the last turn: what was
    /// half done is dropped, never landed.
    pub interrupted: bool,
    /// Whether the pointer's secondary button was tapped since the last
    /// turn, where the eye's pointer is: heard by every plugin, whoever
    /// has the pointer, so a thing in the world is picked to be looked at
    /// again from anywhere.
    pub inspected: bool,
    /// The clock, seconds: what a plugin moves by itself moves by it, so a
    /// shot at a fixed clock draws it the same every time.
    pub now_s: f64,
}

impl Turn<'_> {
    /// Whether a key this plugin asked to read held is down.
    pub fn held(&self, name: &str) -> bool {
        self.keys.contains(&name)
    }
}

/// This client's own body in the world, as a hand that places something in
/// front of it reads it.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Standing {
    /// From the planet's centre, metres.
    pub position: DVec3,
    pub up: DVec3,
    pub facing: DVec3,
}

/// Where a line of the world meets a part.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct PartHit {
    pub id: PartId,
    /// Along the line, metres.
    pub away_m: f64,
    /// Which way the side met faces, in the world, unit.
    pub outward: DVec3,
    /// Which side of the part's own box it is, unit along one axis: `+z` is
    /// its top, `-z` its underside, the rest its sides.
    pub side: DVec3,
}

/// A body in a way of moving a plugin gave: this client's own, or a peer's.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct InWay {
    /// The peer's session, or `None` for this client's own body.
    pub session: Option<u32>,
    /// The way, by its name in the plugin.
    pub way: &'static str,
    /// The feet as drawn, world space: columns right, up, back, tilted with
    /// the ground, leaned into the turn and bobbing as the body is drawn.
    pub at: DAffine3,
    /// Along the ground, metres per second.
    pub speed_mps: f64,
    /// The animal the way carries, by its number: zero for none.
    pub carrier: u64,
}

/// Where the feet of this client's own body are.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Feet {
    /// The seat they are over: a sector of the body that holds them.
    pub seat: Seat,
    /// The column of it they are over, in that body's own blocks.
    pub point: SurfacePoint,
    /// How high over the datum of that body, metres.
    pub height_m: f64,
}

/// What of the core a host lends a plugin for one call.
pub(crate) struct Lent<'a> {
    pub session: &'a mut Session,
    pub events: &'a mut Vec<Event>,
    pub generator: &'a Generator,
    pub cells: &'a mut Cells,
    pub parts: &'a mut Parts,
    /// The trunks of the plants about the body, which stop it.
    pub trunks: &'a Trunks,
    pub history: &'a mut History,
    pub controller: &'a mut Controller,
    pub level: Option<Level>,
    /// The plugin that has the pointer, while one does.
    pub pointer: &'a mut Option<&'static str>,
    /// The controls and the eye, and who they are lent to.
    pub controls: &'a mut Controls,
    /// The cabinets' games, and the live pictures they run in.
    pub carts: &'a mut Carts,
    pub live: &'a Live,
    /// A body's passage, through a portal or travelling, while it lasts.
    pub passage: &'a mut Option<Passage>,
    /// A clip asked for over what the body moves by, while it plays.
    pub played: &'a mut Option<Playing>,
    pub clock_s: f64,
    /// The ways of moving given, and the other bodies, some in one.
    pub ways: &'a Ways,
    /// The things given the body's hand, and the one this body's hand
    /// grasps, by its full name.
    pub things: &'a Things,
    pub hand: &'a mut Option<String>,
    pub peers: &'a Peers,
    /// The animals drawn this frame, ridden or free.
    pub animals: &'a Animals,
    /// The pets of the world, and this client's asks of its life.
    pub pets: &'a mut Pets,
    /// What each plugin draws over the world, by its name.
    pub drawn: &'a mut Vec<(&'static str, Vec<scene::BoxPart>)>,
    /// What each plugin offers in front of the body, by its name.
    pub offered: &'a mut Offered,
    /// What each plugin points out in the world, by its name.
    pub markers: &'a mut Markers,
    /// Where the moon is now.
    pub moon: DVec3,
}

impl<'a> Lent<'a> {
    fn again(&mut self) -> Lent<'_> {
        Lent {
            session: self.session,
            events: self.events,
            generator: self.generator,
            cells: self.cells,
            parts: self.parts,
            trunks: self.trunks,
            history: self.history,
            controller: self.controller,
            level: self.level,
            pointer: self.pointer,
            controls: self.controls,
            carts: self.carts,
            live: self.live,
            passage: self.passage,
            played: self.played,
            clock_s: self.clock_s,
            ways: self.ways,
            things: self.things,
            hand: self.hand,
            peers: self.peers,
            animals: self.animals,
            pets: self.pets,
            drawn: self.drawn,
            offered: self.offered,
            markers: self.markers,
            moon: self.moon,
        }
    }

    pub(crate) fn host(self, plugin: &'static str) -> Host<'a> {
        Host { plugin, lent: self }
    }
}

/// What the core offers a plugin while it handles a command, a message, a
/// key or its turn.
pub struct Host<'a> {
    plugin: &'static str,
    lent: Lent<'a>,
}

impl Host<'_> {
    /// Asks the world for an op of this plugin: the payload rides the core's
    /// envelope under the plugin's name. Nothing goes out while offline.
    pub fn send(&mut self, kind: &str, payload: Vec<u8>) {
        self.lent.session.envelope(self.plugin, kind, payload);
    }

    /// Sends bytes to the sessions that hold the plugin's `thing` together
    /// with this one, as the plugin's world half linked them: each hears
    /// them, this client too ([`Plugin::relayed`]), in the one order the
    /// world takes them in. The world reads none of it, and drops what this
    /// session sends past the wire's rate. Nothing goes out while offline.
    pub fn relay(&mut self, thing: u64, payload: Vec<u8>) {
        self.lent.session.relay(self.plugin, thing, payload);
    }

    /// This client's session in the world, while online.
    pub fn session(&self) -> Option<u32> {
        match self.lent.session.link() {
            crate::session::Link::Online { session } => Some(session),
            _ => None,
        }
    }

    /// Says an event of this plugin over the seam. `body` is an object, and
    /// its `type` becomes the plugin's name and `kind`: `chat.said`.
    pub fn emit<T: Serialize>(&mut self, kind: &str, body: &T) {
        match serde_json::to_value(body) {
            Ok(Value::Object(mut event)) => {
                event.insert("type".into(), format!("{}.{kind}", self.plugin).into());
                self.lent.events.push(Event::Plugin(Value::Object(event)));
            }
            _ => self.reject(format!("the event `{kind}` is no object")),
        }
    }

    /// Refuses what was asked. The message is for logs, not for people.
    pub fn reject(&mut self, message: String) {
        self.lent.events.push(Event::Rejected {
            message: format!("{}: {message}", self.plugin),
        });
    }

    /// A stance as a place: what a person reads out and `GoTo` takes. `None`
    /// for a stance that names nowhere on this body.
    pub fn place(&self, stance: &protocol::Stance) -> Option<String> {
        let grid = self.sphere().blocks();
        Pose::of_stance(grid, stance).map(|pose| pose.place(grid))
    }

    /// A part's anchor as a place: the code `GoTo` takes to stand there,
    /// facing the way the part's box points, its `+y` side, when it points
    /// somewhere on the ground. On the ground the height is left out, as a
    /// stance's is; over it by more than a step, a tower's top, it is
    /// kept, so arriving there arrives flying, onto the floor.
    pub fn place_at(&self, seat: Seat, pose: &PartPose) -> String {
        self.lent.parts.place_at(self.lent.generator, seat, pose)
    }

    /// Takes the body through a portal to a place: the body falls into the
    /// portal, the picture drawn into it, and lands at the place a moment
    /// later, the picture bursting out once the ground there has come. A
    /// body held in that portal by [`Host::fall_into`] goes on from there.
    /// Wrong, the text says what is wrong and the body stays.
    pub fn go_through(&mut self, place: &str, portal: PartId) -> Result<(), String> {
        let grid = self.sphere().blocks();
        Pose::parse(grid, place)?;
        Passage::lead(
            self.lent.passage,
            place.to_owned(),
            Some(portal),
            self.lent.clock_s,
        );
        Ok(())
    }

    /// Takes the body into a portal before the place it leads to is known:
    /// it falls in at once and is held there, the picture drawn into it,
    /// until [`Host::go_through`] says the place or [`Host::let_out`] that
    /// there is none.
    pub fn fall_into(&mut self, portal: PartId) {
        *self.lent.passage = Some(Passage::new(None, Some(portal), self.lent.clock_s));
    }

    /// Lets out where it fell in a body held in a portal that leads
    /// nowhere after all.
    pub fn let_out(&mut self) {
        if let Some(passage) = self.lent.passage {
            passage.let_out(self.lent.clock_s);
        }
    }

    /// Where the centre of a body is this frame, from the planet's: the
    /// moon moves, so what is measured on it is measured from here.
    pub fn body_centre(&self, body: Body) -> DVec3 {
        self.lent.cells.centre(body)
    }

    /// The body of the world: how an address is a place on it.
    pub fn sphere(&self) -> QuadSphere {
        self.lent.generator.sphere()
    }

    /// How high the ground of a seat stands at a column of it, in blocks:
    /// the ground in full detail, as a body stands on it.
    pub fn ground(&self, seat: Seat, point: SurfacePoint) -> f64 {
        cells::ground(self.lent.generator, seat, point)
    }

    /// This client's own body in the world: where it is, from the planet's
    /// centre, which way is up for it, and which way it faces, both unit.
    pub fn body(&self) -> Standing {
        let controller = &self.lent.controller;
        Standing {
            position: controller.position(),
            up: controller.up(),
            facing: controller.facing(),
        }
    }

    /// Where the feet of this client's own body are.
    pub fn feet(&self) -> Feet {
        let controller = &self.lent.controller;
        let (seat, point) = controller.over(self.lent.generator);
        Feet {
            seat,
            point,
            height_m: controller.height_m(),
        }
    }

    /// Lifts the body onto a floor `floor_m` over the datum, when it stands
    /// lower.
    pub fn lift(&mut self, floor_m: f64) {
        self.lent.controller.lift_onto(floor_m);
    }

    /// Puts this client's body in a way of moving this plugin gave, by its
    /// name, at rest; or, with `None`, takes it out of one, on foot, stepped
    /// down off what it rode. False when the plugin gave no such way, when
    /// the way carries (it is mounted: [`Host::mount`]), when the body is
    /// off the planet or under its ground, when it is aloft on what it
    /// rides (it lands first: [`Host::land`]), or, to leave, when it is in
    /// no way of this plugin's.
    pub fn set_way(&mut self, name: Option<&str>) -> bool {
        if self.lent.controller.aloft() {
            return false;
        }
        let Some(name) = name else {
            let inside = self.way().is_some();
            if inside {
                let built = Built {
                    cells: self.lent.cells,
                    parts: self.lent.parts,
                    trunks: self.lent.trunks,
                };
                self.lent.controller.step_off(self.lent.generator, built);
            }
            return inside;
        };
        let full = format!("{}.{name}", self.plugin);
        if self.lent.controller.way() == Some(full.as_str()) {
            return true;
        }
        match self.lent.ways.get(&full) {
            Some((_, way)) => {
                let generator = self.lent.generator;
                self.lent
                    .controller
                    .take_way(&full, &way.profile, way.steering, 0, generator)
            }
            None => false,
        }
    }

    /// Whether this client's body is aloft on what it rides: flying on an
    /// animal's wings.
    pub fn aloft(&self) -> bool {
        self.lent.controller.aloft()
    }

    /// Brings this client's body down from flight on what it rides, onto
    /// the ground under it. False over water deeper than its way wades,
    /// where it stays aloft and the readout says why; true on the ground.
    pub fn land(&mut self) -> bool {
        let built = Built {
            cells: self.lent.cells,
            parts: self.lent.parts,
            trunks: self.lent.trunks,
        };
        self.lent
            .controller
            .land(self.lent.generator, built)
            .is_ok()
    }

    /// Puts this client's body in a way of moving this plugin gave, by its
    /// name, on an animal drawn near it, by its number: the body walks to
    /// where the animal stands, faces its way, and rides it. False when the
    /// plugin gave no such way, when no such animal is drawn free, or when
    /// the body may not reach it or take the way there
    /// ([`Controller::mount`]).
    pub fn mount(&mut self, name: &str, animal: u64) -> bool {
        let full = format!("{}.{name}", self.plugin);
        let (Some((_, way)), Some(near)) = (
            self.lent.ways.get(&full),
            self.animals()
                .iter()
                .copied()
                .find(|near| near.number == animal && !near.ridden),
        ) else {
            return false;
        };
        let built = Built {
            cells: self.lent.cells,
            parts: self.lent.parts,
            trunks: self.lent.trunks,
        };
        self.lent.controller.mount(
            &full,
            &way.profile,
            way.steering,
            &near,
            self.lent.generator,
            built,
        )
    }

    /// Steps the body on foot to `at`, world space, and turns it and its
    /// view to `facing`: how a body takes its place at a thing, a cabinet's
    /// station. Placed at once where a walk from where it stands reaches,
    /// the same for a person and an agent; false, and the body as it was,
    /// when it rides, flies or goes in a way, or something stops the walk.
    pub fn step_to(&mut self, at: DVec3, facing: DVec3) -> bool {
        let built = Built {
            cells: self.lent.cells,
            parts: self.lent.parts,
            trunks: self.lent.trunks,
        };
        self.lent
            .controller
            .step_to(at, facing, self.lent.generator, built)
    }

    /// The animals drawn near this client's body this frame, in no order:
    /// what a plugin offers to ride, and an agent reads.
    pub fn animals(&self) -> &[Near] {
        self.lent.animals.near()
    }

    /// This client's own pet, as the world told it.
    pub fn pet(&self) -> Option<&Pet> {
        self.lent.pets.mine()
    }

    /// The animal this client's body rides becomes its pet, in the place of
    /// the one it had, standing where it is now: asked of the world, which
    /// says so to every session. False when the body rides no such animal
    /// drawn, is aloft or in a jump, or no world is there to ask.
    pub fn tame(&mut self, animal: u64) -> bool {
        if animal == 0 || self.lent.controller.carrier() != animal || !self.grounded() {
            return false;
        }
        let near = self.animals().iter().find(|near| near.number == animal);
        let Some(&Near { at, heading, .. }) = near else {
            return false;
        };
        let now_ms = (self.lent.clock_s * 1000.0).max(0.0) as u64;
        let spot = animals::spot_of(at, heading, now_ms, self.lent.generator);
        self.lent.pets.tame(animal, spot)
    }

    /// Whether this client's body stands on what holds it, on its feet or
    /// a mount's: false aloft, in flight or in a fall.
    pub fn grounded(&self) -> bool {
        self.lent.controller.grounded()
    }

    /// This client's pet goes back to its herd: asked of the world, from
    /// its saddle or from afar. False when it has none, no world is there
    /// to ask, or the body rides it aloft: a pet is let go standing, where
    /// it walks back from.
    pub fn release(&mut self) -> bool {
        let Some(number) = self.lent.pets.mine().map(|pet| pet.number) else {
            return false;
        };
        let aloft_on_it = self.lent.controller.carrier() == number && !self.grounded();
        !aloft_on_it && self.lent.pets.release()
    }

    /// This client's body plays one of the figure's own clips over what it
    /// moves by, for a while: the same request as `Command::Play` at the
    /// seam, ended as well when this plugin rests. False for a length that
    /// is no length of time a clip plays.
    pub fn play(&mut self, play: Play) -> bool {
        let mode = self.lent.controller.mode();
        match Playing::new(play, self.lent.clock_s, Some(self.plugin), mode) {
            Some(playing) => {
                *self.lent.played = Some(playing);
                true
            }
            None => false,
        }
    }

    /// The way of moving this plugin gave that this client's body is in, by
    /// its name in the plugin.
    pub fn way(&self) -> Option<&'static str> {
        let (plugin, way) = self.lent.ways.get(self.lent.controller.way()?)?;
        (plugin == self.plugin).then_some(way.name)
    }

    /// Has the body's hand grasp one of the things this plugin gave, by its
    /// name in the plugin, or let go of the one of this plugin it grasps,
    /// with `None`. False when the plugin gave no such thing, or the hand
    /// grasps another plugin's. The hand keeps it across worlds, as the
    /// body keeps its way, until the plugin lets go or is switched off.
    pub fn grasp(&mut self, name: Option<&str>) -> bool {
        let others =
            self.lent.hand.as_deref().is_some_and(|hand| {
                given::split(hand).is_none_or(|(plugin, _)| plugin != self.plugin)
            });
        if others {
            return false;
        }
        let Some(name) = name else {
            *self.lent.hand = None;
            return true;
        };
        let full = given::full(self.plugin, name);
        if self.lent.things.get(&full).is_none() {
            return false;
        }
        *self.lent.hand = Some(full);
        true
    }

    /// The thing of this plugin the body's hand grasps, by its name in the
    /// plugin.
    pub fn grasping(&self) -> Option<&str> {
        let (plugin, name) = given::split(self.lent.hand.as_deref()?)?;
        (plugin == self.plugin).then_some(name)
    }

    /// Every body in a way of moving this plugin gave, this client's first:
    /// what the plugin draws under each.
    pub fn in_ways(&self) -> Vec<InWay> {
        let controller = &self.lent.controller;
        let own = self.way().map(|way| InWay {
            session: None,
            way,
            at: DAffine3::from_mat3_translation(
                controller.body_basis(),
                controller.drawn_position(),
            ),
            speed_mps: controller.ground_mps(),
            carrier: controller.carrier(),
        });
        let peers = self
            .lent
            .peers
            .in_ways(self.lent.moon, self.lent.ways)
            .filter(|(_, plugin, ..)| *plugin == self.plugin)
            .map(|(session, _, way, carrier, motion)| InWay {
                session: Some(session),
                way: way.name,
                at: DAffine3::from_mat3_translation(motion.basis, motion.position),
                speed_mps: motion.speed_mps,
                carrier,
            });
        own.into_iter().chain(peers).collect()
    }

    /// Draws boxes over the world, in place of what this plugin drew last,
    /// until it draws again or rests.
    pub fn draw(&mut self, parts: Vec<scene::BoxPart>) {
        let drawn = &mut *self.lent.drawn;
        drawn.retain(|(plugin, _)| *plugin != self.plugin);
        if !parts.is_empty() {
            drawn.push((self.plugin, parts));
        }
    }

    /// Offers actions for the prompt, in place of what this plugin offered
    /// last, until it offers again or rests: offered each turn, as it draws.
    /// The core picks one to show, and an act on it reaches
    /// [`Plugin::act`]. False when an offer has neither a tap nor a hold,
    /// or a ring that takes no time: that offer is left out.
    pub fn offer(&mut self, mut offers: Vec<Offer>) -> bool {
        let actable = |offer: &Offer| match offer.hold {
            Some(hold) => hold.ring_s.is_finite() && hold.ring_s > 0.0,
            None => offer.tap.is_some(),
        };
        let whole = offers.iter().all(actable);
        offers.retain(actable);
        let offered = &mut *self.lent.offered;
        let at = offered
            .iter()
            .position(|(plugin, _)| *plugin == self.plugin);
        match (at, offers.is_empty()) {
            (Some(at), false) => offered[at].1 = offers,
            (Some(at), true) => {
                offered.remove(at);
            }
            (None, false) => offered.push((self.plugin, offers)),
            (None, true) => {}
        }
        whole
    }

    /// Points things out in the world, in place of what this plugin pointed
    /// out last, until it does again or rests: given each turn, as it
    /// offers. The core shows the nearest, wherever each is; a front end
    /// draws them and sends a choice's command when it is pressed. Only a
    /// plugin's first [`crate::marker::SHOWN`] are kept.
    pub fn markers(&mut self, markers: Vec<Marker>) {
        self.lent.markers.give(self.plugin, markers);
    }

    /// Where a point of the world is, as feet standing there: the seat under
    /// it, the column of it there, and its height over that body's datum.
    /// On the moon when it is within the moon's reach, else on the planet.
    pub fn feet_at(&self, point: DVec3) -> Feet {
        let generator = self.lent.generator;
        let moon = self.lent.cells.centre(Body::Moon);
        let body = match (point - moon).length() < MOON_REACH_M {
            true => Body::Moon,
            false => Body::Planet,
        };
        let sphere = seat::sphere_of(generator, body);
        let from_centre = point - self.lent.cells.centre(body);
        let point = sphere.blocks().surface_point(from_centre.to_array());
        Feet {
            seat: Seat::on(body, point.sector),
            point,
            height_m: from_centre.length() - sphere.radius_m(),
        }
    }

    /// Where a line of sight from a point of the world along a direction,
    /// unit, goes into the ground of a body within `reach_m`, as feet
    /// standing there, and how far along, metres. `None` when it meets no
    /// ground that far.
    pub fn ground_hit(&self, from: DVec3, toward: DVec3, reach_m: f64) -> Option<(Feet, f64)> {
        let generator = self.lent.generator;
        let bodies = [Body::Planet, Body::Moon].into_iter().filter(|&body| {
            // A line nowhere near a body meets no ground of it.
            let centre = self.lent.cells.centre(body);
            let radius_m = seat::sphere_of(generator, body).radius_m();
            let to = centre - from;
            let along = to.dot(toward).clamp(0.0, reach_m);
            (to - toward * along).length() < radius_m + MOON_REACH_M
        });
        let hits = bodies.filter_map(|body| {
            let sphere = seat::sphere_of(generator, body);
            let from = from - self.lent.cells.centre(body);
            let under = |at: DVec3| {
                let direction = at.normalize_or(DVec3::Y).to_array();
                at.length() - sphere.radius_m() < seat::ground_m(generator, body, direction)
            };
            let hit_m = collision::ground_run_m(from, toward, reach_m, under)?;
            let at = from + toward * hit_m;
            let point = sphere.blocks().surface_point(at.to_array());
            let feet = Feet {
                seat: Seat::on(body, point.sector),
                point,
                height_m: at.length() - sphere.radius_m(),
            };
            Some((hit_m, feet))
        });
        hits.min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(away_m, feet)| (feet, away_m))
    }

    /// Takes the pointer, or lets it go: with it, the pointer stays free to
    /// aim and its button is this plugin's. One plugin has it at a time, the
    /// last to take it.
    pub fn point(&mut self, on: bool) {
        if on {
            *self.lent.pointer = Some(self.plugin);
        } else if self.pointing() {
            *self.lent.pointer = None;
            self.lent.controller.pass_cells(false);
        }
    }

    /// While this plugin has the pointer, lets this client's own body fly
    /// through the cells, or has them hold it again: a builder flies through
    /// what they build. Letting the pointer go ends it.
    pub fn pass(&mut self, on: bool) {
        if self.pointing() {
            self.lent.controller.pass_cells(on);
        }
    }

    /// Whether this plugin has the pointer.
    pub fn pointing(&self) -> bool {
        *self.lent.pointer == Some(self.plugin)
    }

    /// Takes the body's controls, from whoever held them, and with a look
    /// the eye: the body stands where it is and takes no movement, what the
    /// person presses comes in this plugin's turn ([`Turn::controls`]), and
    /// the eye eases to `look`. Asked again, the look follows. False while
    /// a passage takes the body, which nothing holds, and for a look that is
    /// no place.
    pub fn hold_controls(&mut self, look: Option<Look>) -> bool {
        self.lent.passage.is_none() && self.lent.controls.lend(self.plugin, look)
    }

    /// Gives the controls back, when this plugin holds them: the body moves
    /// again at once, and the eye eases back.
    pub fn let_controls_go(&mut self) {
        if self.holds_controls() {
            self.lent.controls.take_back();
        }
    }

    /// Whether this plugin holds the controls. They are taken back without
    /// asking, by the key that leaves or a passage, so a holder reads this
    /// in its turn.
    pub fn holds_controls(&self) -> bool {
        self.lent.controls.holder() == Some(self.plugin)
    }

    fn game_of(&self, id: PartId) -> Option<crate::carts::GameId> {
        crate::carts::game_of(self.lent.parts, id)
    }

    /// Runs the game of the cabinet `id`, asked every frame someone plays
    /// it: a cabinet nobody plays shows its game's cover, still, and runs
    /// nothing. Asked no more, its player left: the game waits for them
    /// where it stopped ([`crate::GAME_WAITS_S`]), and asked again within that,
    /// it goes on from there.
    pub fn play_game(&mut self, id: PartId) {
        if let Some(key) = self.game_of(id) {
            self.lent.carts.play(key);
        }
    }

    /// Whether the cabinet `id` has a game to play: one is in it, it did
    /// not stop for good, and its file came. A game stopped says why on
    /// its screen.
    pub fn game_playable(&self, id: PartId) -> bool {
        self.game_of(id)
            .is_some_and(|key| self.lent.carts.playable(key))
    }

    /// Whether the game of the cabinet `id` waits for the player who left
    /// it: playing it again goes on where it stopped.
    pub fn game_waits(&self, id: PartId) -> bool {
        self.game_of(id)
            .is_some_and(|key| self.lent.carts.waits(key))
    }

    /// Holds `pad` down on the game of the cabinet `id` for this frame: its
    /// buttons are the pad's, the stick its arrows. Asked every frame they
    /// are held; let go the first frame nobody asks. A game fed is never
    /// pressed: it is given every pad's buttons ([`Host::feed_game`]).
    pub fn press_game(&mut self, id: PartId, pad: Pad) {
        if let Some(key) = self.game_of(id) {
            self.lent.carts.press(key, pad);
        }
    }

    /// The pads the game of the cabinet `id` plays, once its file was read
    /// for its cover: a game of more than one is fed.
    pub fn game_plays(&self, id: PartId) -> Option<usize> {
        self.game_of(id).and_then(|key| self.lent.carts.plays(key))
    }

    /// The updates the game of the cabinet `id` was given since it opened,
    /// fed, while it runs here: `None` until it does, and for a game not
    /// fed.
    pub fn game_given(&self, id: PartId) -> Option<u64> {
        self.game_of(id)
            .and_then(|key| self.lent.carts.given(key, self.lent.live))
    }

    /// Gives the game fed of the cabinet `id` its next updates' buttons, an
    /// update's word each, every pad's: it runs them at its pace, or
    /// `at_once` with none of their sound, and holds where they end.
    /// Nothing while [`Host::game_given`] says nothing.
    pub fn feed_game(&mut self, id: PartId, buttons: &[u32], at_once: bool) {
        if let Some(key) = self.game_of(id) {
            self.lent.carts.feed(key, buttons, at_once, self.lent.live);
        }
    }

    /// Runs the game of the cabinet `id` again from its start, as a
    /// console's reset does: it stops as the frame ends, its cover shows,
    /// and it opens again from its seed while it is played, a game fed
    /// given nothing.
    pub fn restart_game(&mut self, id: PartId) {
        if let Some(key) = self.game_of(id) {
            self.lent.carts.restart(key);
        }
    }

    /// The cells, to read: what a volume holds, what stands where.
    pub fn cells(&self) -> &Cells {
        self.lent.cells
    }

    /// A line of sight from a point of the world along a direction, unit,
    /// bent into the cells, and what it meets first: a cell, the ground of a
    /// volume, or a side of `room`, the box a hand builds in, where the line
    /// leaves it.
    pub fn sight(&self, from: DVec3, toward: DVec3, room: Option<(Seat, Span)>) -> Sight {
        self.lent
            .cells
            .sight(self.lent.generator, from, toward, room)
    }

    /// Where a lattice point of a seat's cells is in the world.
    pub fn corner(&self, seat: Seat, p: [i32; 3]) -> DVec3 {
        self.lent.cells.corner(self.lent.generator, seat, p)
    }

    /// Whether this session may change the cells: a builder's and an
    /// admin's, in a world that said so. The server answers the same when a
    /// gesture is an op.
    pub fn may_change(&self) -> bool {
        self.lent.level.is_some_and(|level| level >= Level::Builder)
    }

    fn permitted(&self) -> Result<(), Refusal> {
        match self.may_change() {
            true => Ok(()),
            false => Err(Refusal::Level),
        }
    }

    /// The cells the volume of the plot a column of a seat is on holds, or
    /// would hold once opened: the room there is to build in. Why none can
    /// stand there, where none can.
    pub fn room(&self, seat: Seat, point: SurfacePoint) -> Result<Span, Refusal> {
        self.lent.cells.room(self.lent.generator, seat, point)
    }

    /// Opens the volume of the plot a column of a seat is on, where none
    /// stands: room to build in, with nothing built. A stroke starts on its
    /// ground.
    pub fn open(&mut self, seat: Seat, point: SurfacePoint) -> Result<(), Refusal> {
        self.permitted()?;
        self.lent.cells.open(self.lent.generator, seat, point)
    }

    /// A change of the cells landed here: it enters the one history.
    fn did_cells(&mut self) {
        let ask = self.lent.cells.last_ask();
        self.lent.history.did(Owner::Cells, ask);
    }

    /// Closes the volume over a column of a seat: what was built in it goes
    /// with it, and its plot is nature again. A change like any other, taken
    /// back as one. `Ok(false)` where no volume stands.
    pub fn close(&mut self, seat: Seat, point: SurfacePoint) -> Result<bool, Refusal> {
        self.permitted()?;
        let closed = self.lent.cells.close(seat, point);
        if closed {
            self.did_cells();
        }
        Ok(closed)
    }

    /// Applies gestures to the cells of a seat as one change, taken back as
    /// one. `Ok(false)` when no cell changed.
    pub fn apply(&mut self, seat: impl Into<Seat>, gestures: &[Gesture]) -> Result<bool, Refusal> {
        self.permitted()?;
        let changed = self.lent.cells.apply(self.lent.generator, seat, gestures);
        if changed {
            self.did_cells();
        }
        Ok(changed)
    }

    /// Shows the ghost of a gesture over a seat's cells, or of none: exactly
    /// the cells it would change.
    pub fn preview(&mut self, gesture: Option<(Seat, Gesture)>) {
        self.lent.cells.preview(gesture);
    }

    /// Shows guides over the world, in place of those shown before, and
    /// none with none to show: each a box of a seat's cells as a faint grid.
    pub fn guide(&mut self, guides: &[Guide]) {
        self.lent.cells.guide(self.lent.generator, guides);
    }

    /// The parts, to read: what stands where.
    pub fn parts(&self) -> &Parts {
        self.lent.parts
    }

    /// Asks for the chart: the landmarks of the world, wherever they stand.
    /// In a world the world answers; alone, what is held here does, at
    /// once. The answer comes through [`Host::take_charted`].
    pub fn chart(&mut self) {
        self.lent.parts.chart();
    }

    /// Makes a landmark one of this person's favourites, or no longer one:
    /// an account's are asked of the world, a visitor's are kept by its
    /// front end. The next chart says it.
    pub fn favour(&mut self, id: u64, on: bool) {
        self.lent.parts.favour(id, on);
    }

    /// Makes a landmark this person's home, in the place of the last, or
    /// clears it with `None`, as [`Host::favour`] keeps a favourite.
    pub fn home(&mut self, id: Option<u64>) {
        self.lent.parts.home(id);
    }

    /// The chart's answer since last taken: the home first, then the
    /// favourites, then the rest, each the nearest the body first.
    pub fn take_charted(&mut self) -> Option<Vec<Landmarked>> {
        self.lent
            .parts
            .take_charted(self.lent.generator, self.lent.controller.held_by())
    }

    /// The nearest part a line from a point of the world along a direction,
    /// unit, meets, and how far along, metres.
    pub fn pick(&self, from: DVec3, toward: DVec3) -> Option<(PartId, f64)> {
        self.lent.parts.pick(self.lent.generator, from, toward)
    }

    /// The first part a line of the world meets, and the side it meets.
    pub fn hit_part(&self, from: DVec3, toward: DVec3) -> Option<PartHit> {
        self.lent
            .parts
            .hit(self.lent.generator, from, toward)
            .map(|(id, away_m, outward, side)| PartHit {
                id,
                away_m,
                outward,
                side,
            })
    }

    /// Whether this session may change the parts, in the parts' words.
    fn may_place(&self) -> Result<(), PartRefusal> {
        match self.may_change() {
            true => Ok(()),
            false => Err(PartRefusal::Level),
        }
    }

    /// The world from a part's own space, from the planet's centre: where
    /// it stands this frame. `None` for a name nothing holds.
    pub fn part_transform(&self, id: PartId) -> Option<DAffine3> {
        self.lent.parts.transform(self.lent.generator, id)
    }

    /// A pose said on a seat, settled on the sector its anchor stands over:
    /// as said while that is its own, else read back onto the next as a
    /// gizmo's drag is. `None` for a pose that is none.
    pub fn settle(&self, seat: Seat, pose: &PartPose) -> Option<(Seat, PartPose)> {
        self.lent.parts.settle(self.lent.generator, seat, pose)
    }

    /// The way a body arriving at a part faces, by its name: what a place
    /// at it carries, so a hand shows the arrival as it will be. `None` for
    /// a name nothing holds, or a part that points nowhere on the ground.
    pub fn facing(&self, id: PartId) -> Option<Facing> {
        let (seat, part) = self.lent.parts.get(id)?;
        self.lent
            .parts
            .facing(self.lent.generator, seat, &part.pose)
    }

    /// The seat and pose a part standing in the world would have, on a
    /// body: how a part is moved. A gizmo changes the world transform and
    /// asks here what pose that is. `None` for a transform that is no pose.
    pub fn reseat(&self, body: Body, world: &DAffine3) -> Option<(Seat, PartPose)> {
        self.lent.parts.reseat(self.lent.generator, body, world)
    }

    /// Shows marks over the world, in place of those shown before, and none
    /// with none to show: cubes in a paint drawn over everything, for a hand
    /// to see.
    pub fn mark(&mut self, marks: &[Mark]) {
        self.lent.parts.mark(marks);
    }

    /// Whether a pose is one a part on a seat can have: sound, and inside
    /// the seat's sector.
    fn sound(&self, seat: Seat, part: &Part) -> Result<(), PartRefusal> {
        let pose = &part.pose;
        match part.is_sound() && seat::inside(seat.sphere(self.lent.generator), pose) {
            true => Ok(()),
            false => Err(PartRefusal::Pose),
        }
    }

    /// Places a part on a seat, where its pose says: a change like any
    /// other. The name it is given.
    pub fn place_part(&mut self, seat: Seat, part: Part) -> Result<PartId, PartRefusal> {
        self.may_place()?;
        self.sound(seat, &part)?;
        let id = self.lent.parts.place(self.lent.generator, seat, part);
        let ask = self.lent.parts.last_ask();
        self.lent.history.did(Owner::Parts, ask);
        Ok(id)
    }

    /// Sets a part by its name: moved, turned, stretched, repainted, made
    /// solid or not, or seated elsewhere.
    pub fn set_part(&mut self, id: PartId, seat: Seat, part: Part) -> Result<(), PartRefusal> {
        self.may_place()?;
        self.sound(seat, &part)?;
        match self.lent.parts.set(self.lent.generator, id, seat, part) {
            true => {
                // A set goes to the world at a cadence, under no number yet.
                self.lent.history.set(id.0, None);
                Ok(())
            }
            false => Err(PartRefusal::Gone),
        }
    }

    /// Asks for a model's file, once, so a hand knows its size before it
    /// places it: a model of voxels or a 3D model. What arrives is kept and
    /// drawn for every part that names it.
    pub fn want_model(&mut self, shape: Shape) {
        self.lent.parts.want(shape);
    }

    /// A model's size as a hand places it, in blocks, once its file
    /// arrived, or why it is not drawn. `None` while it is on its way.
    pub fn model_size(&self, shape: Shape) -> Option<Result<[f64; 3], ModelRefusal>> {
        self.lent.parts.size_of(shape)
    }

    /// Removes a part by its name.
    pub fn remove_part(&mut self, id: PartId) -> Result<(), PartRefusal> {
        self.may_place()?;
        match self.lent.parts.remove(id) {
            true => {
                let ask = self.lent.parts.last_ask();
                self.lent.history.did(Owner::Parts, ask);
                Ok(())
            }
            false => Err(PartRefusal::Gone),
        }
    }

    /// Whether there is a change to take back, and one to put back, of
    /// whatever owner.
    pub fn history(&self) -> (bool, bool) {
        self.lent.history.state()
    }

    /// Takes back the last change that landed, whatever owner it was of.
    pub fn take_back(&mut self) -> Result<(), Refusal> {
        self.permitted()?;
        match self.lent.history.take_back() {
            Some(Owner::Cells) => self.lent.cells.take_back(self.lent.generator),
            Some(Owner::Parts) => self.lent.parts.restore(self.lent.generator, true),
            None => {}
        }
        Ok(())
    }

    /// Puts back the last change taken back.
    pub fn put_back(&mut self) -> Result<(), Refusal> {
        self.permitted()?;
        match self.lent.history.put_back() {
            Some(Owner::Cells) => self.lent.cells.put_back(self.lent.generator),
            Some(Owner::Parts) => self.lent.parts.restore(self.lent.generator, false),
            None => {}
        }
        Ok(())
    }
}

struct Plugged {
    plugin: Box<dyn Plugin>,
    /// Whether the world this client is in has it on, at this version.
    on: bool,
    /// The keys it asked for when it was plugged in.
    keys: Vec<KeyAsk>,
}

/// Every plugin a shell plugged in, in the order it did.
#[derive(Default)]
pub(crate) struct Plugins {
    held: Vec<Plugged>,
}

impl Plugins {
    pub(crate) fn plug(&mut self, plugin: Box<dyn Plugin>) {
        let keys = plugin.keys();
        self.held.push(Plugged {
            plugin,
            on: false,
            keys,
        });
    }

    /// Takes the world's statement: a plugin is on when the world says its
    /// name at the version held here, and one that was on and is no longer
    /// rests first. Returns what is on now, and a line for the log about
    /// each one the world speaks in another version.
    pub(crate) fn speak(
        &mut self,
        spoken: &[protocol::Plugin],
        mut lent: Lent<'_>,
    ) -> (Vec<PluginOn>, Vec<String>) {
        let mut apart = Vec::new();
        for held in &mut self.held {
            let said = spoken.iter().find(|said| said.name == held.plugin.name());
            let on = said.is_some_and(|said| said.version == held.plugin.version());
            if held.on && !on {
                let mut host = lent.again().host(held.plugin.name());
                held.plugin.off(&mut host);
                host.grasp(None);
                rest(held, lent.again());
            }
            held.on = on;
            if let Some(said) = said.filter(|_| !on) {
                apart.push(format!(
                    "the world speaks {} {}, this client {}",
                    said.name,
                    said.version,
                    held.plugin.version()
                ));
            }
        }
        (self.on(), apart)
    }

    /// No world has spoken: every plugin is off.
    pub(crate) fn hush(&mut self, lent: Lent<'_>) -> Vec<PluginOn> {
        self.speak(&[], lent).0
    }

    /// Stands in for a world's word: every plugin plugged in is on, at the
    /// version it holds.
    pub(crate) fn rehearse(&mut self) -> Vec<PluginOn> {
        for held in &mut self.held {
            held.on = true;
        }
        self.on()
    }

    /// The world every plugin worked in is another now: each one that is on
    /// rests, and stays on.
    pub(crate) fn rest(&mut self, mut lent: Lent<'_>) {
        for held in self.held.iter_mut().filter(|held| held.on) {
            rest(held, lent.again());
        }
    }

    fn on(&self) -> Vec<PluginOn> {
        self.held
            .iter()
            .filter(|held| held.on)
            .map(|held| PluginOn {
                name: held.plugin.name().to_owned(),
                version: held.plugin.version(),
            })
            .collect()
    }

    /// Hands a command to the plugin it names. False when no plugin of that
    /// name is on.
    pub(crate) fn command(
        &mut self,
        name: &str,
        kind: &str,
        body: Value,
        mut lent: Lent<'_>,
    ) -> bool {
        let Some(held) = self.find(name) else {
            return false;
        };
        let before = *lent.pointer;
        let mut host = lent.again().host(held.plugin.name());
        held.plugin.command(kind, body, &mut host);
        self.handed(before, lent);
        true
    }

    /// Hands a message from the server to the plugin its envelope names. One
    /// for a plugin that is off, or that nobody plugged in, is let pass.
    pub(crate) fn receive(&mut self, envelope: &protocol::Envelope, mut lent: Lent<'_>) {
        if let Some(held) = self.find(&envelope.plugin) {
            let before = *lent.pointer;
            let mut host = lent.again().host(held.plugin.name());
            held.plugin
                .receive(&envelope.kind, &envelope.payload, &mut host);
            self.handed(before, lent);
        }
    }

    /// Hands bytes relayed about a plugin's thing to the plugin it names. One
    /// for a plugin that is off, or that nobody plugged in, is let pass.
    pub(crate) fn relayed(&mut self, relayed: &protocol::Relayed, mut lent: Lent<'_>) {
        if let Some(held) = self.find(&relayed.plugin) {
            let before = *lent.pointer;
            let mut host = lent.again().host(held.plugin.name());
            let relayed = Relayed {
                thing: relayed.thing,
                session: relayed.session,
                payload: &relayed.payload,
                clock_ms: relayed.clock_ms,
            };
            held.plugin.relayed(relayed, &mut host);
            self.handed(before, lent);
        }
    }

    /// Tells every plugin that is on that a session left the world.
    pub(crate) fn left(&mut self, session: u32, mut lent: Lent<'_>) {
        let before = *lent.pointer;
        for held in self.held.iter_mut().filter(|held| held.on) {
            let mut host = lent.again().host(held.plugin.name());
            held.plugin.left(session, &mut host);
        }
        self.handed(before, lent);
    }

    /// Whether a plugin that is on asks for a key with what is held with it:
    /// a shell keeps that key from whatever else would take it.
    pub(crate) fn asks(&self, code: &str, chord: Chord) -> bool {
        self.held
            .iter()
            .filter(|held| held.on)
            .any(|held| held.keys.iter().any(|ask| ask.is(code, chord)))
    }

    /// Hands each key that went down to the plugins that asked to hear it.
    pub(crate) fn keys(&mut self, pressed: &[(String, Chord)], mut lent: Lent<'_>) {
        let before = *lent.pointer;
        for (code, chord) in pressed {
            for held in self.held.iter_mut().filter(|held| held.on) {
                let asked = held
                    .keys
                    .iter()
                    .find(|ask| !ask.held && ask.is(code, *chord));
                if let Some(name) = asked.map(|ask| ask.name) {
                    let mut host = lent.again().host(held.plugin.name());
                    held.plugin.key(name, &mut host);
                }
            }
        }
        self.handed(before, lent);
    }

    /// Hands an act on one of its offers to the plugin that offered it.
    /// False when no plugin of that name is on.
    pub(crate) fn act(&mut self, name: &str, id: u64, act: Act, mut lent: Lent<'_>) -> bool {
        let Some(held) = self.find(name) else {
            return false;
        };
        let before = *lent.pointer;
        let mut host = lent.again().host(held.plugin.name());
        held.plugin.act(id, act, &mut host);
        self.handed(before, lent);
        true
    }

    /// Gives every plugin that is on its turn in the frame, and to the one
    /// the controls are lent to, what they press.
    pub(crate) fn turn(
        &mut self,
        eye: &Eye,
        input: &Input,
        pad: Option<Pad>,
        (interrupted, inspected): (bool, bool),
        mut lent: Lent<'_>,
    ) {
        let before = *lent.pointer;
        for held in self.held.iter_mut().filter(|held| held.on) {
            let name = held.plugin.name();
            let keys: Vec<&'static str> = held
                .keys
                .iter()
                .filter(|ask| ask.held && input.code_held(ask.code))
                .map(|ask| ask.name)
                .collect();
            let turn = Turn {
                eye: *eye,
                using: input.held(Key::Use) && *lent.pointer == Some(name),
                controls: pad.filter(|_| lent.controls.holder() == Some(name)),
                chord: input.chord,
                keys: &keys,
                interrupted,
                inspected,
                now_s: lent.clock_s,
            };
            held.plugin.turn(&turn, &mut lent.again().host(name));
        }
        self.handed(before, lent);
    }

    /// Whether a plugin that is on owes the world work.
    pub(crate) fn busy(&self) -> bool {
        self.held.iter().any(|held| held.on && held.plugin.busy())
    }

    /// Has every plugin that is on do at once the work it owes.
    pub(crate) fn settle(&mut self, mut lent: Lent<'_>) {
        let before = *lent.pointer;
        for held in self.held.iter_mut().filter(|held| held.on) {
            let mut host = lent.again().host(held.plugin.name());
            held.plugin.settle(&mut host);
        }
        self.handed(before, lent);
    }

    /// Has the plugin with the pointer put it down, with what it had in
    /// hand: what a front end asks when a panel of its own opens.
    pub(crate) fn put_down(&mut self, mut lent: Lent<'_>) {
        if let Some(name) = *lent.pointer
            && let Some(held) = self.find(name)
        {
            rest(held, lent.again());
        }
    }

    /// After the plugins acted: one plugin has the pointer at a time, so the
    /// one it was taken from rests, and the seam hears whose it is now.
    fn handed(&mut self, before: Option<&'static str>, mut lent: Lent<'_>) {
        let now = *lent.pointer;
        if before == now {
            return;
        }
        if let (Some(former), Some(_)) = (before, now)
            && let Some(held) = self.find(former)
        {
            let mut host = lent.again().host(held.plugin.name());
            held.plugin.rest(&mut host);
            host.preview(None);
            host.guide(&[]);
        }
        lent.events.push(Event::PointerChanged {
            plugin: now.map(str::to_string),
        });
    }

    fn find(&mut self, name: &str) -> Option<&mut Plugged> {
        self.held
            .iter_mut()
            .find(|held| held.on && held.plugin.name() == name)
    }
}

/// Has a plugin put down what it had in hand, and takes back from it what a
/// host lends one plugin at a time: the pointer, the controls, the ghost and
/// the guides. The body's hand keeps its thing, as the body keeps its way.
fn rest(held: &mut Plugged, lent: Lent<'_>) {
    let mut host = lent.host(held.plugin.name());
    let had_pointer = host.pointing();
    held.plugin.rest(&mut host);
    host.let_controls_go();
    host.draw(Vec::new());
    host.offer(Vec::new());
    host.markers(Vec::new());
    if host.pointing() {
        host.point(false);
        host.preview(None);
        host.guide(&[]);
    }
    if had_pointer {
        host.lent
            .events
            .push(Event::PointerChanged { plugin: None });
    }
}
