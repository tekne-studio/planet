//! The ride plugin's client half: a board and a horse a body rides. Each is
//! a way of moving the plugin gives the core as data, how it goes and the
//! pose a rider holds; the core moves the body in it and holds the pose.
//! The board hovers: the plugin draws the plank under every body on one,
//! this client's and each peer's, growing in with a flip as a body steps on
//! and shrinking away where it stepped off. The horse goes in paces: it is
//! an animal of a world with life, which the core draws and holds under its
//! rider; the plugin offers each one near to ride. One born with wings
//! flies: the core's walk and fly key takes off and lands it.
//!
//! While it rides, the prompt offers to leave what it rides: "Board [E]
//! Leave", "Horse [E] Leave"; aloft, a horse lands first: "Horse [E]
//! Land". On a horse, holding the key tames it, "Tame", the ring longer
//! the more restless its herd; on one's own pet standing, it releases it,
//! "Release". On foot, it offers each horse near that is nobody else's
//! pet, "Horse [E] Ride", held, by the horse's number; the core shows the
//! one before the body. The pet is the core's, kept by the world's life:
//! this plugin is the hand that asks for it. The board is the body's own
//! doing, `H`, its own key.
//! An agent mounts a horse as a person does, through its offer
//! (`{"type":"act","hold":true}`), and says for the rest:
//!
//! ```json
//! {"type":"ride.mount"}
//! {"type":"ride.leave"}
//! ```
//!
//! What the body does is told by the core: `mode_changed` says `ride.board`
//! or `ride.horse` while it rides, and `walk` once it leaves.
//!
//! While the body has a pet and is not on it, the plugin points it out with
//! a marker: its name, how far, and two choices, Call, the whistle, and Let
//! go, `{"type":"ride.release"}`, asked first.

use client::{
    Act, Backing, Flight, Gait, Held, Hold, Host, Hover, Jump, Keeping, KeyAsk, Marked, Marker,
    MarkerChoice, Near, Offer, On, Pace, Paces, Pet, Play, Plugin, Profile, Steering, Turn, Way,
};
use gaia::fauna::horse::HORSE as HORSE_KIND;
use glam::{DAffine3, DQuat, DVec3, Quat, Vec3};
use scene::BoxPart;
use serde_json::Value;

/// The plugin's name, its version and the names of its ways are said once,
/// by its world half.
pub use ride_world::{BOARD, HORSE, NAME, VERSION};

/// `ride.mount`: onto the board, at rest, where the body stands. A horse
/// is mounted through its offer.
pub const MOUNT: &str = "mount";
/// `ride.leave`: off what the body rides, on foot.
pub const LEAVE: &str = "leave";
/// `ride.release`: the body's pet goes back to its herd, from its saddle
/// standing or from afar; what its marker's Let go sends.
pub const RELEASE: &str = "release";

/// The names of the keys it asks for.
pub mod key {
    /// Mounts the board, or leaves it.
    pub const BOARD: &str = "board";
}

/// The words of the horse's paces, slowest first, as its kind lists them.
const GAITS: [&str; 4] = ["walk", "trot", "gallop", "run"];
/// The word of a horse stepping backward.
const BACKING: &str = "back";
/// The words of a horse in flight, of one at the highest it flies, of one
/// refused a take-off for want of room over it or of wings, and of one
/// refused a landing over deep water.
const FLYING: &str = "fly";
const CEILING: &str = "ceiling";
const NO_ROOM: &str = "no_room";
const WINGLESS: &str = "wingless";
const NO_GROUND: &str = "no_ground";

/// How far the eye rolls with a horse banking on its wings, as a share of
/// its bank: a little, so the turn is felt and the horizon stays.
const BANK_EYE: f64 = 0.3;

/// How stiffly the eye comes after the ground under a horse, per second: a
/// touch slower than on foot, so its gait over rough ground sways the view.
const EYE_PER_S: f64 = 10.0;

/// A jump of the horse's kind, as the core takes it.
fn jump_of(jump: gaia::fauna::Jump) -> Jump {
    Jump {
        gather_s: jump.gather_s,
        height_m: jump.height_m,
        length_m: jump.length_m,
    }
}

/// How a horse goes under a rider: its kind's paces, named.
pub fn paces() -> Paces {
    let carry = HORSE_KIND.carry.expect("a horse carries a rider");
    let [ahead_m, behind_m] = HORSE_KIND.reach_m();
    Paces {
        paces: carry
            .paces
            .iter()
            .zip(GAITS)
            .map(|(pace, name)| Pace {
                name,
                speed_mps: pace.speed_mps,
                turn_per_s: pace.turn_per_s,
                steep: pace.steep,
                jump: pace.jump.map(jump_of),
            })
            .collect(),
        stepped: carry.stepped,
        sprint: carry.sprint,
        accel_mps2: carry.accel_mps2,
        decel_mps2: carry.decel_mps2,
        wade_m: carry.wade_m,
        room_m: carry.height_m,
        ahead_m,
        behind_m,
        eye_per_s: EYE_PER_S,
        standing: carry.standing.map(jump_of),
        backing: carry.backing.map(|backing| Backing {
            name: BACKING,
            from_mps: backing.from_mps,
            gain_mps2: backing.gain_mps2,
            top_mps: backing.top_mps,
            turn_per_s: backing.turn_per_s,
        }),
        flight: HORSE_KIND.wings.map(|winged| {
            let flight = winged.flight;
            Flight {
                name: FLYING,
                ceiling: CEILING,
                no_room: NO_ROOM,
                wingless: WINGLESS,
                no_ground: NO_GROUND,
                speed_mps: flight.speed_mps,
                per_m: flight.per_m,
                sprint: flight.sprint,
                top_mps: flight.top_mps,
                ceiling_m: flight.ceiling_m,
                takeoff_m: flight.takeoff_m,
                land_mps: flight.land_mps,
                land_per_m: flight.land_per_m,
                bank_eye: BANK_EYE,
            }
        }),
    }
}

/// How far a seated rider's thighs reach forward and apart, and how far
/// back the shins hang from them, radians.
const THIGH: f32 = 0.8;
const ASTRIDE: f32 = 0.42;
const SHIN: f32 = 1.0;

/// The pose a rider holds on a horse: astride, the thighs down and forward,
/// which the core turns out round the back it sits on, the shins hanging,
/// the back upright and the hands low before the belly on the reins. The
/// hips sit in the saddle: their height is how far over the soles they
/// stand in this pose.
pub fn seated() -> Held {
    let thigh = |side: f32| Quat::from_rotation_z(side * ASTRIDE) * Quat::from_rotation_x(THIGH);
    // At rest the arms are straight out to the sides: lowered most of the
    // way, a little forward, and the elbows bent so the hands meet low
    // before the belly.
    let arm = |side: f32| Quat::from_rotation_y(side * 0.5) * Quat::from_rotation_z(-side * 1.2);
    let forearm = |side: f32| Quat::from_rotation_y(side * 1.1);
    let shin = Quat::from_rotation_x(-SHIN);
    // Thigh and shin as long, each half the leg.
    let below = 0.5 * (THIGH.cos() + (THIGH - SHIN).cos()) * ASTRIDE.cos();
    Held {
        bones: vec![
            ("spine", Quat::from_rotation_x(0.05)),
            ("leftUpperLeg", thigh(-1.0)),
            ("leftLowerLeg", shin),
            ("rightUpperLeg", thigh(1.0)),
            ("rightLowerLeg", shin),
            ("leftUpperArm", arm(-1.0)),
            ("leftLowerArm", forearm(-1.0)),
            ("rightUpperArm", arm(1.0)),
            ("rightLowerArm", forearm(1.0)),
        ],
        hips: Vec3::new(0.0, below, 0.0),
    }
}

/// The words its offer says, keys of the panel's words: the thing, and
/// what a tap on it does.
pub mod word {
    pub const BOARD: &str = "board";
    pub const HORSE: &str = "horse";
    pub const RIDE: &str = "ride";
    pub const LEAVE: &str = "leave";
    pub const LAND: &str = "land";
    pub const TAME: &str = "tame";
    pub const RELEASE: &str = "release";
    pub const CALL: &str = "call";
    pub const LET_GO: &str = "let_go";
    /// What letting the pet go does, said before it is confirmed.
    pub const LET_GO_CONFIRM: &str = "let_go.confirm";
    pub const ON_ITS_WAY: &str = "on_its_way";
}

/// The icon the pet's marker is drawn with.
const PET_GLYPH: &str = "paw-print";
/// The kind of the pet's marker, the word a person hides it by.
const PET_KIND: &str = "pet";

/// Leaving what the body rides, or landing it aloft, as the prompt offers
/// it and an act names it. A horse is offered by its own number, which is
/// never zero.
pub const OFFER_LEAVE: u64 = 0;

/// How long the ring of taming takes on an animal of a middling herd,
/// seconds: half that on the calmest, half again on the most restless.
const TAME_S: f64 = 2.0;
/// How long the ring of releasing one's own pet takes, seconds.
const RELEASE_S: f32 = 1.5;

/// How high over a horse's feet its prompt hangs, past its saddle, metres.
const OVER_SADDLE_M: f64 = 0.4;

/// How the board hovers: half a metre over the ground or the sea, smoothed
/// over a few metres, as fast as a car, carving and leaning hard. The boost
/// speeds it on; a jump hops it a metre and a half, and held lifts it on up
/// to twenty metres over level ground, and it sinks back at a glider's
/// pace; it climbs mountainsides.
pub const HOVER: Hover = Hover {
    height_m: 0.5,
    top_mps: 40.0,
    sprint_mps: 70.0,
    accel_mps2: 16.0,
    turn_per_s: 2.4,
    lean_rad: 0.5,
    coast_per_s: 0.5,
    slope: 0.5,
    over_sea: true,
    bob_m: 0.03,
    bob_hz: 0.8,
    span_m: 1.5,
    footprint_m: 4.0,
    lift_mps: 10.0,
    sink_mps: 7.0,
    ceiling_m: 20.0,
    hop_m: 0.45,
    jump_m: 1.5,
};

/// How far the knees bend, radians from straight, and how far apart each
/// leg swings, radians from down: the feet half a metre apart.
const KNEES: f32 = 0.45;
const APART: f32 = 0.33;
/// The ankle's height over the sole, as a share of the hips' at rest.
const ANKLE: f32 = 0.08;

/// The pose a rider holds: sideways on the deck, left foot ahead, knees
/// bent, the shoulders and the head turned back toward where it goes, the
/// arms out for balance. Bones turn from rest, the figure's `x` right, `y`
/// up, `z` back.
pub fn stance() -> Held {
    let thigh = |side: f32| Quat::from_rotation_z(side * APART) * Quat::from_rotation_x(KNEES);
    let foot = |side: f32| Quat::from_rotation_x(KNEES) * Quat::from_rotation_z(-side * APART);
    Held {
        bones: vec![
            // Facing the right of the heading: the left foot leads.
            ("hips", Quat::from_rotation_y(-core::f32::consts::FRAC_PI_2)),
            (
                "spine",
                Quat::from_rotation_y(0.1) * Quat::from_rotation_x(-0.15),
            ),
            ("chest", Quat::from_rotation_y(0.2)),
            ("neck", Quat::from_rotation_y(0.35)),
            ("head", Quat::from_rotation_y(0.5)),
            ("leftUpperLeg", thigh(-1.0)),
            ("leftLowerLeg", Quat::from_rotation_x(-2.0 * KNEES)),
            ("leftFoot", foot(-1.0)),
            ("rightUpperLeg", thigh(1.0)),
            ("rightLowerLeg", Quat::from_rotation_x(-2.0 * KNEES)),
            ("rightFoot", foot(1.0)),
            ("leftUpperArm", Quat::from_rotation_z(0.6)),
            ("rightUpperArm", Quat::from_rotation_z(-0.6)),
        ],
        hips: Vec3::new(0.0, KNEES.cos() * APART.cos() * (1.0 - ANKLE) + ANKLE, 0.0),
    }
}

/// How long a body stepping on plays its fall onto the board, as into a
/// portal, seconds: while it hops on and the board grows in.
const FALL_S: f64 = 0.45;

/// How long a board takes to grow in under a body stepping on, and to
/// shrink away once it steps off, seconds.
const COME_S: f64 = 0.45;
const GO_S: f64 = 0.35;

/// The plugin: the boards it draws, under bodies on them and where bodies
/// stepped off, by the clock its last turn said.
#[derive(Default)]
pub struct Ride {
    under: Vec<Board>,
    left: Vec<Board>,
    now_s: f64,
    /// The pet's marker as last given, which stands until given again.
    marked: Option<Marker>,
}

/// A board drawn: under whose body, by session (`None` is this client's),
/// where it was drawn last, and since when it grows in or shrinks away.
struct Board {
    session: Option<u32>,
    at: DAffine3,
    since_s: f64,
}

/// The plugin as a shell plugs it in.
pub fn plugin() -> Box<dyn Plugin> {
    Box::new(Ride::default())
}

impl Plugin for Ride {
    fn name(&self) -> &'static str {
        NAME
    }

    fn version(&self) -> u32 {
        VERSION
    }

    fn command(&mut self, kind: &str, body: Value, host: &mut Host<'_>) {
        match (kind, body.get("way").and_then(Value::as_str)) {
            (MOUNT, None | Some(BOARD)) => mount(host),
            (MOUNT, Some(HORSE)) => host.reject(
                "a horse is mounted through its offer: {\"type\":\"act\",\"hold\":true}".into(),
            ),
            (MOUNT, Some(way)) => host.reject(format!("`{way}` is no way of ride")),
            (LEAVE, _) => leave(host),
            (RELEASE, _) => {
                if !host.release() {
                    host.reject("no pet to let go, or the body rides it aloft".into());
                }
            }
            _ => host.reject(format!("`{kind}` is no command of ride")),
        }
    }

    fn keys(&self) -> Vec<KeyAsk> {
        vec![KeyAsk::pressed(key::BOARD, "KeyH")]
    }

    fn key(&mut self, name: &str, host: &mut Host<'_>) {
        if name == key::BOARD {
            board(host);
        }
    }

    fn act(&mut self, id: u64, act: Act, host: &mut Host<'_>) {
        if id == OFFER_LEAVE && act == Act::Hold {
            hold(host);
        } else if id == OFFER_LEAVE && host.aloft() {
            // Over deep water it stays aloft, and the readout says why.
            host.land();
        } else if id == OFFER_LEAVE {
            host.set_way(None);
        } else if !host.mount(HORSE, id) {
            host.reject("this horse cannot be ridden from here".into());
        }
    }

    fn ways(&self) -> Vec<Way> {
        vec![
            Way {
                name: BOARD,
                profile: Profile::Hover(HOVER),
                steering: Steering::Steered,
                held: stance(),
            },
            Way {
                name: HORSE,
                profile: Profile::Paces(paces()),
                steering: Steering::Steered,
                held: seated(),
            },
        ]
    }

    /// Resting, what it gave is taken back: the marker is given again on
    /// its next turn.
    fn rest(&mut self, _host: &mut Host<'_>) {
        self.marked = None;
    }

    fn turn(&mut self, turn: &Turn<'_>, host: &mut Host<'_>) {
        self.now_s = turn.now_s;
        self.draw(host);
        // Riding, leaving what the body rides, or aloft landing it; on
        // foot, each free horse near, the core showing the one before the
        // body.
        let offers = match host.way() {
            Some(way) => {
                vec![Offer {
                    id: OFFER_LEAVE,
                    on: On::Body,
                    thing: if way == HORSE {
                        word::HORSE
                    } else {
                        word::BOARD
                    },
                    tap: Some(if host.aloft() {
                        word::LAND
                    } else {
                        word::LEAVE
                    }),
                    hold: (way == HORSE).then(|| held(host)).flatten(),
                }]
            }
            None => {
                let up = host.body().up;
                let carry = HORSE_KIND.carry.expect("a horse carries a rider");
                host.animals()
                    .iter()
                    .filter(|near| near.kind.number == HORSE_KIND.number && !near.ridden)
                    .filter(|near| near.keeping != Keeping::Anothers)
                    .map(|near| Offer {
                        id: near.number,
                        on: On::Point(near.at + up * (carry.saddle.y + OVER_SADDLE_M)),
                        thing: word::HORSE,
                        tap: None,
                        hold: Some(Hold {
                            verb: word::RIDE,
                            ring_s: carry.mount_s as f32,
                        }),
                    })
                    .collect()
            }
        };
        host.offer(offers);
        self.mark_pet(host, turn.now_s);
    }

    fn settle(&mut self, host: &mut Host<'_>) {
        self.draw(host);
    }
}

impl Ride {
    /// Points out the body's pet while it is not on it, giving the marker
    /// again only when what it says changed: it stands until then.
    fn mark_pet(&mut self, host: &mut Host<'_>, now_s: f64) {
        let shown = host
            .pet()
            .filter(|pet| ridden(host).is_none_or(|near| near.number != pet.number))
            .map(|pet| {
                // Called from afar, it is nowhere until it arrives.
                let arrives_s = pet.spot.at_ms as f64 / 1000.0;
                let coming = pet.from.is_some_and(|from| from.at_ms < pet.spot.at_ms);
                (pet, coming && now_s < arrives_s)
            });
        let same = match (&self.marked, &shown) {
            (Some(marked), Some((pet, on_its_way))) => {
                marked.id == pet.number
                    && marked.label == pet.name
                    && marked.note.is_some() == *on_its_way
            }
            (marked, shown) => marked.is_none() && shown.is_none(),
        };
        if same {
            return;
        }
        self.marked = shown.map(|(pet, on_its_way)| pet_marker(pet, on_its_way));
        host.markers(self.marked.iter().cloned().collect());
    }

    /// The plank under every body on a board, growing in under one that
    /// just stepped on, and shrinking away where one stepped off.
    fn draw(&mut self, host: &mut Host<'_>) {
        let now_s = self.now_s;
        let mut on = host.in_ways();
        on.retain(|body| body.way == BOARD);
        let (stay, left): (Vec<Board>, Vec<Board>) = self
            .under
            .drain(..)
            .partition(|board| on.iter().any(|body| body.session == board.session));
        self.left.extend(left.into_iter().map(|board| Board {
            since_s: now_s,
            ..board
        }));
        self.left.retain(|board| now_s - board.since_s < GO_S);
        self.under = on
            .iter()
            .map(|body| Board {
                session: body.session,
                at: body.at,
                since_s: stay
                    .iter()
                    .find(|board| board.session == body.session)
                    .map_or(now_s, |board| board.since_s),
            })
            .collect();

        let flicker = (now_s * 54.0).sin();
        let mut parts = Vec::new();
        for (body, board) in on.iter().zip(&self.under) {
            let grown = ((now_s - board.since_s) / COME_S).clamp(0.0, 1.0);
            parts.extend(plank(board.at, body.speed_mps, flicker, grown));
        }
        for board in &self.left {
            let grown = 1.0 - ((now_s - board.since_s) / GO_S).clamp(0.0, 1.0);
            parts.extend(plank(board.at, 0.0, flicker, grown));
        }
        host.draw(parts);
    }
}

/// How a board shows as it grows in, from nothing at 0 to whole at 1: it
/// swells past its size and back, as a thing conjured does, flipping over
/// along its length once, and glowing until it is whole. The turn about
/// the deck's middle, and how much it glows, 0 to 1.
pub fn appear(grown: f64) -> (DAffine3, f32) {
    let t = grown.clamp(0.0, 1.0);
    // Ease out with a swell past the end.
    let (c1, c3) = (1.70158, 2.70158);
    let scale = 1.0 + c3 * (t - 1.0).powi(3) + c1 * (t - 1.0).powi(2);
    let flip = (1.0 - t).powi(3) * core::f64::consts::TAU;
    let middle = DVec3::new(0.0, -0.05, 0.0);
    let turn = DAffine3::from_translation(middle)
        * DAffine3::from_quat(DQuat::from_rotation_z(flip))
        * DAffine3::from_scale(DVec3::splat(scale.max(0.0)))
        * DAffine3::from_translation(-middle);
    (turn, (1.0 - t) as f32)
}

/// The animal this client's body rides, by its number, and whose it is.
fn ridden(host: &Host<'_>) -> Option<Near> {
    let carrier = host.in_ways().first()?.carrier;
    host.animals()
        .iter()
        .copied()
        .find(|near| near.number == carrier && near.ridden)
}

/// The body's own pet, pointed out wherever it is while the body is not on
/// it: called by the whistle, or let go once the person confirms. Said
/// `on its way` while it comes from afar, nowhere until it arrives.
fn pet_marker(pet: &Pet, on_its_way: bool) -> Marker {
    let whistle = client::voices::WHISTLE.name;
    Marker {
        id: pet.number,
        on: Marked::Animal(pet.number),
        kind: PET_KIND,
        thing: word::HORSE,
        label: pet.name.clone(),
        glyph: PET_GLYPH,
        note: on_its_way.then_some(word::ON_ITS_WAY),
        choices: vec![
            MarkerChoice {
                verb: word::CALL,
                command: serde_json::json!({ "type": "sound", "voice": whistle }),
                confirm: None,
            },
            MarkerChoice {
                verb: word::LET_GO,
                command: serde_json::json!({ "type": format!("{NAME}.{RELEASE}") }),
                confirm: Some(word::LET_GO_CONFIRM),
            },
        ],
    }
}

/// What holding the key does on the animal the body rides, standing:
/// tames a wild one, the ring longer the more restless its herd, releases one's
/// own pet; nothing aloft, nor on another's.
fn held(host: &Host<'_>) -> Option<Hold> {
    let near = ridden(host)?;
    match near.keeping {
        Keeping::Wild => host.grounded().then_some(Hold {
            verb: word::TAME,
            ring_s: (TAME_S * (0.5 + near.restless)) as f32,
        }),
        Keeping::Mine => host.grounded().then_some(Hold {
            verb: word::RELEASE,
            ring_s: RELEASE_S,
        }),
        Keeping::Anothers => None,
    }
}

/// The key held on the animal the body rides: tames it, or releases it
/// when it is the body's pet.
fn hold(host: &mut Host<'_>) {
    let Some(near) = ridden(host) else {
        return host.reject("this body rides no animal".into());
    };
    let asked = match near.keeping {
        Keeping::Wild => host.tame(near.number),
        Keeping::Mine => host.release(),
        Keeping::Anothers => false,
    };
    if !asked {
        host.reject("this animal is not to be tamed or released from here".into());
    }
}

/// Onto the board from foot or a horse, or off it; never off a horse
/// aloft.
fn board(host: &mut Host<'_>) {
    match host.way() == Some(BOARD) {
        true => leave(host),
        false => mount(host),
    }
}

/// Why a body aloft does not leave what it rides.
const ALOFT: &str = "a horse aloft lands before its rider leaves it: F, or its offer";

/// Off what the body rides, on foot; aloft, refused.
fn leave(host: &mut Host<'_>) {
    if host.aloft() {
        host.reject(ALOFT.into());
    } else {
        host.set_way(None);
    }
}

/// Onto the board, falling onto it as it grows in; a body already on it
/// stays as it is, and one on a horse aloft lands it first.
fn mount(host: &mut Host<'_>) {
    let on = host.way() == Some(BOARD);
    if host.aloft() {
        host.reject(ALOFT.into());
    } else if !host.set_way(Some(BOARD)) {
        host.reject("no board is ridden off the planet or under its ground".into());
    } else if !on {
        host.play(Play {
            clip: Gait::Fall,
            for_s: Some(FALL_S),
            until_moved: false,
        });
    }
}

const DECK: Vec3 = Vec3::new(0.55, 0.1, 0.02);
const STRIPE: Vec3 = Vec3::new(0.85, 0.8, 0.65);
const PAD: Vec3 = Vec3::new(0.15, 0.75, 1.0);
const FLAME: Vec3 = Vec3::new(1.0, 0.35, 0.04);

/// The plank under feet drawn `at`, in their frame: `x` right, `y` up, `z`
/// back, the nose ahead at `-z`. A deck with its nose and tail kicked up, a
/// stripe along it, two pads that glow under it and, at speed, a flame
/// behind that grows with it; as far `grown` as it has.
fn plank(at: DAffine3, speed_mps: f64, flicker: f64, grown: f64) -> Vec<BoxPart> {
    let (turn, glow) = appear(grown);
    let under = at * turn;
    let deck = |at: DVec3, turn: f64, offset: DVec3, size: DVec3| {
        under
            * DAffine3::from_translation(at)
            * DAffine3::from_quat(DQuat::from_rotation_x(turn))
            * DAffine3::from_translation(offset)
            * DAffine3::from_scale(size)
    };
    let solid = |transform, color| BoxPart {
        transform,
        color,
        shine: glow,
        shadow_only: false,
        sheet: false,
    };
    let glowing = |transform, color, shine: f32| BoxPart {
        transform,
        color,
        shine: shine.max(glow),
        shadow_only: false,
        sheet: false,
    };
    let (top, length) = (-0.035, 1.0);
    let ends = length / 2.0;
    let kick = DVec3::new(0.0, 0.0, 0.09);
    let mut parts = vec![
        solid(
            deck(
                DVec3::new(0.0, top, 0.0),
                0.0,
                DVec3::ZERO,
                DVec3::new(0.34, 0.07, length),
            ),
            DECK,
        ),
        solid(
            deck(
                DVec3::new(0.0, top, -ends),
                0.35,
                -kick,
                DVec3::new(0.34, 0.07, 0.18),
            ),
            DECK,
        ),
        solid(
            deck(
                DVec3::new(0.0, top, ends),
                -0.35,
                kick,
                DVec3::new(0.34, 0.07, 0.18),
            ),
            DECK,
        ),
        solid(
            deck(
                DVec3::new(0.0, 0.001, 0.0),
                0.0,
                DVec3::ZERO,
                DVec3::new(0.08, 0.002, 0.9),
            ),
            STRIPE,
        ),
    ];
    for z in [-0.28, 0.28] {
        let pad = deck(
            DVec3::new(0.0, -0.09, z),
            0.0,
            DVec3::ZERO,
            DVec3::new(0.24, 0.04, 0.22),
        );
        parts.push(glowing(pad, PAD, 0.6));
    }
    let pace = (speed_mps / HOVER.top_mps).min(1.6);
    if pace > 0.08 {
        let flame_m = (0.12 + 0.55 * pace) * (1.0 + 0.15 * flicker);
        let behind = |reach: f64, width: f64| {
            let at = DVec3::new(0.0, -0.05, ends + 0.06 + flame_m * reach / 2.0);
            deck(
                at,
                0.0,
                DVec3::ZERO,
                DVec3::new(width, width * 0.5, flame_m * reach),
            )
        };
        parts.push(glowing(behind(1.0, 0.16), FLAME, 0.35));
        parts.push(glowing(behind(0.6, 0.08), Vec3::ONE, 0.8));
    }
    parts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_horse_goes_at_its_kinds_paces_named() {
        let paces = paces();
        let carry = HORSE_KIND.carry.unwrap();
        assert_eq!(
            paces.paces.len(),
            carry.paces.len(),
            "a word for every pace"
        );
        let named: Vec<(&str, f64)> = paces
            .paces
            .iter()
            .map(|pace| (pace.name, pace.speed_mps))
            .collect();
        assert_eq!(
            named,
            [("walk", 1.5), ("trot", 3.6), ("gallop", 9.0), ("run", 11.7)]
        );
        assert_eq!(paces.paces[paces.stepped].name, "gallop", "W gallops");
        assert_eq!(paces.paces[paces.sprint].name, "run", "sprint runs");
        assert_eq!((paces.wade_m, paces.room_m), (0.6, 2.6));
        let flight = paces.flight.expect("a horse born with wings flies");
        let winged = HORSE_KIND.wings.unwrap().flight;
        assert_eq!(flight.name, "fly");
        assert_eq!(
            (flight.speed_mps, flight.ceiling_m, flight.takeoff_m),
            (winged.speed_mps, winged.ceiling_m, winged.takeoff_m)
        );
    }

    #[test]
    fn a_board_grows_in_from_nothing_swelling_and_flipping_to_whole() {
        let (none, glow) = appear(0.0);
        assert!(none.matrix3.abs_diff_eq(glam::DMat3::ZERO, 1e-12));
        assert_eq!(glow, 1.0);
        let (whole, glow) = appear(1.0);
        assert!(whole.abs_diff_eq(DAffine3::IDENTITY, 1e-12));
        assert_eq!(glow, 0.0);
        // Somewhere on the way it is bigger than whole, and turned over.
        let swells = (1..20).any(|i| appear(f64::from(i) / 20.0).0.matrix3.x_axis.length() > 1.05);
        assert!(swells);
        let (half, _) = appear(0.3);
        assert!(half.matrix3.y_axis.normalize().y < 0.0, "{}", half.matrix3);
    }
}
