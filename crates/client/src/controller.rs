//! The avatar controller: walk with radial gravity, swim, fly like superman.
//!
//! State lives in address space (`SurfacePoint` + height), the one place where
//! blocks are unit cubes and where collision with the build layer will run.
//! World space enters only as velocities: a wish direction in metres is
//! converted to an address space delta through the local tangents, so speed
//! feels the same everywhere although blocks are not perfectly square.

use glam::{DMat3, DQuat, DVec3};
use protocol::Body;
use scene::Camera;
use topology::{BLOCK_M, Grid, QuadSphere, SurfacePoint};
use worldgen::Generator;

use crate::animals::Near;
use crate::boom::{self, Boom};
use crate::cells::{Cells, Seat};
use crate::collision::{self, BODY_M, Footing, STEP_M};
use crate::hover::{self, Glide, Hover, Step};
use crate::paces::{Ahead, Flight, Jump, Paces, Pacing, Reins};
use crate::parts::Parts;
use crate::spring::{Follow, Spring};
use crate::trunks::Trunks;
use crate::way::{Profile, Steering};

/// What stands on the ground, as a body stands on it and is stopped by it:
/// the cells of the volumes, the solid parts and the trunks of the plants,
/// each a system of the core.
#[derive(Clone, Copy)]
pub struct Built<'a> {
    pub cells: &'a Cells,
    pub parts: &'a Parts,
    pub trunks: &'a Trunks,
}

impl<'a> Built<'a> {
    /// What a body with its feet `feet_m` over the datum, standing in a
    /// column of a seat, has under and over it of what is built. `None`
    /// with nothing built near.
    pub(crate) fn footing(
        &self,
        generator: &Generator,
        seat: Seat,
        point: SurfacePoint,
        feet_m: f64,
    ) -> Option<Footing> {
        self.footings(generator, seat, point, feet_m)
            .reduce(Footing::with)
    }

    /// [`Built::footing`] one column or part at a time, before the highest
    /// floor and the lowest roof are taken.
    pub(crate) fn footings(
        &self,
        generator: &'a Generator,
        seat: Seat,
        point: SurfacePoint,
        feet_m: f64,
    ) -> impl Iterator<Item = Footing> + 'a {
        let cells = self.cells.footings(seat, point, feet_m);
        let parts = self.parts.footings(generator, seat, point, feet_m);
        let trunks = self
            .trunks
            .footings(generator, self.cells, seat, point, feet_m);
        cells.chain(parts).chain(trunks)
    }

    /// Whether anything stands on a body.
    fn stands_on(&self, body: Body) -> bool {
        self.cells.stands_on(body) || self.parts.stands_on(body) || self.trunks.stands_on(body)
    }
}
use crate::seam::Mode;

const WALK_MPS: f64 = 4.8;
/// Running is this much faster than walking; flying fast, than cruising.
const RUN_FACTOR: f64 = 2.5;
const FLY_SPRINT_FACTOR: f64 = 4.0;
const JUMP_MPS: f64 = 6.0;
pub(crate) const GRAVITY_MPS2: f64 = 14.0;
/// Flight speed grows with altitude so orbit is a short trip.
const FLY_MIN_MPS: f64 = 12.0;
const FLY_PER_ALTITUDE: f64 = 0.8;
const SWIM_MPS: f64 = 2.4;
const SWIM_SPRINT_FACTOR: f64 = 2.0;
/// Feet height while floating: the waterline sits at the waist, and a body
/// lying prone in a swim clip breaks the surface.
const FLOAT_M: f64 = -0.8;
/// Below this the swimmer is diving and steers with the camera pitch.
const DIVING_M: f64 = FLOAT_M - 0.35;
/// Looking down past this while swimming forward starts a dive, radians.
const DIVE_PITCH: f64 = -0.5;
/// Idle swimmers drift back up.
const BUOYANCY_MPS: f64 = 0.7;
/// The moon pulls a fifth as hard: the same legs jump five times as high.
const MOON_GRAVITY_MPS2: f64 = GRAVITY_MPS2 / 5.0;
/// The moon's sphere of influence, in moon radii from its centre. Inside it
/// the avatar is stored relative to the moon and rides its orbit. It is wide
/// on purpose: the hand-over happens where nothing can be seen of it. Leaving
/// takes a little more distance than entering, so the edge never flickers.
const MOON_SOI_ENTER: f64 = 4.0;
const MOON_SOI_EXIT: f64 = 4.4;
/// Between these distances (moon radii from its centre) down turns from the
/// planet to the moon, continuously.
const MOON_PULL_FAR: f64 = 3.0;
const MOON_PULL_NEAR: f64 = 1.3;
/// How fast the frame turns to gravity on foot, per second.
const WALK_ALIGN_PER_S: f64 = 4.0;
/// In flight the frame is the flyer's own. It only settles to the horizon
/// close to a surface: fully under `FLY_ALIGN_NEAR` body radii of clearance,
/// not at all over `FLY_ALIGN_FAR`.
const FLY_ALIGN_PER_S: f64 = 1.5;
const FLY_ALIGN_NEAR: f64 = 0.02;
const FLY_ALIGN_FAR: f64 = 0.30;
const EYE_M: f64 = 1.5;
/// How far under the eye a third person camera aims on a view taller than
/// wide, metres: at the body's middle, so a phone held upright shows the
/// body in the middle of the screen and not under it.
const TALL_AIM_UNDER_M: f64 = 0.6;
/// How far to its side a body steps down off what it rode, metres, and the
/// most the ground there may climb or drop from where the animal stood.
const STEP_DOWN_M: f64 = 1.0;
const STEP_DOWN_RISE_M: f64 = 0.6;
/// How far under a roof the eye is held, metres: a view from inside the
/// roof would see through it.
const EYE_UNDER_M: f64 = 0.15;
/// Taking off on wings, the climb comes in over this long, seconds, as
/// what it carried off the ground goes, and closes on the take-off height
/// this fast, per second, at most this many metres a second.
const RISE_EASE_S: f64 = 0.5;
const RISE_PER_S: f64 = 2.0;
const RISE_MOST_MPS: f64 = 5.0;
/// How long a take-off or a landing refused is said, seconds.
const REFUSED_S: f64 = 2.0;
/// How far over the ground as drawn a flyer is held, metres: on what it
/// rides, eased in as the climb is.
const FLYER_OVER_M: f64 = 0.5;
/// Within this of its way's ceiling, metres, a body aloft on what it rides
/// is at it: what the readout says.
const AT_CEILING_M: f64 = 1.0;
/// Coming down to land, a body this near the floor under it is down,
/// metres.
const TOUCH_M: f64 = 0.05;
/// On wings, a body is held this high over water deeper than its way
/// wades, metres: it comes down to it and no further.
const SEA_OVER_M: f64 = 1.5;
/// Metres between the looks a jump takes along its arc for a roof.
const ARC_LOOK_M: f64 = BLOCK_M / 2.0;

/// The most points on a body's spine, ahead and behind together.
const SPINE_POINTS: usize = 8;

/// How far apart the points of a body's spine are, at most, metres: under
/// the width a point stands on plus a column, so no column between two is
/// missed.
const SPINE_APART_M: f64 = 1.0;
/// A rise under this in one frame is ground, and what is drawn follows it at
/// once: a slope climbed at a walk rises a few centimetres a frame, and a body
/// eased over it sinks into the hill on the way up and floats on the way down.
/// A step is more, up to [`STEP_M`], and is eased.
const GROUND_M: f64 = STEP_M / 2.0;

/// The longest step a body in paces takes, seconds: at a gallop, a few
/// decimetres of ground.
const PACED_DT: f64 = 1.0 / 30.0;
/// How stiffly the figure and the eye come after a body that took a step, per
/// second. Each spring is critically damped: it starts and ends with ease and
/// never passes the body. The figure is there in a tenth of a second, so its
/// feet leave a stair little; the eye in a third, so the view stays calm.
const FIGURE_SETTLE_PER_S: f64 = 48.0;
const EYE_SETTLE_PER_S: f64 = 16.0;
/// How stiffly the eye comes after the saddle's height, per second: slow
/// enough that a stride's rise and rock never reach the view, and the eye
/// still rises into the saddle in a second as the body mounts.
const RIDING_PER_S: f64 = 3.0;
const LOOK_RAD_PER_PX: f64 = 0.0025;
const PITCH_LIMIT: f64 = 1.45;
const TURN_PER_S: f64 = 12.0;
/// In a steered way the camera follows the heading. Turned by the pointer,
/// it stays where it was turned until the pointer has been left alone this
/// long, then eases back behind the heading at this pace.
const CHASE_AFTER_S: f64 = 0.8;
const CHASE_PER_S: f64 = 2.5;

/// What a body flying on what it rides asks of a step: the way it wants
/// to go, world space, and that way along the ground, each a unit or none;
/// whether sprint is held, and how far over the ground it is, metres.
#[derive(Clone, Copy, Debug)]
struct Reach {
    wish: DVec3,
    flat: DVec3,
    sprint: bool,
    clearance_m: f64,
}

/// What is under a body aloft on what it rides: see [`Controller::below`].
#[derive(Clone, Copy, Debug)]
struct Below {
    over_m: f64,
    wet: bool,
}

/// What the controller wants this frame, already free of key codes.
#[derive(Clone, Copy, Debug, Default)]
pub struct Wish {
    /// `[right, forward]`, each `-1..=1`.
    pub movement: [f32; 2],
    pub up: bool,
    pub down: bool,
    pub sprint: bool,
    /// Whether forward and back went down since the last frame, however
    /// briefly: what steps a paced way's pace.
    pub struck: [bool; 2],
    /// How far forward a stick held is pushed, of its reach, negative
    /// pulled back; none with no stick: what paces a way in paces by its
    /// bands, the keys left to the rest ([`Input::reach`]).
    ///
    /// [`Input::reach`]: crate::Input::reach
    pub reach: Option<f32>,
    pub look: [f32; 2],
    pub zoom: f32,
}

/// The celestial body whose frame of reference holds the avatar: what its
/// position is stored relative to. An avatar in the moon's sphere of influence
/// rides the moon's orbit without anyone moving it.
///
/// Which way is down is a separate matter, and continuous: see
/// [`Controller::gravity_up`].
#[derive(Clone, Copy, PartialEq, Debug)]
enum Site {
    /// Position is `point` + `height_m` over the datum sphere.
    Planet,
    /// Position is `direction` (unit, from the moon's centre) + `height_m`
    /// over the moon's datum sphere.
    Moon { direction: DVec3 },
}

/// Where the moon is this frame: centre in world space, and radius.
#[derive(Clone, Copy, Debug)]
pub struct MoonBody {
    pub center: DVec3,
    pub radius_m: f64,
}

/// How the body moves: on foot, in flight, or in a way of moving a plugin
/// gave.
#[derive(Clone, Debug)]
enum Moves {
    Walk,
    Fly,
    /// In a way of moving, by its full name: who turns it, how the body
    /// moves in it, and the animal it carries, its number; zero for none.
    Way {
        name: String,
        steering: Steering,
        kept: Kept,
        carrier: u64,
    },
}

/// How a body moves in a way: its profile, and what it keeps between
/// steps there.
#[derive(Clone, Debug)]
enum Kept {
    /// On a cushion, and how it glides now.
    Glide { hover: Hover, glide: Box<Glide> },
    /// On feet, and at what pace now; or aloft on an animal's wings, and
    /// whether a take-off or a landing was refused a moment ago, and why.
    Paced {
        paces: Box<Paces>,
        pacing: Box<Pacing>,
        aloft: Option<Aloft>,
        refused: Option<(Refused, f64)>,
    },
}

/// Where a body in paces is in its flight, on an animal with wings.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Aloft {
    /// Springing off its legs, `s` seconds from the press, by its way's
    /// [`crate::paces::Spring`]: carried along the ground at `mps`, to top `top_m` over
    /// where it leaves it. On its feet until the top.
    Springing { s: f64, mps: f64, top_m: f64 },
    /// On its wings from the top of its spring, for `s` seconds so far: it
    /// climbs to the way's take-off height, eased in, as what it `carried`
    /// off the ground, world space, eases out.
    Rising { s: f64, carried: DVec3 },
    /// Flying where the body looks, as a body on its own flies.
    Flying,
    /// Coming down to land, slower the nearer the ground.
    Landing,
}

/// Why a body in paces did not take off or land.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Refused {
    /// Its way does not fly here: the animal it rides has no wings, or it
    /// is off the planet.
    Flightless,
    /// A roof over it leaves no room to climb.
    NoRoom,
    /// Under it is water deeper than it wades.
    NoGround,
}

impl Kept {
    /// At rest in a profile, pointing along `heading` with `up` over it.
    fn at_rest(profile: &Profile, heading: DVec3, up: DVec3) -> Kept {
        match profile {
            Profile::Hover(hover) => Kept::Glide {
                hover: *hover,
                glide: Box::new(Glide::new(heading, up)),
            },
            Profile::Paces(paces) => Kept::Paced {
                paces: paces.clone(),
                pacing: Box::new(Pacing::new(heading)),
                aloft: None,
                refused: None,
            },
        }
    }
}

#[derive(Clone, Debug)]
pub struct Controller {
    moves: Moves,
    /// The body this controller walks: its size, and the metres that follow.
    sphere: QuadSphere,
    point: SurfacePoint,
    /// Feet, metres above the datum sphere.
    height_m: f64,
    vertical_mps: f64,
    grounded: bool,
    /// Where the avatar faces: a unit tangent in world space.
    facing: DVec3,
    /// Where the camera looks, flattened on the tangent plane.
    view: DVec3,
    /// Camera pitch, radians, positive looks up.
    pitch: f64,
    /// Whether the view is taller than wide.
    tall: bool,
    /// Seconds since the pointer last turned the view.
    looked_s: f64,
    /// How far behind the eye the camera hangs: zero in first person.
    boom: Boom,
    /// Speed over the last step, metres per second.
    speed_mps: f64,
    /// The part of it along the ground.
    ground_mps: f64,
    /// Distance walked, for the walk cycle.
    stride_m: f64,
    sprinting: bool,
    /// In water too deep to stand in. Only ever true on foot.
    swimming: bool,
    /// Whether the body flies through cells: see [`Controller::pass_cells`].
    passes: bool,
    site: Site,
    moon: Option<MoonBody>,
    /// The footprint the ground around the avatar is drawn with right now:
    /// see [`Controller::shown_ground_m`].
    drawn_footprint_m: f64,
    /// Up of the avatar's own frame: what the camera, the controls and the
    /// body stand on. It turns toward gravity by rotation, never by a cut:
    /// quickly on foot, and in flight only near a surface, because in flight
    /// the flyer's frame wins. Look at the planet from the moon and fly: you go
    /// to the planet, and nothing turns you on the way.
    frame_up: DVec3,
    /// How far the figure and the eye stand off the body's own height, along
    /// the frame's up. A body takes a step whole, in the frame it happens, and
    /// what is drawn is left where it was to come after, each at its own pace:
    /// see [`Controller::stand_on`].
    figure: Spring,
    eye: Spring,
    /// How high over its feet the body rides, metres: a rider sits in a
    /// saddle, and the eye rises with it, eased.
    riding: Follow,
    /// How far the eye rolls with what the body rides, radians, eased:
    /// banking into a turn on the wing.
    banked: Follow,
    /// Metres of room over the feet under the roof the body stood under
    /// last step; none under open sky. Where the eye is held.
    roof_m: Option<f64>,
}

impl Controller {
    /// Stands on the ground at `point`, looking along the sector's `u` axis.
    pub fn spawn(point: SurfacePoint, generator: &Generator) -> Controller {
        let sphere = generator.sphere();
        let tangents = sphere.blocks().tangents(point);
        let view = DVec3::from(tangents.du).normalize();
        Controller {
            moves: Moves::Walk,
            sphere,
            point,
            height_m: generator.sample(sphere.blocks().direction(point)).height_m,
            vertical_mps: 0.0,
            grounded: true,
            facing: view,
            view,
            pitch: -0.25,
            tall: false,
            looked_s: 0.0,
            boom: Boom::at(6.0),
            speed_mps: 0.0,
            ground_mps: 0.0,
            stride_m: 0.0,
            sprinting: false,
            swimming: false,
            passes: false,
            site: Site::Planet,
            moon: None,
            drawn_footprint_m: 0.0,
            frame_up: DVec3::from(tangents.up),
            figure: Spring::default(),
            eye: Spring::default(),
            riding: Follow::default(),
            banked: Follow::default(),
            roof_m: None,
        }
    }

    /// The body this controller is on.
    pub fn sphere(&self) -> QuadSphere {
        self.sphere
    }

    /// How the body moves, as the seam says it.
    pub fn mode(&self) -> Mode {
        match &self.moves {
            Moves::Walk => Mode::Walk,
            Moves::Fly => Mode::Fly,
            Moves::Way { name, .. } => Mode::Way(name.clone()),
        }
    }

    /// Whether the body moves as `mode` says: [`Controller::mode`] asked,
    /// without making one.
    pub fn moves_as(&self, mode: &Mode) -> bool {
        match (&self.moves, mode) {
            (Moves::Walk, Mode::Walk) | (Moves::Fly, Mode::Fly) => true,
            (Moves::Way { name, .. }, Mode::Way(way)) => name == way,
            _ => false,
        }
    }

    fn walking(&self) -> bool {
        matches!(self.moves, Moves::Walk)
    }

    /// On feet: walking, or in a way that goes on them, not yet on wings.
    /// The ground holds such a body as it holds a walker.
    fn on_feet(&self) -> bool {
        matches!(
            self.moves,
            Moves::Walk
                | Moves::Way {
                    kept: Kept::Paced {
                        aloft: None | Some(Aloft::Springing { .. }),
                        ..
                    },
                    ..
                }
        )
    }

    /// How the body flies on what it rides, and where it is in that
    /// flight: none on its feet, and in any way that does not fly.
    fn mount_aloft(&self) -> Option<(Flight, Aloft)> {
        match &self.moves {
            Moves::Way {
                kept:
                    Kept::Paced {
                        paces,
                        aloft: Some(aloft),
                        ..
                    },
                ..
            } => Some((paces.flight?, *aloft)),
            _ => None,
        }
    }

    /// Aloft on what it rides: flying on an animal's wings, or springing
    /// off its legs to, from the press that took it off.
    pub fn aloft(&self) -> bool {
        self.mount_aloft().is_some()
    }

    /// On an animal's wings: aloft and past its spring.
    fn on_wings(&self) -> bool {
        self.mount_aloft()
            .is_some_and(|(_, aloft)| !matches!(aloft, Aloft::Springing { .. }))
    }

    /// Whether a body aloft on what it rides flies as high over the sea as
    /// its way lets it.
    pub fn at_ceiling(&self) -> bool {
        self.mount_aloft().is_some_and(|(flight, aloft)| {
            aloft == Aloft::Flying && self.height_m >= flight.ceiling_m - AT_CEILING_M
        })
    }

    /// Why the body in paces did not take off or land, a moment ago.
    pub fn refused(&self) -> Option<Refused> {
        match &self.moves {
            Moves::Way {
                kept: Kept::Paced { refused, .. },
                ..
            } => refused.map(|(why, _)| why),
            _ => None,
        }
    }

    /// Off the ground on an animal's wings, as [`Controller::land`] brings
    /// it down: in a way in paces that flies, and only where a roof leaves
    /// room over it for the way's take-off. On the ground it springs off
    /// its legs from this frame on, as fast as it went; in a jump its wings
    /// take it at once. Refused, the readout says why for a moment, and the
    /// body stays as it is.
    pub fn take_off(
        &mut self,
        winged: bool,
        generator: &Generator,
        built: Built<'_>,
    ) -> Result<(), Refused> {
        let room_m = self
            .footing_at(
                Seat::Sector(self.point.sector),
                self.point,
                generator,
                self.built(built),
            )
            .room_m();
        let planet = self.site == Site::Planet;
        let underground = self.underground(self.ground_m(generator));
        let grounded = self.grounded;
        let Moves::Way {
            kept:
                Kept::Paced {
                    paces,
                    pacing,
                    aloft,
                    refused,
                },
            ..
        } = &mut self.moves
        else {
            return Err(Refused::Flightless);
        };
        if aloft.is_some() {
            return Ok(());
        }
        let why = match paces.flight {
            None => Refused::Flightless,
            Some(_) if !winged || !planet => Refused::Flightless,
            Some(flight) if underground || room_m < paces.room_m + flight.takeoff_m => {
                Refused::NoRoom
            }
            Some(flight) => {
                let mps = pacing.speed_mps.max(0.0);
                *aloft = Some(match grounded {
                    true => Aloft::Springing {
                        s: 0.0,
                        mps,
                        top_m: flight.spring.height_m,
                    },
                    false => Aloft::Rising {
                        s: 0.0,
                        carried: pacing.heading * mps,
                    },
                });
                *refused = None;
                pacing.leaping = None;
                self.grounded = grounded;
                self.vertical_mps = 0.0;
                return Ok(());
            }
        };
        *refused = Some((why, 0.0));
        Err(why)
    }

    /// Down from flight onto the ground under the body, as it was before
    /// it took off: refused over water deeper than the way wades, and it
    /// stays aloft, the readout saying why for a moment.
    pub fn land(&mut self, generator: &Generator, built: Built<'_>) -> Result<(), Refused> {
        let under = self.ahead(DVec3::ZERO, generator, built);
        let Moves::Way {
            kept:
                Kept::Paced {
                    paces,
                    aloft,
                    refused,
                    ..
                },
            ..
        } = &mut self.moves
        else {
            return Ok(());
        };
        match aloft {
            None => Ok(()),
            Some(_) if under.water_m > paces.wade_m => {
                *refused = Some((Refused::NoGround, 0.0));
                Err(Refused::NoGround)
            }
            Some(aloft) => {
                *aloft = Aloft::Landing;
                *refused = None;
                Ok(())
            }
        }
    }

    /// The animal the body's way carries, by its number: zero for none, on
    /// foot and in flight.
    pub fn carrier(&self) -> u64 {
        match &self.moves {
            Moves::Way { carrier, .. } => *carrier,
            _ => 0,
        }
    }

    /// Whether the body in paces steps backward.
    pub fn backs(&self) -> bool {
        matches!(
            &self.moves,
            Moves::Way {
                kept: Kept::Paced { pacing, .. },
                ..
            } if pacing.backs()
        )
    }

    /// The pace a body in paces goes at now, as its way numbers them: none
    /// at a halt or in any other way of moving.
    pub fn pace(&self) -> Option<usize> {
        match &self.moves {
            Moves::Way {
                kept: Kept::Paced { paces, pacing, .. },
                ..
            } => pacing.going(paces, self.sprinting),
            _ => None,
        }
    }

    /// Seconds since the way under the body balked at what it was asked
    /// into; none in a way that does not, or since it set off again.
    pub fn balked_s(&self) -> Option<f64> {
        match &self.moves {
            Moves::Way {
                kept: Kept::Paced { pacing, .. },
                ..
            } => pacing.balked_s,
            _ => None,
        }
    }

    /// How far the way under the body has gathered itself to jump, 0 to 1;
    /// none unless it does.
    pub fn gathering(&self) -> Option<f64> {
        match &self.moves {
            Moves::Way {
                kept: Kept::Paced { paces, pacing, .. },
                ..
            } => pacing.gathering(paces),
            _ => None,
        }
    }

    /// How tall the body is, metres: what a roof has to clear. In paces,
    /// the height its way asks room for, rider and all; otherwise a body
    /// standing.
    fn body_m(&self) -> f64 {
        match &self.moves {
            Moves::Way {
                kept: Kept::Paced { paces, .. },
                ..
            } => paces.room_m,
            _ => BODY_M,
        }
    }

    /// In flight: on its own, or on the wings of what it rides.
    pub fn flying(&self) -> bool {
        matches!(self.moves, Moves::Fly) || self.on_wings()
    }

    /// The way of moving the body is in, by its full name.
    pub fn way(&self) -> Option<&str> {
        match &self.moves {
            Moves::Way { name, .. } => Some(name),
            _ => None,
        }
    }

    /// On foot, wherever the body is: in a way it hops off as the way says,
    /// in flight it falls.
    pub fn walk(&mut self) {
        if let Moves::Way {
            kept: Kept::Glide { hover, .. },
            ..
        } = &self.moves
        {
            self.vertical_mps = hover.hop_off_mps(GRAVITY_MPS2);
            self.grounded = false;
        }
        self.moves = Moves::Walk;
    }

    /// How high over its feet the body rides now, metres: the saddle it
    /// sits in, zero on its own feet. The eye follows it slowly over `dt`,
    /// so the saddle's stride moves the rider and never the view.
    pub fn ride_at(&mut self, riding_m: f64, dt: f64) {
        self.riding.follow(riding_m, RIDING_PER_S, dt);
    }

    /// How far the animal under the body banks now, radians, right side
    /// up positive: aloft, the eye rolls with it as far as the way's
    /// flight says, eased over `dt`; anywhere else it comes level.
    pub fn bank_at(&mut self, bank: f64, dt: f64) {
        let share = self
            .mount_aloft()
            .map_or(0.0, |(flight, _)| flight.bank_eye);
        self.banked.follow(bank * share, RIDING_PER_S, dt);
    }

    /// Stands the body at rest on what is under a point of the planet near
    /// it, facing a way: where it mounts what it rides, and where it steps
    /// down. False off the planet.
    pub fn set_down(
        &mut self,
        point: SurfacePoint,
        facing: DVec3,
        generator: &Generator,
        built: Built<'_>,
    ) -> bool {
        if self.site != Site::Planet {
            return false;
        }
        let point = self.sphere.blocks().wrapped(point);
        let footing = self.footing_at(Seat::Sector(point.sector), point, generator, Some(built));
        self.point = point;
        self.height_m = footing.floor_m.unwrap_or_else(|| self.ground_m(generator));
        self.vertical_mps = 0.0;
        self.frame_up = self.gravity_up();
        self.facing =
            (facing - self.frame_up * facing.dot(self.frame_up)).normalize_or(self.facing);
        self.figure = Spring::default();
        self.eye = Spring::default();
        true
    }

    /// On foot, steps the body to `at`, world space, standing on what is
    /// under it there, and turns it and its view to `facing`: where a body
    /// takes its place at something, a cabinet's station. The step is
    /// placed at once, where a walk from where the body stands reaches. False,
    /// and the body as it was, when it rides, flies or goes in a way, when
    /// something stops the walk there, or off the planet.
    pub fn step_to(
        &mut self,
        at: DVec3,
        facing: DVec3,
        generator: &Generator,
        built: Built<'_>,
    ) -> bool {
        if !matches!(self.moves, Moves::Walk) {
            return false;
        }
        let Some(point) = self.reaches(at - self.position(), generator, built) else {
            return false;
        };
        if !self.set_down(point, facing, generator, built) {
            return false;
        }
        self.view = self.facing;
        true
    }

    /// The point of the planet a body on foot walks to, `offset` from where
    /// it stands, world space, when nothing stops it on the way there: none
    /// when a wall, a rise of more than a step or a roof too low does, or
    /// off the planet.
    fn reaches(
        &self,
        offset: DVec3,
        generator: &Generator,
        built: Built<'_>,
    ) -> Option<SurfacePoint> {
        if self.site != Site::Planet {
            return None;
        }
        let (du, dv) = self.address_step(offset);
        let grid = self.sphere.blocks();
        let held = self.built(built);
        let asked = grid.wrapped(SurfacePoint::new(
            self.point.sector,
            self.point.u + du,
            self.point.v + dv,
        ));
        let walked = self.walk_to(grid, self.point, du, dv, &Spine::NONE, |point| {
            self.footing_at(Seat::Sector(point.sector), point, generator, held)
        });
        (walked == asked).then_some(walked)
    }

    /// On an animal drawn near, in a way that carries, by its full name: the
    /// body walks to it on foot, or comes down to it in flight, sits on it
    /// facing its way, and rides it, every pace as many times as fast as
    /// the animal's `speed_share` says. False, and the body as it was, when
    /// the way does not carry, when the body rides already, when something is
    /// in the way of a walker to the animal, when a roof over it is too low
    /// for the way, or off the planet.
    pub fn mount(
        &mut self,
        name: &str,
        profile: &Profile,
        steering: Steering,
        animal: &Near,
        generator: &Generator,
        built: Built<'_>,
    ) -> bool {
        let Profile::Paces(paces) = profile else {
            return false;
        };
        let flying = matches!(self.moves, Moves::Fly);
        if animal.number == 0 || self.carrier() != 0 || !(self.on_feet() || flying) {
            return false;
        }
        let point = match flying {
            // Over the ground it needs no path: it comes down into the saddle.
            true => self.sphere.blocks().surface_point(animal.at.to_array()),
            false => {
                let offset = animal.at - self.position();
                let Some(point) = self.reaches(offset, generator, built) else {
                    return false;
                };
                point
            }
        };
        let footing = self.footing_at(
            Seat::Sector(point.sector),
            point,
            generator,
            self.built(built),
        );
        if footing.room_m() < paces.room_m {
            return false;
        }
        // It goes as fast as the animal under it does.
        let profile = Profile::Paces(Box::new(paces.scaled(animal.speed_share, animal.size)));
        self.set_down(point, animal.heading, generator, built)
            && self.take_way(name, &profile, steering, animal.number, generator)
    }

    /// On foot, wherever the body is, as [`Controller::walk`]; off an
    /// animal, it steps down beside it: to its left where it walks there
    /// and the ground takes a step and is dry, else to its right, else
    /// where it sat.
    pub fn step_off(&mut self, generator: &Generator, built: Built<'_>) {
        // Aloft, it falls from where it flew, as a flyer on foot does.
        let carried = self.carrier() != 0 && !self.aloft();
        self.walk();
        if !carried {
            return;
        }
        let (facing, left) = (self.facing, self.frame_up.cross(self.facing));
        for side in [left, -left] {
            let offset = side * STEP_DOWN_M;
            let Some(point) = self.reaches(offset, generator, built) else {
                continue;
            };
            let ahead = self.ahead(offset, generator, built);
            if ahead.rise_m.abs() <= STEP_DOWN_RISE_M && ahead.water_m <= 0.0 {
                self.set_down(point, facing, generator, built);
                return;
            }
        }
    }

    /// In flight, from wherever the body is.
    pub fn fly(&mut self) {
        self.moves = Moves::Fly;
        self.grounded = false;
    }

    /// In a way of moving, by its full name, how a body moves in it, who
    /// turns it and the animal it rides, zero for none: it starts at rest,
    /// pointing where the body faces, and hops onto a cushion as the way
    /// says. A way that carries is taken with an animal and any other
    /// without (see [`Controller::mount`]). Only on the planet and over its
    /// ground: a cushion holds a body over the heightfield, and paces are
    /// looked ahead in it, so on the moon or in a cave the body stays as it
    /// is and the answer is false.
    pub fn take_way(
        &mut self,
        name: &str,
        profile: &Profile,
        steering: Steering,
        carrier: u64,
        generator: &Generator,
    ) -> bool {
        if profile.carried() != (carrier != 0)
            || self.site != Site::Planet
            || self.underground(self.ground_m(generator))
        {
            return false;
        }
        let mut kept = Kept::at_rest(profile, self.facing, self.frame_up);
        if let Kept::Glide { hover, glide } = &mut kept {
            glide.hop(hover.hop_on_mps(GRAVITY_MPS2));
        }
        self.moves = Moves::Way {
            name: name.to_owned(),
            steering,
            kept,
            carrier,
        };
        self.swimming = false;
        true
    }

    /// The unit direction from the body's centre through the avatar.
    fn point_direction(&self) -> [f64; 3] {
        self.sphere.blocks().direction(self.point)
    }

    pub fn point(&self) -> SurfacePoint {
        self.point
    }

    /// Up of the avatar's frame. See the `frame_up` field.
    pub fn up(&self) -> DVec3 {
        self.frame_up
    }

    /// From the centre of the site's body through the avatar.
    pub fn radial(&self) -> DVec3 {
        match self.site {
            Site::Planet => DVec3::from(self.point_direction()),
            Site::Moon { direction } => direction,
        }
    }

    /// How much of "down" belongs to the moon here, `0..=1`.
    fn moon_pull(&self) -> f64 {
        match (self.site, self.moon) {
            (Site::Moon { .. }, Some(moon)) => {
                let radii = (moon.radius_m + self.height_m) / moon.radius_m;
                1.0 - ((radii - MOON_PULL_NEAR) / (MOON_PULL_FAR - MOON_PULL_NEAR)).clamp(0.0, 1.0)
            }
            _ => 0.0,
        }
    }

    /// Against gravity, here. Between the bodies it turns continuously from
    /// one to the other: the first gravity fields.
    pub fn gravity_up(&self) -> DVec3 {
        let pull = self.moon_pull();
        let planet_up = self.position().normalize();
        if pull <= 0.0 {
            return planet_up;
        }
        let eased = pull * pull * (3.0 - 2.0 * pull);
        rotate_toward(planet_up, self.radial(), eased, self.view)
    }

    /// The avatar's frame, columns right, up, back. In a way, tilted with
    /// the surface under it and leaned into the turn.
    pub fn body_basis(&self) -> DMat3 {
        match &self.moves {
            Moves::Way {
                kept: Kept::Glide { glide, .. },
                ..
            } => crate::figure::basis(glide.up(), self.facing),
            _ => crate::figure::basis(self.frame_up, self.facing),
        }
    }

    pub fn on_moon(&self) -> bool {
        self.site != Site::Planet
    }

    /// Feet, metres above the datum sphere of whichever body holds them.
    pub fn height_m(&self) -> f64 {
        self.height_m
    }

    /// Camera pitch, radians, positive looks up.
    pub fn pitch(&self) -> f64 {
        self.pitch
    }

    /// Turns the avatar and the camera to a bearing, degrees clockwise from
    /// north. The same convention `topology::bearing_deg` reads, so a bearing
    /// written down and handed back points the same way.
    pub fn face(&mut self, bearing_deg: f64) {
        const NORTH: DVec3 = DVec3::Y;
        let up = self.up();
        let north = NORTH - up * up.dot(NORTH);
        if north.length() < 1e-6 {
            // At a pole there is no bearing to honour, so nothing turns.
            return;
        }
        let north = north.normalize();
        let east = north.cross(up);
        let radians = bearing_deg.to_radians();
        let heading = north * radians.cos() + east * radians.sin();
        self.view = heading.normalize();
        self.facing = self.view;
        // A way steers by a heading of its own, which turns with the body.
        if let Moves::Way { kept, .. } = &mut self.moves {
            match kept {
                Kept::Glide { glide, .. } => glide.heading = self.facing,
                Kept::Paced { pacing, .. } => pacing.heading = self.facing,
            }
        }
    }

    /// Stands the avatar at a height of its own rather than on the ground.
    /// `None` puts it back on the ground, which is what standing means. A
    /// body in a way stays in it: on the ground, off any wings; at a
    /// height, aloft if it flew there, else coming down onto what is
    /// under it. On its own, at a height it flies.
    pub fn stand_at(&mut self, height_m: Option<f64>, generator: &Generator) {
        let ground = self.ground_m(generator);
        // Aloft on what it rides, it arrives no higher than its way flies.
        let ceiling_m = self
            .mount_aloft()
            .map_or(f64::MAX, |(flight, _)| flight.ceiling_m);
        self.height_m = height_m.map_or(ground, |wanted| wanted.min(ceiling_m).max(ground));
        self.grounded = self.height_m <= ground + 1e-6;
        self.vertical_mps = 0.0;
        match (&mut self.moves, height_m) {
            (Moves::Way { kept, .. }, _) => {
                if let Kept::Paced { aloft, .. } = kept
                    && self.grounded
                {
                    *aloft = None;
                }
            }
            (moves, None) => *moves = Moves::Walk,
            (moves, Some(_)) if !self.grounded => *moves = Moves::Fly,
            (_, Some(_)) => {}
        }
    }

    /// Lifts the avatar onto something built under it, when it stands lower:
    /// on foot it stands there as it does on the ground, and in flight it
    /// keeps flying.
    pub fn lift_onto(&mut self, floor_m: f64) {
        if self.height_m >= floor_m {
            return;
        }
        self.height_m = floor_m;
        self.vertical_mps = 0.0;
        self.grounded = self.on_feet();
    }

    /// Puts the avatar on the moon, at a direction from its centre. The moon
    /// has to be where the clock says before this, because a body without one
    /// has nothing to stand on.
    pub fn stand_on_moon(&mut self, direction: DVec3, generator: &Generator) {
        self.site = Site::Moon {
            direction: direction.normalize_or(DVec3::Y),
        };
        self.height_m = self.ground_m(generator);
        self.grounded = true;
        self.moves = Moves::Walk;
        self.vertical_mps = 0.0;
        let tangents = self.sphere.blocks().tangents(self.point);
        self.view = DVec3::from(tangents.du).normalize();
        self.facing = self.view;
    }

    /// The body that holds the avatar, and where its feet are from the
    /// centre of it.
    pub fn held_by(&self) -> (Body, DVec3) {
        let body = match self.site {
            Site::Planet => Body::Planet,
            Site::Moon { .. } => Body::Moon,
        };
        (body, self.radial() * (self.body().1 + self.height_m))
    }

    /// Centre and datum radius of the body that holds the avatar.
    fn body(&self) -> (DVec3, f64) {
        match (self.site, self.moon) {
            (Site::Moon { .. }, Some(moon)) => (moon.center, moon.radius_m),
            _ => (DVec3::ZERO, self.sphere.radius_m()),
        }
    }

    /// Ground height over the datum of the current body, metres.
    fn ground_m(&self, generator: &Generator) -> f64 {
        match self.site {
            Site::Planet => generator.sample(self.point_direction()).height_m,
            Site::Moon { direction } => {
                generator.moon_sample_at(direction.to_array(), 0.0).height_m
            }
        }
    }

    /// Lets the body fly through cells, or has them hold it in flight as
    /// they do on foot.
    pub fn pass_cells(&mut self, on: bool) {
        self.passes = on;
    }

    /// What is built that holds this body: on foot always, on a mount
    /// aloft too, as it holds the mount on its feet, and in flight unless
    /// it was let pass through them.
    fn built<'a>(&self, built: Built<'a>) -> Option<Built<'a>> {
        (!self.flying() || !self.passes || self.aloft()).then_some(built)
    }

    /// The seat the body is over, and the column of it: a sector of the
    /// body that holds it, on that body's own grid. What is built is seated
    /// there.
    pub fn over(&self, generator: &Generator) -> (Seat, SurfacePoint) {
        match self.site {
            Site::Planet => (Seat::Sector(self.point.sector), self.point),
            Site::Moon { direction } => {
                let grid = generator.moon().blocks();
                let point = grid.surface_point(direction.to_array());
                (Seat::Moon(point.sector), point)
            }
        }
    }

    /// What the body stands on, and what is over its head. Out in the open
    /// the floor is the surface and there is no roof; inside a volume both
    /// are its cells.
    fn footing(&self, generator: &Generator, built: Option<Built<'_>>) -> Footing {
        let (seat, point) = self.over(generator);
        self.footing_at(seat, point, generator, built)
    }

    /// What a body with its feet where they are would stand on at a column
    /// of a seat: the ground, and what is built under or over it.
    fn footing_at(
        &self,
        seat: Seat,
        point: SurfacePoint,
        generator: &Generator,
        built: Option<Built<'_>>,
    ) -> Footing {
        let direction = seat.sphere(generator).blocks().direction(point);
        let ground = Footing::solid(seat::ground_m(generator, seat.body(), direction));
        match built.and_then(|built| built.footing(generator, seat, point, self.height_m)) {
            Some(built) => ground.with(built),
            None => ground,
        }
    }

    /// Whether the body is under the ground as the heightfield draws it: in a
    /// cave, or in the rock around one. Rules that keep a body over the
    /// terrain point the wrong way there.
    fn underground(&self, ground_m: f64) -> bool {
        self.site == Site::Planet && self.height_m < ground_m
    }

    /// Tells the controller how coarse the mesh around the avatar is this frame.
    pub fn set_drawn_footprint(&mut self, footprint_m: f64) {
        self.drawn_footprint_m = footprint_m;
    }

    /// The ground as it is on screen along `direction` from the body's centre:
    /// the real ground, or the coarser mesh over it while finer patches are
    /// still streaming in, whichever is higher. Physics on foot uses the real
    /// ground; the camera and a flyer stay above this one, so nobody ends up
    /// looking at the terrain from underneath.
    fn shown_ground_m(&self, generator: &Generator, direction: DVec3) -> f64 {
        let at = |footprint_m: f64| match self.site {
            Site::Planet => {
                generator
                    .sample_at(direction.to_array(), footprint_m)
                    .height_m
            }
            Site::Moon { .. } => {
                generator
                    .moon_sample_at(direction.to_array(), footprint_m)
                    .height_m
            }
        };
        at(0.0).max(at(self.drawn_footprint_m))
    }

    fn gravity_mps2(&self) -> f64 {
        let pull = self.moon_pull();
        GRAVITY_MPS2 + (MOON_GRAVITY_MPS2 - GRAVITY_MPS2) * pull
    }

    /// Feet in world space: where the body is, for collision and the wire.
    pub fn position(&self) -> DVec3 {
        let (center, radius_m) = self.body();
        center + self.radial() * (radius_m + self.height_m)
    }

    /// Feet as drawn, world space: the body with what a step left the figure
    /// to close. The figure, its label and what it brushes stand here: see
    /// [`Controller::stand_on`].
    pub fn drawn_position(&self) -> DVec3 {
        let bob_m = match &self.moves {
            Moves::Way {
                kept: Kept::Glide { hover, glide },
                ..
            } => hover::bob_m(hover, glide.time_s),
            _ => 0.0,
        };
        self.position() + self.frame_up * (self.figure.off + bob_m)
    }

    /// Where the camera looks, flattened on the tangent plane.
    ///
    /// This, and not [`Controller::facing`], is what a compass reads: standing
    /// still and turning the mouse moves the view and leaves the body where it
    /// was, and "which way am I looking" is a question about the eyes.
    pub fn view(&self) -> DVec3 {
        self.view
    }

    pub fn facing(&self) -> DVec3 {
        self.facing
    }

    pub fn speed_mps(&self) -> f64 {
        self.speed_mps
    }

    pub fn ground_mps(&self) -> f64 {
        self.ground_mps
    }

    /// Metres per second along up: positive while rising.
    pub fn vertical_mps(&self) -> f64 {
        self.vertical_mps
    }

    /// Tells the controller where the moon is this frame.
    pub fn set_moon(&mut self, moon: MoonBody) {
        self.moon = Some(moon);
    }

    /// Metres of free space around the avatar: to the ground or sea below, or
    /// to the moon, whichever is nearer. Flight speed scales with it, so a
    /// trip to the moon is quick and the arrival is gentle.
    fn clearance_m(&self, generator: &Generator) -> f64 {
        let to_moon = match (self.site, self.moon) {
            (Site::Planet, Some(moon)) => (self.position() - moon.center).length() - moon.radius_m,
            _ => f64::MAX,
        };
        self.altitude_m(generator).min(to_moon).max(0.0)
    }

    /// Hands the avatar to the frame of reference it is in now. World position
    /// does not change, only what it is measured from.
    fn resolve_site(&mut self) {
        let Some(moon) = self.moon else {
            return;
        };
        let position = self.position();
        let offset = position - moon.center;
        let radii = offset.length() / moon.radius_m;
        match self.site {
            Site::Planet if radii < MOON_SOI_ENTER => {
                self.site = Site::Moon {
                    direction: offset.normalize_or(DVec3::Y),
                };
                self.height_m = offset.length() - moon.radius_m;
            }
            Site::Moon { .. } if radii > MOON_SOI_EXIT => {
                self.site = Site::Planet;
                self.point = self.sphere.blocks().surface_point(position.to_array());
                self.height_m = position.length() - self.sphere.radius_m();
            }
            _ => {}
        }
    }

    /// Turns the avatar's frame toward gravity: see the `frame_up` field.
    fn align_frame(&mut self, dt: f64, generator: &Generator) {
        let rate = match self.moves {
            Moves::Walk | Moves::Way { .. } => WALK_ALIGN_PER_S,
            Moves::Fly => {
                let body_radius = self.body().1;
                let clearance = self.altitude_m(generator).max(0.0);
                let far =
                    (clearance / body_radius - FLY_ALIGN_NEAR) / (FLY_ALIGN_FAR - FLY_ALIGN_NEAR);
                FLY_ALIGN_PER_S * (1.0 - far.clamp(0.0, 1.0))
            }
        };
        // The whole frame turns as one: what the avatar looks at and faces
        // turns with its up, so aligning never reads as a camera move.
        let fraction = 1.0 - (-rate * dt).exp();
        let turn = turn_toward(self.frame_up, self.gravity_up(), fraction, self.view);
        self.frame_up = (turn * self.frame_up).normalize();
        self.view = turn * self.view;
        self.facing = turn * self.facing;
    }

    pub fn swimming(&self) -> bool {
        self.swimming
    }

    /// Swimming under the surface.
    pub fn diving(&self) -> bool {
        self.swimming && self.height_m < DIVING_M
    }

    /// Sprint is held while moving.
    pub fn sprinting(&self) -> bool {
        self.sprinting
    }

    pub fn stride_m(&self) -> f64 {
        self.stride_m
    }

    pub fn grounded(&self) -> bool {
        self.grounded
    }

    /// Flies to a place in world space.
    pub fn fly_to(&mut self, position: DVec3) {
        self.site = Site::Planet;
        self.point = self.sphere.blocks().surface_point(position.to_array());
        self.height_m = position.length() - self.sphere.radius_m();
        self.moves = Moves::Fly;
        self.grounded = false;
        self.resolve_site();
        self.frame_up = self.gravity_up();
        self.transport();
    }

    /// Moves to a point of the surface, standing on the ground there, as
    /// it was: facing the same way by the compass, and in the way it moved
    /// in, on what it rode, at the pace it went and aloft if it flew.
    /// [`Controller::stand_at`] then says how high it arrives. Every far
    /// move passes here, so a mount follows its rider through each.
    pub fn teleport(&mut self, point: SurfacePoint, generator: &Generator) {
        let bearing = topology::bearing_deg(self.up().to_array(), self.facing.to_array());
        let moves = self.moves.clone();
        *self = Controller {
            pitch: self.pitch,
            boom: self.boom,
            ..Controller::spawn(point, generator)
        };
        if let Some(bearing) = bearing {
            self.face(bearing);
        }
        self.moves = match moves {
            Moves::Way {
                name,
                steering,
                kept,
                carrier,
            } => Moves::Way {
                name,
                steering,
                carrier,
                kept: match kept {
                    Kept::Glide { hover, .. } => Kept::Glide {
                        hover,
                        glide: Box::new(Glide::new(self.facing, self.frame_up)),
                    },
                    Kept::Paced {
                        paces,
                        pacing,
                        aloft,
                        ..
                    } => Kept::Paced {
                        paces,
                        pacing: Box::new(pacing.arrived(self.facing)),
                        aloft: aloft.map(|_| Aloft::Flying),
                        refused: None,
                    },
                },
            },
            moves => moves,
        };
    }

    /// Aims the camera up or down, radians, positive looks up. The boom is
    /// left where it was: how far back someone watches from is theirs.
    pub fn set_pitch(&mut self, pitch: f64) {
        self.pitch = pitch.clamp(-PITCH_LIMIT, PITCH_LIMIT);
    }

    /// Whether the view is taller than wide: a third person camera aims
    /// lower there, [`TALL_AIM_UNDER_M`].
    pub fn set_tall(&mut self, tall: bool) {
        self.tall = tall;
    }

    /// Puts the camera `boom_m` behind the eye at once, or at the eye at zero.
    pub fn set_boom(&mut self, boom_m: f64) {
        self.boom.set(boom_m);
    }

    /// Whether the camera is at the eye, or swinging there.
    pub fn first_person(&self) -> bool {
        self.boom.eye()
    }

    /// Swings the camera to the eye, or out of it to the boom it had.
    pub fn toggle_first_person(&mut self) {
        self.boom.toggle();
    }

    /// Whether the camera is far enough out of the body to see it.
    pub fn sees_own_body(&self) -> bool {
        !self.boom.inside()
    }

    /// Aims the preview camera: `pitch` in radians, `boom_m` behind the eye,
    /// zero at it.
    pub fn pose_view(&mut self, pitch: f64, boom_m: f64) {
        self.pitch = pitch.clamp(-PITCH_LIMIT, PITCH_LIMIT);
        self.set_boom(boom_m);
    }

    /// Places the avatar and the camera for a preview shot: `altitude_m` above
    /// the ground (flying when positive), camera `pitch` in radians, `boom_m`
    /// behind the eye, zero at it.
    pub fn pose(&mut self, altitude_m: f64, pitch: f64, boom_m: f64, generator: &Generator) {
        let ground = self.ground_m(generator);
        // Negative altitudes are depths under the sea, never under the ground.
        self.height_m = (ground.max(0.0) + altitude_m).max(ground);
        self.moves = if altitude_m > 0.0 {
            Moves::Fly
        } else {
            Moves::Walk
        };
        self.grounded = altitude_m <= 0.0;
        self.pose_view(pitch, boom_m);
    }

    /// Metres above the ground, or above the sea where there is one. Negative
    /// under water.
    pub fn altitude_m(&self, generator: &Generator) -> f64 {
        let ground = self.ground_m(generator);
        // Only the planet has a sea to be above.
        let surface = if self.site == Site::Planet {
            ground.max(0.0)
        } else {
            ground
        };
        self.height_m - surface
    }

    pub fn update(&mut self, dt: f64, wish: Wish, generator: &Generator, built: Built<'_>) {
        // `up` is the avatar's own frame: look, steer and fly by it. `gravity`
        // is the world's: fall and float by it. On foot the two agree within a
        // moment; in flight they may not, and the frame wins.
        let up = self.frame_up;
        let gravity = self.gravity_up();
        self.look(wish, up);
        self.looked_s = if wish.look == [0.0; 2] {
            self.looked_s + dt
        } else {
            0.0
        };
        self.boom.advance(dt);
        self.forget_refused(dt);
        match self.moves {
            Moves::Way {
                kept: Kept::Glide { .. },
                ..
            } => return self.glide(dt, wish, generator, built),
            Moves::Way { .. } if self.on_feet() => {
                // A long frame is ridden as short ones: a gallop crosses
                // metres of a slope in a slow frame, and in one step that
                // reads as a wall. A pace is stepped once, in the first.
                // Off the top of a spring its wings take it.
                let slices = (dt / PACED_DT).ceil().max(1.0);
                for slice in 0..slices as usize {
                    let wish = match slice {
                        0 => wish,
                        _ => Wish {
                            struck: [false; 2],
                            ..wish
                        },
                    };
                    match self.mount_aloft() {
                        None => self.paced(dt / slices, wish, generator, built),
                        Some((flight, Aloft::Springing { .. })) => {
                            self.spring(dt / slices, flight, generator, built);
                        }
                        Some(_) => break,
                    }
                }
                // What the slices left is eased once, over the whole frame.
                self.settle_drawn(dt);
                return;
            }
            _ => {}
        }

        let right = self.view.cross(up);
        let [x, y] = wish.movement.map(f64::from);
        let flat = (right * x + self.view * y).normalize_or_zero();
        let sprint = match (wish.sprint, self.flying()) {
            (false, _) => 1.0,
            (true, false) => RUN_FACTOR,
            (true, true) => FLY_SPRINT_FACTOR,
        };

        // Deep enough to float, and sunk to floating depth: swim. Wading in
        // from a beach and falling in from a cliff both end up here.
        let ground = self.ground_m(generator);
        self.swimming = self.site == Site::Planet
            && self.walking()
            && ground < FLOAT_M - 0.1
            && self.height_m <= FLOAT_M + 0.05;

        let lift = f64::from(u8::from(wish.up)) - f64::from(u8::from(wish.down));
        // A body in a way glided or paced above: this one walks, swims or
        // flies, on its own or on what it rides.
        let velocity = match self.flying() {
            // A leap out of the water: from the surface only. Gravity brings
            // the swimmer back, and they are swimming again on the way down.
            _ if self.swimming && wish.up && !self.diving() => {
                self.swimming = false;
                self.vertical_mps = JUMP_MPS * 0.8;
                self.height_m = FLOAT_M + 0.06;
                flat * SWIM_MPS + gravity * self.vertical_mps
            }
            _ if self.swimming => {
                self.vertical_mps = 0.0;
                self.grounded = false;
                // Under water the camera steers. At the surface you swim flat,
                // unless you look well down: then forward is a dive.
                let steered = self.diving() || self.pitch < DIVE_PITCH;
                let forward = if steered {
                    self.view * self.pitch.cos() + up * self.pitch.sin()
                } else {
                    self.view
                };
                let wish_dir = (right * x + forward * y + gravity * lift).normalize_or_zero();
                let pace = if wish.sprint { SWIM_SPRINT_FACTOR } else { 1.0 };
                let drift = if wish_dir == DVec3::ZERO {
                    gravity * BUOYANCY_MPS
                } else {
                    DVec3::ZERO
                };
                wish_dir * SWIM_MPS * pace + drift
            }
            false => {
                let fall_mps2 = self.gravity_mps2();
                self.on_foot(dt, flat * WALK_MPS * sprint, wish.up, gravity, fall_mps2)
            }
            true => {
                self.vertical_mps = 0.0;
                self.grounded = false;
                // Forward follows the camera pitch: look up, fly up.
                let forward = self.view * self.pitch.cos() + up * self.pitch.sin();
                let wish_dir = (right * x + forward * y + up * lift).normalize_or_zero();
                let clearance = self.clearance_m(generator);
                match self.mount_aloft() {
                    None => wish_dir * (FLY_MIN_MPS + clearance * FLY_PER_ALTITUDE) * sprint,
                    Some((flight, _)) => {
                        let reach = Reach {
                            wish: wish_dir,
                            flat,
                            sprint: wish.sprint,
                            clearance_m: clearance,
                        };
                        self.flown(dt, flight, reach, generator, built)
                    }
                }
            }
        };

        self.settle_drawn(dt);
        self.step(velocity * dt, generator, built);
        self.resolve_site();
        self.touch_down(generator, built);
        self.speed_mps = velocity.length();
        self.sprinting = wish.sprint && self.speed_mps > 0.1;
        self.ground_mps = (velocity - gravity * velocity.dot(gravity)).length();
        if self.grounded {
            self.stride_m += flat.length() * WALK_MPS * sprint * dt;
        }

        // Turn the body toward where it goes; in flight, where the camera looks.
        let target = match self.flying() {
            true => self.view,
            false => flat,
        };
        if target != DVec3::ZERO {
            let blend = 1.0 - (-TURN_PER_S * dt).exp();
            self.facing = self.facing.lerp(target, blend);
        }
        self.align_frame(dt, generator);
        self.transport();
    }

    /// The velocity of a body on the wings of what it rides, one step of
    /// `dt`, world space, at the way's speeds: as a body on its own flies,
    /// where it looks, held under the way's ceiling and over the sea. Off
    /// the top of its spring, it climbs to the way's take-off height, eased
    /// in as what it carried eases out, and flies on; landing, it goes no
    /// faster than its fastest pace and comes down slower the nearer what
    /// it lands on.
    fn flown(
        &mut self,
        dt: f64,
        flight: Flight,
        reach: Reach,
        generator: &Generator,
        built: Built<'_>,
    ) -> DVec3 {
        let up = self.frame_up;
        let (height_up, height_m) = (self.height_up(), self.height_m);
        let below = self.below(generator, built);
        let Moves::Way {
            kept:
                Kept::Paced {
                    paces,
                    aloft: Some(aloft),
                    ..
                },
            ..
        } = &mut self.moves
        else {
            return DVec3::ZERO;
        };
        let speed_mps = flight.speed_at(reach.clearance_m, reach.sprint);
        match aloft {
            // Stepped on its legs: [`Controller::spring`].
            Aloft::Springing { .. } => DVec3::ZERO,
            Aloft::Rising { s, carried } => {
                *s += dt;
                let ease = rise_ease(*s);
                let carried = *carried - up * carried.dot(up);
                let rise_mps = ((flight.takeoff_m - below.over_m) * RISE_PER_S)
                    .clamp(0.0, RISE_MOST_MPS)
                    * ease;
                if below.over_m >= flight.takeoff_m - 0.1 {
                    *aloft = Aloft::Flying;
                }
                reach.wish * speed_mps * ease + carried * (1.0 - ease) + up * rise_mps
            }
            Aloft::Flying => {
                let velocity = reach.wish * speed_mps;
                // A step rises at most to the ceiling, over the sea as
                // `height_m` is, along the up it is measured by.
                let room_mps = (flight.ceiling_m - height_m).max(0.0) / dt;
                let rise = velocity.dot(height_up);
                let velocity = match rise > room_mps {
                    true => velocity - height_up * (rise - room_mps),
                    false => velocity,
                };
                let climb = velocity.dot(up);
                match below.wet && below.over_m <= 0.0 && climb < 0.0 {
                    true => velocity - up * climb,
                    false => velocity,
                }
            }
            Aloft::Landing => {
                let fastest = paces.paces.last().map_or(0.0, |pace| pace.speed_mps);
                reach.flat * speed_mps.min(fastest) - up * flight.sink_at(below.over_m.max(0.0))
            }
        }
    }

    /// What is under a body aloft on what it rides: how high it is over
    /// where it would come down, the floor, or [`SEA_OVER_M`] over water
    /// deeper than its way wades, and whether that is water.
    fn below(&self, generator: &Generator, built: Built<'_>) -> Below {
        let under = self.ahead(DVec3::ZERO, generator, built);
        let wade_m = match &self.moves {
            Moves::Way {
                kept: Kept::Paced { paces, .. },
                ..
            } => paces.wade_m,
            _ => 0.0,
        };
        let wet = under.water_m > wade_m;
        Below {
            over_m: match wet {
                true => self.height_m - SEA_OVER_M,
                false => -under.rise_m,
            },
            wet,
        }
    }

    /// A body coming down to land that reached the floor under it is on
    /// its feet again, at the pace nearest how fast it went, its eye eased
    /// down after it. Over water deeper than its way wades it flies on, and
    /// the readout says why.
    fn touch_down(&mut self, generator: &Generator, built: Built<'_>) {
        if !matches!(self.mount_aloft(), Some((_, Aloft::Landing))) {
            return;
        }
        let below = self.below(generator, built);
        if below.over_m > TOUCH_M {
            return;
        }
        let (facing, ground_mps) = (self.facing, self.ground_mps);
        let Moves::Way {
            kept:
                Kept::Paced {
                    paces,
                    pacing,
                    aloft,
                    refused,
                },
            ..
        } = &mut self.moves
        else {
            return;
        };
        if below.wet {
            *aloft = Some(Aloft::Flying);
            *refused = Some((Refused::NoGround, 0.0));
            return;
        }
        *aloft = None;
        let fastest = paces.paces.last().map_or(0.0, |pace| pace.speed_mps);
        let mut landed = Pacing::new(facing);
        landed.pace = paces
            .paces
            .iter()
            .rposition(|pace| pace.speed_mps <= ground_mps + 0.5)
            .map(|pace| pace.min(paces.stepped))
            .filter(|_| ground_mps > 0.5);
        landed.speed_mps = ground_mps.min(fastest);
        **pacing = landed;
        self.grounded = true;
        self.vertical_mps = 0.0;
        self.stand_on(self.height_m - below.over_m);
    }

    /// A refused take-off or landing is said for [`REFUSED_S`], then
    /// forgotten.
    fn forget_refused(&mut self, dt: f64) {
        if let Moves::Way {
            kept: Kept::Paced { refused, .. },
            ..
        } = &mut self.moves
        {
            *refused = refused
                .map(|(why, s)| (why, s + dt))
                .filter(|&(_, s)| s < REFUSED_S);
        }
    }

    /// The fall and the jump of a body on feet, one step of `dt`: it jumps
    /// when asked and it stands, falls `fall_mps2` while nothing holds it,
    /// and goes `along` the ground. Returns its velocity, world space.
    fn on_foot(
        &mut self,
        dt: f64,
        along: DVec3,
        jump: bool,
        gravity: DVec3,
        fall_mps2: f64,
    ) -> DVec3 {
        if self.grounded && jump {
            self.vertical_mps = JUMP_MPS;
            self.grounded = false;
        }
        if !self.grounded {
            self.vertical_mps -= fall_mps2 * dt;
        }
        along + gravity * self.vertical_mps
    }

    /// One step in a way that goes on feet: left and right turn it, forward
    /// and back step its pace, sprint is the pace its way says, jump leaps
    /// from a pace that jumps, and the camera follows behind. It goes as a
    /// walker does, held by the ground and stopped by a wall, and balks at
    /// a wall that stops it dead, at what its way refuses ahead of it and
    /// at a jump that would land on it.
    fn paced(&mut self, dt: f64, wish: Wish, generator: &Generator, built: Built<'_>) {
        let Moves::Way {
            kept: Kept::Paced { paces, pacing, .. },
            ..
        } = &self.moves
        else {
            return;
        };
        if dt <= 0.0 {
            return;
        }
        let up = self.frame_up;
        let gravity = self.gravity_up();
        let tangent = |v: DVec3| v - up * v.dot(up);
        let mut pacing = **pacing;
        pacing.heading = tangent(pacing.heading).normalize_or(self.facing);
        let ahead = self.ahead(pacing.way() * pacing.reach_m(paces), generator, built);
        let [x, y] = wish.movement.map(f64::from);
        let heading = pacing.heading;
        // A stick held paces it alone: its reach past the run is the pace
        // sprint means, never a walker's sprint over another band.
        let sprint = wish.sprint && wish.reach.is_none();
        let reins = Reins {
            turn: x,
            throttle: y,
            struck: wish.struck,
            sprint,
            jump: wish.up,
            reach: wish.reach.map(f64::from),
        };
        let mut along = pacing.step(paces, up, dt, reins, ahead);
        // A turn its body has no room for is held at the facing it has.
        if pacing.heading != heading && !self.turns(heading, pacing.heading, generator, built) {
            along = pacing.held(heading);
        }
        // Gathered, it leaves the ground if it would land where it goes and
        // no roof over the arc stops it on the way.
        let launch = pacing.gathered(paces).map(|jump| {
            let landing = self.ahead(pacing.heading * jump.length_m, generator, built);
            let clear = self.clears(jump, pacing.heading, generator, built);
            (jump, self.grounded && clear && paces.takes(landing))
        });
        self.chase(heading, pacing.heading, dt);
        match launch {
            Some((jump, true)) => {
                self.vertical_mps = pacing.fly(jump);
                self.grounded = false;
            }
            Some((_, false)) => pacing.refuse(),
            None => {}
        }

        let fall_mps2 = pacing.fall_mps2().unwrap_or(self.gravity_mps2());
        let velocity = self.on_foot(dt, along, false, gravity, fall_mps2);
        let position = self.position();
        // The body steps as it faces now, its spine along it.
        self.facing = pacing.heading;
        self.step(velocity * dt, generator, built);
        self.resolve_site();
        // A wall takes what it stopped.
        let went = tangent(self.position() - position) / dt;
        if went.length() < along.length() * 0.99 {
            pacing.stopped(went.dot(pacing.way()));
        }
        if self.grounded && pacing.fall_mps2().is_some() && self.vertical_mps <= 0.0 {
            pacing.land();
        }

        self.speed_mps = went.length();
        self.ground_mps = self.speed_mps;
        self.sprinting = pacing.urged(sprint) && self.speed_mps > 0.1;
        if self.grounded {
            self.stride_m += self.speed_mps * dt;
        }
        self.align_frame(dt, generator);
        self.transport();
        if let Moves::Way {
            kept: Kept::Paced { pacing: kept, .. },
            ..
        } = &mut self.moves
        {
            **kept = pacing;
        }
    }

    /// One step of a body in paces springing off the ground to take off,
    /// by `flight`'s spring: it gathers, carried on as fast as it went,
    /// then leaves the ground, carried at least as fast as the spring says,
    /// to top the spring's height over the ground where it tops, higher
    /// where the ground ahead rises. The ground holds it as it holds a
    /// jump. At the top its wings take it.
    fn spring(&mut self, dt: f64, flight: Flight, generator: &Generator, built: Built<'_>) {
        let Moves::Way {
            kept:
                Kept::Paced {
                    ref pacing,
                    aloft: Some(Aloft::Springing { s, mps, top_m }),
                    ..
                },
            ..
        } = self.moves
        else {
            return;
        };
        if dt <= 0.0 {
            return;
        }
        let spring = flight.spring;
        let (heading, is) = (pacing.heading, s + dt);
        let off = is > spring.gather_s;
        let mps = match off {
            true => mps.max(spring.forward_mps),
            false => mps,
        };
        let top_m = match s <= spring.gather_s && off {
            true => {
                let ahead = self.ahead(heading * mps * spring.rise_s, generator, built);
                spring.height_m + ahead.rise_m.max(0.0)
            }
            false => top_m,
        };
        if off {
            self.grounded = false;
        }
        self.vertical_mps = (spring.height_at(is, top_m) - spring.height_at(s, top_m)) / dt;
        let up = self.frame_up;
        let velocity = heading * mps + self.gravity_up() * self.vertical_mps;
        let position = self.position();
        self.facing = heading;
        self.step(velocity * dt, generator, built);
        self.resolve_site();
        let went = self.position() - position;
        self.ground_mps = (went - up * went.dot(up)).length() / dt;
        self.speed_mps = self.ground_mps;
        self.sprinting = false;
        if self.grounded {
            self.stride_m += self.ground_mps * dt;
        }
        self.align_frame(dt, generator);
        self.transport();
        let topped = is >= spring.top_s();
        if topped {
            (self.grounded, self.vertical_mps) = (false, 0.0);
        }
        if let Moves::Way {
            kept: Kept::Paced { aloft, .. },
            ..
        } = &mut self.moves
        {
            *aloft = Some(match topped {
                true => Aloft::Rising {
                    s: 0.0,
                    carried: heading * mps,
                },
                false => Aloft::Springing { s: is, mps, top_m },
            });
        }
    }

    /// Whether a jump along `heading` keeps the body under every roof over
    /// its arc, looked at every [`ARC_LOOK_M`] from where it leaves to
    /// where it lands.
    fn clears(&self, jump: Jump, heading: DVec3, generator: &Generator, built: Built<'_>) -> bool {
        let looks = (jump.length_m / ARC_LOOK_M).ceil().max(1.0) as u32;
        let body_m = self.body_m();
        (0..=looks).all(|look| {
            let along = f64::from(look) / f64::from(looks);
            let ahead = self.ahead(heading * jump.length_m * along, generator, built);
            ahead
                .room_m
                .is_none_or(|room_m| ahead.rise_m + room_m >= jump.height_at(along) + body_m)
        })
    }

    /// What a body on feet would go into `offset` from where it stands,
    /// world space, along the ground: how far the floor there rises over its
    /// feet, how deep the sea is over it and how much room a roof leaves.
    /// Off the planet it finds nothing.
    fn ahead(&self, offset: DVec3, generator: &Generator, built: Built<'_>) -> Ahead {
        let open = Ahead {
            rise_m: 0.0,
            water_m: 0.0,
            room_m: None,
        };
        if self.site != Site::Planet {
            return open;
        }
        let (du, dv) = self.address_step(offset);
        let grid = self.sphere.blocks();
        let point = grid.wrapped(SurfacePoint::new(
            self.point.sector,
            self.point.u + du,
            self.point.v + dv,
        ));
        let footing = self.footing_at(
            Seat::Sector(point.sector),
            point,
            generator,
            self.built(built),
        );
        let ground_m = generator.sample(grid.direction(point)).height_m;
        let floor_m = footing.floor_m.unwrap_or(ground_m);
        Ahead {
            rise_m: floor_m - self.height_m,
            water_m: (-floor_m).max(0.0),
            room_m: footing.ceiling_m.map(|ceiling_m| ceiling_m - floor_m),
        }
    }

    /// One step in a way of moving. Steered, left and right turn the
    /// heading and the camera follows it; otherwise the body goes where it
    /// is pushed, as seen from the camera. Glided as the way says, held over
    /// the surface by its cushion, stopped by a wall as a walker is. The
    /// body faces along its heading.
    fn glide(&mut self, dt: f64, wish: Wish, generator: &Generator, built: Built<'_>) {
        let Moves::Way {
            steering,
            kept: Kept::Glide { hover, glide },
            ..
        } = &self.moves
        else {
            return;
        };
        let (hover, steering, mut glide) = (*hover, *steering, **glide);
        // A step of no time moves nothing, and how fast it went is no
        // number at all.
        if dt <= 0.0 {
            return;
        }
        let up = self.frame_up;
        let tangent = |v: DVec3| v - up * v.dot(up);
        glide.heading = tangent(glide.heading).normalize_or(self.facing);
        glide.velocity = tangent(glide.velocity);

        let [x, y] = wish.movement.map(f64::from);
        let step = Step {
            hover: &hover,
            up,
            gravity_mps2: self.gravity_mps2(),
            dt,
        };
        let heading = glide.heading;
        let velocity = match steering {
            Steering::Steered => {
                let velocity = glide.steered(step, x, y, wish.sprint);
                self.chase(heading, glide.heading, dt);
                velocity
            }
            Steering::Relative => {
                let right = self.view.cross(up);
                let wish_dir = (right * x + self.view * y).clamp_length_max(1.0);
                glide.toward(step, wish_dir, wish.sprint)
            }
        };

        let position = self.position();
        self.settle_drawn(dt);
        self.step_on_planet(velocity * dt, generator, built);
        // A wall takes what it stopped.
        let went = tangent(self.position() - position) / dt;
        if went.length() < velocity.length() * 0.99 {
            glide.velocity = went;
        }

        // The surface around the body and ahead of it, where the body is
        // now, smoothed over the way's footprint; under it, the true ground
        // and what is built there are its floor.
        let position = self.position();
        let surface = |offset: DVec3| {
            let direction = (position + offset).normalize().to_array();
            let ground_m = generator.sample_at(direction, hover.footprint_m).height_m;
            hover::surface_m(&hover, ground_m)
        };
        let speed_mps = went.dot(glide.heading);
        let ground_m = hover::surface_m(&hover, self.ground_m(generator));
        let mut reading = hover::read(&hover, up, glide.heading, speed_mps, surface).over(ground_m);
        let built = self.built(built);
        let footing = self.footing(generator, built);
        if let Some(floor_m) = footing.floor_m.filter(|floor_m| *floor_m > ground_m) {
            reading = reading.on(floor_m);
        }
        glide.tilt(step, &reading);
        // Sprint speeds it alone; jump hops it, and held lifts it.
        glide.jump(&hover, step.gravity_mps2, wish.up, self.grounded);
        let mut height_m = glide.float(step, self.height_m, &reading, went, wish.up);
        if let Some(ceiling_m) = footing.ceiling_m {
            let head_m = (ceiling_m - self.body_m()).max(reading.ground_m);
            if height_m > head_m {
                height_m = head_m;
                glide.vertical_mps = glide.vertical_mps.min(0.0);
            }
        }

        self.height_m = height_m;
        self.roof_m = footing.ceiling_m.map(|ceiling_m| ceiling_m - height_m);
        self.vertical_mps = glide.vertical_mps;
        self.grounded = glide.held(&hover, height_m, &reading);
        self.swimming = false;
        self.speed_mps = went.length();
        self.ground_mps = self.speed_mps;
        self.sprinting = wish.sprint && self.speed_mps > 0.1;
        self.facing = glide.heading;
        self.align_frame(dt, generator);
        self.transport();
        if let Moves::Way {
            kept: Kept::Glide { glide: kept, .. },
            ..
        } = &mut self.moves
        {
            **kept = glide;
        }
    }

    /// The chase camera of a steered way: the view turns as the heading
    /// turned from `was` to `is`, so what the pointer set stays set, and
    /// once the pointer is left alone it eases back behind the heading.
    fn chase(&mut self, was: DVec3, is: DVec3, dt: f64) {
        let up = self.frame_up;
        let yaw = |from: DVec3, to: DVec3| from.cross(to).dot(up).atan2(from.dot(to));
        self.view = DQuat::from_axis_angle(up, yaw(was, is)) * self.view;
        if self.looked_s >= CHASE_AFTER_S {
            let ease = 1.0 - (-CHASE_PER_S * dt).exp();
            self.view = DQuat::from_axis_angle(up, yaw(self.view, is) * ease) * self.view;
        }
        self.view = (self.view - up * self.view.dot(up)).normalize_or(is);
    }

    /// Applies pointer look and zoom.
    fn look(&mut self, wish: Wish, up: DVec3) {
        let yaw = -f64::from(wish.look[0]) * LOOK_RAD_PER_PX;
        self.view = DQuat::from_axis_angle(up, yaw) * self.view;
        self.pitch = (self.pitch - f64::from(wish.look[1]) * LOOK_RAD_PER_PX)
            .clamp(-PITCH_LIMIT, PITCH_LIMIT);
        self.boom.zoom(f64::from(wish.zoom));
    }

    /// Moves by a world space displacement: in address space on the planet,
    /// over a plain sphere on the moon, where the cells of a volume in the
    /// way stop it all the same.
    fn step(&mut self, delta: DVec3, generator: &Generator, built: Built<'_>) {
        match self.site {
            Site::Planet => self.step_on_planet(delta, generator, built),
            Site::Moon { direction } => {
                let radius = self.body().1 + self.height_m;
                let along = delta - direction * direction.dot(delta);
                let asked = (direction * radius + along).normalize();
                let held = self
                    .built(built)
                    .filter(|built| built.stands_on(Body::Moon));
                self.site = Site::Moon {
                    direction: match held {
                        Some(built) => self.past_built(direction, asked, generator, built),
                        None => asked,
                    },
                };
                self.height_m += direction.dot(delta);
            }
        }

        let ground = self.ground_m(generator);
        if self.swimming {
            // Between the sea floor and floating depth: nobody swims into the air.
            self.height_m = self.height_m.min(FLOAT_M).max(ground);
            self.roof_m = None;
            return;
        }
        // A mount aloft over water deeper than it wades is held over the
        // sea, a floor whatever its speed and the frame's length.
        let sea_m = (self.aloft() && self.below(generator, built).wet).then_some(SEA_OVER_M);
        let built = self.built(built);
        let footing = self.footing(generator, built);
        let floor_m = match self.moves {
            // A flyer is held over the ground as it is drawn, so it never
            // ends up looking at the terrain from underneath, and over the
            // cells of a volume it may not pass. Under the ground that rule
            // points the wrong way, and the footing, which knows about the
            // cave, takes over.
            // Coming down to land, it is held by what it lands on.
            _ if self.flying()
                && !self.underground(ground)
                && !matches!(self.mount_aloft(), Some((_, Aloft::Landing))) =>
            {
                let over_m = match self.mount_aloft() {
                    Some((_, Aloft::Rising { s, .. })) => FLYER_OVER_M * rise_ease(s),
                    _ => FLYER_OVER_M,
                };
                let shown_m = self.shown_ground_m(generator, self.radial()) + over_m;
                let (seat, point) = self.over(generator);
                let built_m = built
                    .and_then(|built| built.footing(generator, seat, point, self.height_m))
                    .and_then(|footing| footing.floor_m);
                Some(built_m.map_or(shown_m, |built_m| shown_m.max(built_m)))
            }
            _ => footing.floor_m,
        };
        let floor_m = match (floor_m, sea_m) {
            (Some(floor_m), Some(sea_m)) => Some(floor_m.max(sea_m)),
            (floor_m, sea_m) => floor_m.or(sea_m),
        };
        match floor_m {
            // Nothing within reach under the feet: keep falling.
            None => self.grounded = false,
            Some(floor_m) if self.height_m <= floor_m => {
                self.stand_on(floor_m);
                if self.on_feet() {
                    self.grounded = true;
                    self.vertical_mps = 0.0;
                }
            }
            Some(floor_m) if self.on_feet() && self.grounded => {
                // Walking downhill: stay glued to the ground over a step
                // (auto step), fall off anything deeper.
                if self.height_m - floor_m < STEP_M {
                    self.stand_on(floor_m);
                } else {
                    self.grounded = false;
                }
            }
            Some(_) => {}
        }

        // A roof stops a rise the way the ground stops a fall. Squeezed into
        // a gap thinner than a body, the floor wins: better low than sunk.
        if let Some(ceiling_m) = footing.ceiling_m {
            let head_m = (ceiling_m - self.body_m()).max(floor_m.unwrap_or(f64::MIN));
            if self.height_m > head_m {
                self.height_m = head_m;
                self.vertical_mps = self.vertical_mps.min(0.0);
            }
        }
        self.roof_m = footing.ceiling_m.map(|ceiling_m| ceiling_m - self.height_m);
    }

    /// Puts the feet on the floor under them. On foot, a change over
    /// [`GROUND_M`] and up to a step leaves the figure and the eye where they
    /// were, to come after with ease: a stair jolts the body and never the
    /// figure or the view. Less is ground, and more is the world changing
    /// under the body: both are followed at once. In paces every change up
    /// to a step leaves the eye: a gallop crosses ground fast enough that
    /// its rises, small or not, would jolt it.
    fn stand_on(&mut self, floor_m: f64) {
        let rise_m = floor_m - self.height_m;
        let step = rise_m.abs() <= STEP_M + 1e-6;
        if !self.flying() && step && rise_m.abs() >= GROUND_M {
            self.figure.left_by(rise_m);
        }
        if !self.flying() && step && (rise_m.abs() >= GROUND_M || self.paced_eye().is_some()) {
            self.eye.left_by(rise_m);
        }
        self.height_m = floor_m;
    }

    /// How stiffly the eye comes after the ground in the paces the body
    /// goes in; none out of them.
    fn paced_eye(&self) -> Option<f64> {
        match &self.moves {
            Moves::Way {
                kept: Kept::Paced { paces, .. },
                ..
            } => Some(paces.eye_per_s),
            _ => None,
        }
    }

    /// Closes what a step left between what is drawn and the body.
    fn settle_drawn(&mut self, dt: f64) {
        self.figure.settle(FIGURE_SETTLE_PER_S, dt);
        let eye_per_s = self.paced_eye().unwrap_or(EYE_SETTLE_PER_S);
        self.eye.settle(eye_per_s, dt);
    }

    /// A displacement on the planet, world space, as blocks along each
    /// address axis of the column the body is in, at its height.
    fn address_step(&self, delta: DVec3) -> (f64, f64) {
        let tangents = self.sphere.blocks().tangents(self.point);
        let radius = self.sphere.radius_m() + self.height_m;
        // Metres per block along each address axis, at this height.
        let (tu, tv) = (
            DVec3::from(tangents.du) * radius,
            DVec3::from(tangents.dv) * radius,
        );

        // Solve `du * tu + dv * tv = delta` on the tangent plane. The axes are
        // not orthogonal, so this is a 2x2 system, not two dot products.
        let (a, b, c) = (tu.dot(tu), tu.dot(tv), tv.dot(tv));
        let (p, q) = (tu.dot(delta), tv.dot(delta));
        let det = a * c - b * b;
        ((c * p - b * q) / det, (a * q - b * p) / det)
    }

    /// The up a step measures height along, `height_m` changed by a move's
    /// part along it: the column's on the planet.
    fn height_up(&self) -> DVec3 {
        match self.site {
            Site::Planet => DVec3::from(self.sphere.blocks().tangents(self.point).up),
            Site::Moon { direction } => direction,
        }
    }

    fn step_on_planet(&mut self, delta: DVec3, generator: &Generator, built: Built<'_>) {
        let up = self.height_up();
        let (du, dv) = self.address_step(delta);

        let held = self.built(built);
        let (grid, here) = (self.sphere.blocks(), self.point);
        let seat = |point: SurfacePoint| Seat::Sector(point.sector);
        self.point = if self.stopped_by_rock(generator) {
            let spine = self.spine(self.facing);
            self.walk_to(grid, here, du, dv, &spine, |point| {
                self.footing_at(seat(point), point, generator, held)
            })
        } else if let Some(held) = held.filter(|_| !self.on_feet()) {
            // Open ground holds a flyer, or a body in a way, up and never
            // stops it; what is built is a wall to it all the same.
            self.walk_to(grid, here, du, dv, &Spine::NONE, |point| {
                let built = held.footing(generator, seat(point), point, self.height_m);
                built.unwrap_or(Footing::OPEN)
            })
        } else {
            self.sphere.blocks().wrapped(SurfacePoint::new(
                self.point.sector,
                self.point.u + du,
                self.point.v + dv,
            ))
        };
        self.height_m += up.dot(delta);
    }

    /// Whether rock in the way stops this body. A walker and a body in a
    /// way, always: a rise of more than a step over the feet is a wall to
    /// both. A flyer only inside the ground, where it is in a cave and the
    /// walls are real. Swimmers pass: the sea has no walls, and its floor is
    /// a height.
    fn stopped_by_rock(&self, generator: &Generator) -> bool {
        !self.swimming && (!self.flying() || self.underground(self.ground_m(generator)))
    }

    /// Where a step over the moon ends, from one direction from its centre
    /// toward another: where it asked, or short of what is built in the
    /// way. The moon's ground stops nobody, and what is built there is
    /// walked in the address space of the moon's own grid, as the planet's.
    fn past_built(
        &self,
        from: DVec3,
        asked: DVec3,
        generator: &Generator,
        built: Built<'_>,
    ) -> DVec3 {
        let grid = generator.moon().blocks();
        let here = grid.surface_point(from.to_array());
        let there = grid.surface_point(asked.to_array());
        // Over a seam no volume stands.
        if here.sector != there.sector {
            return asked;
        }
        let (du, dv) = (there.u - here.u, there.v - here.v);
        let point = self.walk_to(grid, here, du, dv, &Spine::NONE, |point| {
            let footing = built.footing(generator, Seat::Moon(point.sector), point, self.height_m);
            footing.unwrap_or(Footing::OPEN)
        });
        match point == grid.wrapped(SurfacePoint::new(here.sector, here.u + du, here.v + dv)) {
            true => asked,
            false => DVec3::from(grid.direction(point)),
        }
    }

    /// Where a step of `du, dv` from a column of a grid ends, in address
    /// space: where it asked, or short of whatever stopped it.
    ///
    /// A rise of one block is taken in stride and more is a wall, so a cave
    /// wall stops a body exactly the way a cliff does and the only way past
    /// either is to jump or to fly. Blocked, the step is tried one address
    /// axis at a time, which is what slides a body along a wall instead of
    /// sticking it to one.
    ///
    /// The step goes a cell at a time along either axis, so no column it
    /// crosses is jumped over and no wall is thinner than a step. A flyer's
    /// goes in one: at its speeds, a cell at a time is a look at every cell
    /// it crosses, and a wall thinner than its step is passed.
    ///
    /// Each point of the body's `spine` goes the same piece and is admitted
    /// as the body is, its feet as high as the body's or on a floor of its
    /// own that is higher: so a long body stops with its nose at a wall,
    /// backs with its rump to one, slides along one whole, and flies one
    /// only when all of it clears.
    fn walk_to(
        &self,
        grid: Grid,
        from: SurfacePoint,
        du: f64,
        dv: f64,
        spine: &Spine,
        footing: impl Fn(SurfacePoint) -> Footing,
    ) -> SurfacePoint {
        let pieces = if self.flying() {
            1
        } else {
            du.abs().max(dv.abs()).ceil().max(1.0) as u32
        };
        // Where a point of the body is with the pieces taken along each
        // axis: a share of the step past its offset, in the frame of the
        // column it started from, so the whole step ends where it asked.
        let at = |[along_u, along_v]: [u32; 2], [off_u, off_v]: [f64; 2]| {
            let share = |taken: u32| f64::from(taken) / f64::from(pieces);
            let (u, v) = (from.u + du * share(along_u), from.v + dv * share(along_v));
            grid.wrapped(SurfacePoint::new(from.sector, u + off_u, v + off_v))
        };
        let (mut taken, mut here) = ([0, 0], footing(from));
        let mut reached = [Footing::OPEN; SPINE_POINTS];
        for (point, offset) in reached.iter_mut().zip(spine.points()) {
            *point = footing(at(taken, offset));
        }
        let body_m = self.body_m();
        for _ in 0..pieces {
            let piece = [[1, 1], [1, 0], [0, 1]].into_iter().find_map(|[u, v]| {
                if du * f64::from(u) == 0.0 && dv * f64::from(v) == 0.0 {
                    return None;
                }
                let tried = [taken[0] + u, taken[1] + v];
                let there = footing(at(tried, [0.0, 0.0]));
                if !collision::admits(here, there, self.height_m, body_m) {
                    return None;
                }
                let mut reaches = reached;
                for (point, offset) in reaches.iter_mut().zip(spine.points()) {
                    let (was, will) = (*point, footing(at(tried, offset)));
                    let feet_m = self.feet_m(here, was);
                    if !collision::admits(was, will, feet_m, body_m) {
                        return None;
                    }
                    *point = will;
                }
                Some((tried, there, reaches))
            });
            match piece {
                Some((tried, there, reaches)) => (taken, here, reached) = (tried, there, reaches),
                None => break,
            }
        }
        if taken == [0, 0] {
            from
        } else {
            at(taken, [0.0, 0.0])
        }
    }

    /// The body's spine, facing `facing`: a body in paces on its feet as
    /// long as its way says; any other, none.
    fn spine(&self, facing: DVec3) -> Spine {
        match &self.moves {
            Moves::Way {
                kept: Kept::Paced {
                    paces, aloft: None, ..
                },
                ..
            } => {
                let (du, dv) = self.address_step(facing);
                Spine::along([du, dv], paces.ahead_m, paces.behind_m)
            }
            _ => Spine::NONE,
        }
    }

    /// Whether the body may turn where it stands from facing `from` to
    /// facing `to`: each point of its spine admitted from where it was to
    /// where it swings, as a step would admit it.
    fn turns(&self, from: DVec3, to: DVec3, generator: &Generator, built: Built<'_>) -> bool {
        if self.site != Site::Planet || !self.stopped_by_rock(generator) {
            return true;
        }
        let (grid, held, body_m) = (self.sphere.blocks(), self.built(built), self.body_m());
        let footing = |[du, dv]: [f64; 2]| {
            let point = SurfacePoint::new(self.point.sector, self.point.u + du, self.point.v + dv);
            let point = grid.wrapped(point);
            self.footing_at(Seat::Sector(point.sector), point, generator, held)
        };
        let seat = footing([0.0, 0.0]);
        let (was, will) = (self.spine(from), self.spine(to));
        was.points().zip(will.points()).all(|(was, will)| {
            let (here, there) = (footing(was), footing(will));
            collision::admits(here, there, self.feet_m(seat, here), body_m)
        })
    }

    /// Where the feet are at a point of the body's spine standing on
    /// `point`, metres, the body on `seat`: as high as the body's, or more
    /// by as far as the point's floor is over the body's. On the ground it
    /// follows a rise and keeps the body's height down a fall; in a jump all
    /// of it is in the air.
    fn feet_m(&self, seat: Footing, point: Footing) -> f64 {
        match (seat.floor_m, point.floor_m) {
            (Some(seat_m), Some(point_m)) => self.height_m + (point_m - seat_m).max(0.0),
            _ => self.height_m,
        }
    }

    /// Keeps the tangent vectors tangent to the frame: projected back on the
    /// plane under `frame_up`. With the frame following gravity as the avatar
    /// moves this is parallel transport, and it is why seams and corners need
    /// no special case.
    fn transport(&mut self) {
        let up = self.frame_up;
        let flatten = |v: DVec3, fallback: DVec3| {
            let flat = (v - up * v.dot(up)).normalize_or_zero();
            if flat == DVec3::ZERO { fallback } else { flat }
        };
        let any = up.any_orthonormal_vector();
        self.view = flatten(self.view, any);
        self.facing = flatten(self.facing, self.view);
    }

    /// How high over the feet the eye is, metres: as tall as the body
    /// stands or rides, held under a roof.
    fn eye_m(&self) -> f64 {
        let eye_m = EYE_M + self.eye.off + self.riding.value();
        self.roof_m
            .map_or(eye_m, |roof_m| eye_m.min(roof_m - EYE_UNDER_M))
    }

    /// The camera at the end of the boom: behind and above the eye in third
    /// person, at it in first, never underground.
    pub fn camera(&self, generator: &Generator) -> Camera {
        let up = self.frame_up;
        let forward = self.view * self.pitch.cos() + up * self.pitch.sin();
        let boom_m = self.boom.length_m();
        let at_eye = 1.0 - (boom_m / boom::NEAREST_M).min(1.0);
        let under_m = if self.tall { TALL_AIM_UNDER_M } else { 0.0 };
        let target = self.position() + up * (self.eye_m() - under_m * (1.0 - at_eye));
        let mut position = target - forward * boom_m;

        // Never inside the body the avatar stands on, as drawn. In a cave
        // that rule would yank the camera out through the roof, so there the
        // boom is cut by the rock behind it instead: the camera stays in the
        // cave with the body, as near to it as the walls allow. A boom of
        // zero has no run to cut, and the camera stays at the eye.
        let (center, datum_m) = self.body();
        if self.underground(self.ground_m(generator)) {
            let run_m = collision::clear_run_m(generator, target, -forward, boom_m);
            position = target - forward * run_m;
        } else {
            let from_center = position - center;
            let ground = self.shown_ground_m(generator, from_center.normalize());
            let min_radius = datum_m + ground + 0.4;
            if from_center.length() < min_radius {
                position = center + from_center.normalize() * min_radius;
            }
        }

        // Aim at the target from wherever the camera ended up. Nearer than
        // the nearest third person boom the aim turns to where the view looks,
        // all there is at the eye, wherever the floor over the ground put it.
        let aim = (target - position).normalize_or(forward);
        let look = aim.lerp(forward, at_eye).normalize();
        let right = look.cross(up).normalize();
        let rotation = DQuat::from_mat3(&DMat3::from_cols(right, right.cross(look), -look))
            * DQuat::from_rotation_z(self.banked.value());
        Camera {
            position,
            rotation,
            fov_y: 60f32.to_radians(),
            near: 0.1,
        }
    }
}

/// How far a climb on wings has come in `s` seconds off the top of a
/// spring, 0 to 1, eased over [`RISE_EASE_S`].
fn rise_ease(s: f64) -> f64 {
    let ease = (s / RISE_EASE_S).clamp(0.0, 1.0);
    ease * ease * (3.0 - 2.0 * ease)
}

/// The rotation that takes `from` a `fraction` of the way to `to`, both unit.
/// When they are opposite any axis would do, so the caller names one.
fn turn_toward(from: DVec3, to: DVec3, fraction: f64, fallback_axis: DVec3) -> DQuat {
    let angle = from.dot(to).clamp(-1.0, 1.0).acos();
    if angle < 1e-9 {
        return DQuat::IDENTITY;
    }
    let axis = from.cross(to).normalize_or(
        fallback_axis
            .cross(from)
            .normalize_or(from.any_orthonormal_vector()),
    );
    DQuat::from_axis_angle(axis, angle * fraction)
}

/// `from` turned a `fraction` of the way to `to`.
fn rotate_toward(from: DVec3, to: DVec3, fraction: f64, fallback_axis: DVec3) -> DVec3 {
    (turn_toward(from, to, fraction, fallback_axis) * from).normalize()
}

/// A body's length along the way it faces: points ahead of where it
/// stands to its nose and behind it to its rump, each kept off what stops
/// the body. A body on foot is where it stands, and has none.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Spine {
    /// Each point's offset from where the body stands, blocks along `u`
    /// and `v`.
    points: [[f64; 2]; SPINE_POINTS],
    len: usize,
}

impl Spine {
    const NONE: Spine = Spine {
        points: [[0.0; 2]; SPINE_POINTS],
        len: 0,
    };

    /// Points `ahead_m` along `per_m`, blocks a metre, and `behind_m`
    /// against it, each end one and the rest even between it and the
    /// middle, at most [`SPINE_APART_M`] apart while there are points
    /// enough.
    fn along(per_m: [f64; 2], ahead_m: f64, behind_m: f64) -> Spine {
        let mut spine = Spine::NONE;
        for (reach_m, sign) in [(ahead_m, 1.0), (behind_m, -1.0)] {
            let count = ((reach_m / SPINE_APART_M).ceil() as usize).min(SPINE_POINTS / 2);
            for point in 1..=count {
                let m = sign * reach_m * point as f64 / count as f64;
                spine.points[spine.len] = per_m.map(|blocks| blocks * m);
                spine.len += 1;
            }
        }
        spine
    }

    fn points(&self) -> impl Iterator<Item = [f64; 2]> + '_ {
        self.points[..self.len].iter().copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hover::BOARD;
    use topology::{BLOCK_M, Sector};
    use voxel::{Gesture, Span};
    use worldgen::Recipe;

    /// A walker over a recorded input: walking, running, jumping and
    /// looking round, up a stair of cells, into a wall and off a ledge, for
    /// a minute of frames. Every frame's body, bit for bit, folded into one
    /// number.
    fn walked() -> u64 {
        let generator = Generator::new(Recipe::new(1)).unwrap();
        let side = f64::from(generator.sphere().blocks().side());
        let point = SurfacePoint::new(Sector::new(4).unwrap(), side * 0.41, side * 0.37);
        let mut build = Cells::default();
        let [x, y, top] = build.floor(&generator, point);
        let stair = (0..4).map(|step| Gesture::Create {
            span: Span::between(
                [x + 6 + step * 2, y - 8, top],
                [x + 7 + step * 2, y + 8, top + step],
            ),
            paint: 0,
        });
        let wall = Gesture::Create {
            span: Span::between([x + 16, y - 8, top], [x + 17, y + 8, top + 7]),
            paint: 0,
        };
        let gestures: Vec<Gesture> = stair.chain([wall]).collect();
        assert!(build.apply(&generator, point.sector, &gestures));
        let parts = Parts::default();
        let built = Built {
            cells: &build,
            parts: &parts,
            trunks: &Trunks::default(),
        };
        let mut controller = Controller::spawn(point, &generator);
        let mut seed = 0x5eed_u64;
        let mut fold = 0xcbf2_9ce4_8422_2325_u64;
        let mut put = |bits: u64| {
            fold ^= bits;
            fold = fold.wrapping_mul(0x0100_0000_01b3);
        };
        let mut wish = Wish::default();
        for frame in 0..3600 {
            // A new hand on the keys every half second.
            if frame % 30 == 0 {
                seed = seed
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1_442_695_040_888_963_407);
                let bit = |n: u32| (seed >> (60 - n)) & 1 == 1;
                wish = Wish {
                    movement: [
                        [0.0, 1.0, -1.0, 0.4][(seed >> 33) as usize % 4],
                        [1.0, 1.0, 0.0, -1.0][(seed >> 41) as usize % 4],
                    ],
                    up: bit(0) && bit(1),
                    sprint: bit(2),
                    look: [((seed >> 20) % 41) as f32 - 20.0, 0.0],
                    ..Wish::default()
                };
            }
            let dt = [1.0 / 60.0, 1.0 / 30.0, 1.0 / 144.0][frame % 3];
            controller.update(dt, wish, &generator, built);
            wish.look = [0.0; 2];
            let position = controller.position();
            for value in [
                position.x,
                position.y,
                position.z,
                controller.height_m(),
                controller.facing().x,
                controller.facing().y,
                controller.facing().z,
                controller.speed_mps(),
                controller.stride_m(),
                controller.drawn_position().x,
            ] {
                put(value.to_bits());
            }
            put(u64::from(controller.grounded()));
        }
        fold
    }

    /// The walker's frames are what they were before ways on feet shared its
    /// fall and its floor.
    #[test]
    fn a_walker_walks_as_it_did() {
        assert_eq!(walked(), WALKED, "{:#x}", walked());
    }

    /// [`walked`] as the walker gave it before ways on feet.
    const WALKED: u64 = 0xab41_998f_a7ee_d7e6;

    /// Seed 1 about 0-7ZZZNHR, where a rider's horse stood dead on the hills:
    /// ground there rises up to 1.1 a metre. Galloped off each of eight ways
    /// for twenty seconds, it goes on along every one, climbs, and its pace
    /// falls on the steep; a slow frame now and then, as a browser gives,
    /// never stops it.
    #[test]
    fn a_horse_rides_the_hills_where_one_stood_dead() {
        let generator = Generator::new(Recipe::new(1)).unwrap();
        let grid = generator.sphere().blocks();
        let pose = crate::place::Pose::parse(grid, "0-7ZZZNHR").unwrap();
        let paces = crate::paces::tests::horse();
        let built = Built {
            cells: &Cells::default(),
            parts: &Parts::default(),
            trunks: &crate::Trunks::default(),
        };
        let mut climbed_m: f64 = 0.0;
        let mut held_down = false;
        for way in 0..8 {
            let mut controller =
                Controller::spawn(SurfacePoint::center_of(pose.column), &generator);
            let up = controller.frame_up;
            controller.facing =
                DQuat::from_axis_angle(up, f64::from(way) * core::f64::consts::TAU / 8.0)
                    * controller.facing;
            assert!(controller.take_way(
                "test.horse",
                &Profile::Paces(Box::new(paces.clone())),
                Steering::Steered,
                1,
                &generator
            ));
            let (from, from_m) = (controller.position(), controller.height_m);
            for frame in 0..20 * 60 {
                let wish = Wish {
                    struck: [frame < 30 && frame % 10 == 0, false],
                    ..Wish::default()
                };
                let dt = match frame % 97 {
                    50 => 0.5,
                    _ => 1.0 / 60.0,
                };
                controller.update(dt, wish, &generator, built);
                assert_eq!(controller.balked_s(), None, "way {way}, frame {frame}");
                if frame > 60 {
                    assert!(
                        controller.speed_mps > 1.0,
                        "way {way}, frame {frame}: stood"
                    );
                    let pace = controller.pace();
                    held_down |= pace < Some(2);
                }
            }
            climbed_m = climbed_m.max(controller.height_m - from_m);
            assert!((controller.position() - from).length() > 40.0, "way {way}");
        }
        assert!(held_down, "the steep never held the gallop down");
        assert!(climbed_m > 15.0, "climbed {climbed_m} m at most");
    }

    /// A body in a steered way from `point`, flat out and turning gently
    /// for `seconds` in frames of `dt`: the least its feet came over the
    /// surface under them, where they ended over it, and how fast.
    fn glide_from(point: SurfacePoint, dt: f64, seconds: f64) -> (f64, f64, f64) {
        let generator = Generator::new(Recipe::new(1)).unwrap();
        let mut controller = Controller::spawn(point, &generator);
        assert!(controller.take_way(
            "test.way",
            &Profile::Hover(BOARD),
            Steering::Steered,
            0,
            &generator
        ));
        assert_eq!(controller.mode(), Mode::Way("test.way".into()));
        let wish = Wish {
            movement: [0.3, 1.0],
            sprint: false,
            ..Wish::default()
        };
        let (cells, parts) = (Cells::default(), Parts::default());
        let built = Built {
            cells: &cells,
            parts: &parts,
            trunks: &Trunks::default(),
        };
        let (mut least_m, mut over_m) = (f64::MAX, 0.0);
        for _ in 0..(seconds / dt).round() as usize {
            controller.update(dt, wish, &generator, built);
            let surface_m = hover::surface_m(&BOARD, controller.ground_m(&generator));
            over_m = controller.height_m() - surface_m;
            least_m = least_m.min(over_m);
        }
        (least_m, over_m, controller.speed_mps())
    }

    /// On a board, sprint only speeds it, and jump held lifts it.
    #[test]
    fn on_a_board_sprint_speeds_and_jump_lifts() {
        let generator = Generator::new(Recipe::new(1)).unwrap();
        let side = f64::from(generator.sphere().blocks().side());
        let point = SurfacePoint::new(Sector::new(4).unwrap(), side * 0.41, side * 0.37);
        let (cells, parts) = (Cells::default(), Parts::default());
        let built = Built {
            cells: &cells,
            parts: &parts,
            trunks: &Trunks::default(),
        };
        let over = |sprint: bool, up: bool| {
            let mut controller = Controller::spawn(point, &generator);
            assert!(controller.take_way(
                "test.way",
                &Profile::Hover(BOARD),
                Steering::Steered,
                0,
                &generator
            ));
            let wish = Wish {
                movement: [0.0, 1.0],
                sprint,
                up,
                ..Wish::default()
            };
            run(&mut controller, 3.0, wish, &generator, built);
            let surface_m = hover::surface_m(&BOARD, controller.ground_m(&generator));
            (controller.height_m() - surface_m, controller.speed_mps())
        };
        let (cruise_m, cruise_mps) = over(false, false);
        let (sprint_m, sprint_mps) = over(true, false);
        let (lift_m, _) = over(false, true);
        assert!(
            sprint_mps > cruise_mps + 2.0,
            "{sprint_mps} m/s, {cruise_mps} without"
        );
        assert!(sprint_m < cruise_m + 1.0, "sprint lifted it {sprint_m} m");
        assert!(lift_m > cruise_m + 10.0, "jump held lifted it {lift_m} m");
    }

    #[test]
    fn a_body_in_a_way_never_sinks_into_the_ground_flat_out_with_long_frames() {
        let generator = Generator::new(Recipe::new(1)).unwrap();
        let side = f64::from(generator.sphere().blocks().side());
        let point = SurfacePoint::new(Sector::new(4).unwrap(), side * 0.41, side * 0.37);
        assert!(
            generator
                .sample(generator.sphere().blocks().direction(point))
                .height_m
                > 0.0
        );
        let (least_m, _, _) = glide_from(point, 0.1, 30.0);
        assert!(least_m >= 0.35 * 0.25 - 1e-6, "{least_m}");
    }

    #[test]
    fn a_body_hops_onto_a_way_and_off_it() {
        let generator = Generator::new(Recipe::new(1)).unwrap();
        let side = f64::from(generator.sphere().blocks().side());
        let point = SurfacePoint::new(Sector::new(4).unwrap(), side * 0.41, side * 0.37);
        let mut controller = Controller::spawn(point, &generator);
        let (cells, parts) = (Cells::default(), Parts::default());
        let built = Built {
            cells: &cells,
            parts: &parts,
            trunks: &Trunks::default(),
        };
        let dt = 1.0 / 60.0;
        assert!(controller.take_way(
            "test.way",
            &Profile::Hover(BOARD),
            Steering::Steered,
            0,
            &generator
        ));
        let ground_m = hover::surface_m(&BOARD, controller.ground_m(&generator));
        let (mut highest_m, mut last_m): (f64, f64) = (0.0, 0.0);
        for _ in 0..120 {
            controller.update(dt, Wish::default(), &generator, built);
            last_m = controller.height_m() - ground_m;
            highest_m = highest_m.max(last_m);
        }
        // From the ground to its hop over the cushion, and back down onto
        // the cushion, held there: the cushion rests over the surface
        // smoothed, a little off the true ground here.
        let over_m = controller.height_m() - ground_m;
        assert!(highest_m > 0.35 + 0.45 - 0.05, "{highest_m}");
        assert!(highest_m > over_m + 0.2, "{highest_m} {over_m}");
        assert!((over_m - 0.35).abs() < 0.25, "{over_m}");
        controller.update(dt, Wish::default(), &generator, built);
        assert!((controller.height_m() - ground_m - last_m).abs() < 0.01);

        // Off it, up first, and down on its feet.
        controller.walk();
        assert_eq!(controller.way(), None);
        controller.update(dt, Wish::default(), &generator, built);
        assert!(
            controller.height_m() - ground_m > over_m,
            "{}",
            controller.height_m()
        );
        assert!(!controller.grounded());
        for _ in 0..120 {
            controller.update(dt, Wish::default(), &generator, built);
        }
        assert!(controller.grounded());
    }

    /// A wall `thick` cells thick and four metres high across a plot, near
    /// its far edge, and a body in a way ridden at it along `u` from each of
    /// a hundred starts a tenth of a cell apart, already at `speed_mps`, in
    /// frames of `dt`. How many end past the wall.
    fn through_a_wall(thick: i32, speed_mps: f64, dt: f64) -> usize {
        let generator = Generator::new(Recipe::new(1)).unwrap();
        let side = f64::from(generator.sphere().blocks().side());
        let point = SurfacePoint::new(Sector::new(4).unwrap(), side * 0.41, side * 0.37);
        let mut build = Cells::default();
        let [x0, y, top] = build.floor(&generator, point);
        let x = x0 + 56;
        let wall = Gesture::Create {
            span: Span::between([x, y, top], [x + thick - 1, y + 63, top + 7]),
            paint: 0,
        };
        assert!(build.apply(&generator, point.sector, &[wall]));
        let parts = Parts::default();
        let built = Built {
            cells: &build,
            parts: &parts,
            trunks: &Trunks::default(),
        };
        let fast = Hover {
            top_mps: 40.0,
            sprint_mps: 70.0,
            accel_mps2: 16.0,
            ..BOARD
        };
        let ahead = Wish {
            movement: [0.0, 1.0],
            ..Wish::default()
        };
        (0..100)
            .filter(|start| {
                let u = f64::from(x0) + 1.5 + f64::from(*start) * 0.1;
                let mut controller =
                    Controller::spawn(SurfacePoint::new(point.sector, u, point.v), &generator);
                controller.stand_at(Some(f64::from(top) * BLOCK_M), &generator);
                assert!(controller.take_way(
                    "test.way",
                    &Profile::Hover(fast),
                    Steering::Steered,
                    0,
                    &generator
                ));
                if let Moves::Way {
                    kept: Kept::Glide { glide, .. },
                    ..
                } = &mut controller.moves
                {
                    glide.velocity = controller.facing * speed_mps;
                }
                for _ in 0..(1.5 / dt).round() as usize {
                    controller.update(dt, ahead, &generator, built);
                }
                controller.point().u >= f64::from(x + thick)
            })
            .count()
    }

    #[test]
    fn a_body_in_a_way_never_passes_a_wall_whatever_its_speed_or_frame() {
        // A wall of a cell at top speed, of two at the boost, of four with
        // frames of a tenth of a second: steps of up to eight cells.
        for (thick, speed_mps, dt) in [
            (1, 40.0, 1.0 / 60.0),
            (1, 40.0, 1.0 / 30.0),
            (2, 70.0, 1.0 / 30.0),
            (4, 40.0, 0.1),
        ] {
            let through = through_a_wall(thick, speed_mps, dt);
            assert_eq!(
                through, 0,
                "{thick} cells at {speed_mps} m/s, {dt} s frames"
            );
        }
    }

    #[test]
    fn a_step_of_no_time_moves_a_body_in_a_way_nowhere() {
        let generator = Generator::new(Recipe::new(1)).unwrap();
        let side = f64::from(generator.sphere().blocks().side());
        let point = SurfacePoint::new(Sector::new(4).unwrap(), side * 0.41, side * 0.37);
        let mut controller = Controller::spawn(point, &generator);
        let (cells, parts) = (Cells::default(), Parts::default());
        let built = Built {
            cells: &cells,
            parts: &parts,
            trunks: &Trunks::default(),
        };
        assert!(controller.take_way(
            "test.way",
            &Profile::Hover(BOARD),
            Steering::Steered,
            0,
            &generator
        ));
        let ahead = Wish {
            movement: [0.0, 1.0],
            ..Wish::default()
        };
        for _ in 0..30 {
            controller.update(1.0 / 60.0, ahead, &generator, built);
        }
        let (at, speed_mps) = (controller.point(), controller.speed_mps());
        controller.update(0.0, ahead, &generator, built);
        assert_eq!(controller.point(), at);
        assert_eq!(controller.speed_mps(), speed_mps);
        assert!(controller.vertical_mps().is_finite());
    }

    #[test]
    fn a_steered_way_turns_the_camera_with_the_heading_and_never_the_heading_with_the_camera() {
        let generator = Generator::new(Recipe::new(1)).unwrap();
        let side = f64::from(generator.sphere().blocks().side());
        let point = SurfacePoint::new(Sector::new(4).unwrap(), side * 0.41, side * 0.37);
        let mut controller = Controller::spawn(point, &generator);
        assert!(controller.take_way(
            "test.way",
            &Profile::Hover(BOARD),
            Steering::Steered,
            0,
            &generator
        ));
        let (cells, parts) = (Cells::default(), Parts::default());
        let built = Built {
            cells: &cells,
            parts: &parts,
            trunks: &Trunks::default(),
        };
        let dt = 1.0 / 60.0;
        let behind = |controller: &Controller| controller.view.dot(controller.facing);
        // Steered left for a second: the camera stays behind the body.
        let left = Wish {
            movement: [-1.0, 1.0],
            ..Wish::default()
        };
        let facing = controller.facing;
        for _ in 0..60 {
            controller.update(dt, left, &generator, built);
        }
        assert!(
            controller.facing.dot(facing) < 0.0,
            "turned past a right angle"
        );
        assert!(behind(&controller) > 0.999, "{}", behind(&controller));
        // The pointer swings the view to the side going straight: the body
        // holds its heading, and the view stays where it was swung while the
        // pointer moves.
        let ahead = Wish {
            movement: [0.0, 1.0],
            ..Wish::default()
        };
        let facing = controller.facing;
        for _ in 0..30 {
            let look = Wish {
                look: [20.0, 0.0],
                ..ahead
            };
            controller.update(dt, look, &generator, built);
        }
        assert!(controller.facing.dot(facing) > 0.999);
        let swung = behind(&controller);
        assert!(swung < 0.5, "{swung}");
        // Left alone, the view eases back behind the heading.
        for _ in 0..30 {
            controller.update(dt, ahead, &generator, built);
        }
        assert!((behind(&controller) - swung).abs() < 1e-6, "held a moment");
        for _ in 0..240 {
            controller.update(dt, ahead, &generator, built);
        }
        assert!(behind(&controller) > 0.99, "{}", behind(&controller));
    }

    #[test]
    fn a_body_in_a_way_rides_the_sea_at_its_height() {
        let generator = Generator::new(Recipe::new(1)).unwrap();
        let grid = generator.sphere().blocks();
        let side = f64::from(grid.side());
        let sea = (1..40)
            .flat_map(|i| (1..40).map(move |j| (i, j)))
            .map(|(i, j)| {
                let (u, v) = (f64::from(i) / 40.0 * side, f64::from(j) / 40.0 * side);
                SurfacePoint::new(Sector::ALL[0], u, v)
            })
            .find(|point| generator.sample(grid.direction(*point)).height_m < -200.0)
            .expect("seed 1 has deep sea in sector 0");
        let (_, over_m, speed_mps) = glide_from(sea, 1.0 / 60.0, 12.0);
        assert!((over_m - 0.35).abs() < 0.02, "{over_m}");
        assert!((speed_mps - 14.0).abs() < 1.0, "{speed_mps}");
    }

    /// A flyer a metre over a plot, in front of a wall four metres high that
    /// runs the width of it, flying at it for two seconds. How far along `u`
    /// it ends short of the wall, in cells: negative past it.
    fn fly_at_a_wall(passes: bool) -> f64 {
        let generator = Generator::new(Recipe::new(1)).unwrap();
        let side = f64::from(generator.sphere().blocks().side());
        let point = SurfacePoint::new(Sector::new(4).unwrap(), side * 0.41, side * 0.37);
        let mut build = Cells::default();
        let [_, y, top] = build.floor(&generator, point);
        let floor_m = f64::from(top) * BLOCK_M;
        // A cell is the address it has: the wall stands on the floor eight
        // columns on from the body, across the whole of the plot.
        let x = point.u.floor() as i32 + 8;
        let wall = Gesture::Create {
            span: Span::between([x, y, top], [x, y + 63, top + 7]),
            paint: 0,
        };
        build.apply(&generator, point.sector, &[wall]);
        let mut controller = Controller::spawn(point, &generator);
        controller.pass_cells(passes);
        controller.stand_at(Some(floor_m + 2.0 * BLOCK_M), &generator);
        controller.set_pitch(0.0);
        let wish = Wish {
            movement: [0.0, 1.0],
            ..Wish::default()
        };
        for _ in 0..120 {
            controller.update(
                1.0 / 60.0,
                wish,
                &generator,
                Built {
                    cells: &build,
                    parts: &Parts::default(),
                    trunks: &Trunks::default(),
                },
            );
        }
        point.u.floor() + 8.0 - controller.point().u
    }

    /// A walker on a slab, walking onto a stair one cell high that runs the
    /// width of the plot. The most the feet rise in one tick, the most the
    /// figure as drawn does and the most the camera does, metres, and how far
    /// the camera ends from where a camera with no ease stands.
    fn walk_up_a_stair() -> (f64, f64, f64, f64) {
        let generator = Generator::new(Recipe::new(1)).unwrap();
        let side = f64::from(generator.sphere().blocks().side());
        let point = SurfacePoint::new(Sector::new(4).unwrap(), side * 0.41, side * 0.37);
        let mut build = Cells::default();
        let [_, y, top] = build.floor(&generator, point);
        let floor_m = f64::from(top) * BLOCK_M;
        let x = point.u.floor() as i32 + 3;
        let stair = Gesture::Create {
            span: Span::between([x, y, top], [x + 8, y + 63, top]),
            paint: 0,
        };
        build.apply(&generator, point.sector, &[stair]);
        let mut controller = Controller::spawn(point, &generator);
        controller.lift_onto(floor_m);
        let wish = Wish {
            movement: [0.0, 1.0],
            ..Wish::default()
        };
        let radius = |c: &Controller| c.camera(&generator).position.length();
        let drawn = |c: &Controller| c.drawn_position().length();
        let rigid_m = radius(&controller) - controller.height_m;
        let (mut feet_m, mut figure_m, mut eye_m) = (0.0f64, 0.0f64, 0.0f64);
        for _ in 0..48 {
            let before = (controller.height_m, drawn(&controller), radius(&controller));
            controller.update(
                1.0 / 60.0,
                wish,
                &generator,
                Built {
                    cells: &build,
                    parts: &Parts::default(),
                    trunks: &Trunks::default(),
                },
            );
            feet_m = feet_m.max(controller.height_m - before.0);
            figure_m = figure_m.max(drawn(&controller) - before.1);
            eye_m = eye_m.max(radius(&controller) - before.2);
        }
        assert!((controller.height_m - floor_m - BLOCK_M).abs() < 1e-6);
        let off_m = radius(&controller) - controller.height_m - rigid_m;
        (feet_m, figure_m, eye_m, off_m)
    }

    #[test]
    fn what_is_drawn_eases_over_a_step_the_body_takes_at_once() {
        let (feet_m, figure_m, eye_m, off_m) = walk_up_a_stair();
        assert!((feet_m - BLOCK_M).abs() < 1e-6, "{feet_m}");
        // Over a step the eye moves three metres a second at most, a twentieth
        // of a metre a tick; the figure, three times as stiff, nine.
        assert!(figure_m < 0.16, "{figure_m}");
        assert!(eye_m < 0.06, "{eye_m}");
        assert!(off_m.abs() < 0.01, "{off_m}");
    }

    /// A horse at each of its paces onto a stair one cell high, at the
    /// speed of the pace, two frames in three short and every third a long
    /// one: the feet take the step whole, and the eye rises with ease.
    #[test]
    fn a_horse_takes_a_step_and_the_eye_rises_with_ease() {
        let generator = Generator::new(Recipe::new(1)).unwrap();
        let side = f64::from(generator.sphere().blocks().side());
        let point = SurfacePoint::new(Sector::new(4).unwrap(), side * 0.41, side * 0.37);
        let mut build = Cells::default();
        let [_, y, top] = build.floor(&generator, point);
        let floor_m = f64::from(top) * BLOCK_M;
        let x = point.u.floor() as i32 + 3;
        let stair = Gesture::Create {
            span: Span::between([x, y, top], [x + 8, y + 63, top]),
            paint: 0,
        };
        build.apply(&generator, point.sector, &[stair]);
        let built = Built {
            cells: &build,
            parts: &Parts::default(),
            trunks: &crate::Trunks::default(),
        };
        let paces = crate::paces::tests::horse();
        for (pace, gone) in paces.paces.iter().enumerate() {
            let mut controller = Controller::spawn(point, &generator);
            controller.lift_onto(floor_m);
            assert!(controller.take_way(
                "test.horse",
                &Profile::Paces(Box::new(paces.clone())),
                Steering::Steered,
                1,
                &generator
            ));
            if let Moves::Way {
                kept: Kept::Paced { pacing, .. },
                ..
            } = &mut controller.moves
            {
                pacing.pace = Some(pace);
                pacing.speed_mps = gone.speed_mps;
            }
            let radius = |c: &Controller| c.camera(&generator).position.length();
            let (mut feet_m, mut eye_m) = (0.0f64, 0.0f64);
            // Until it is halfway along the stair.
            let mut frame = 0;
            while controller.point().u < x as f64 + 4.0 {
                frame += 1;
                let before = (controller.height_m, radius(&controller));
                let dt = if frame % 3 == 2 {
                    1.0 / 20.0
                } else {
                    1.0 / 60.0
                };
                controller.update(dt, Wish::default(), &generator, built);
                feet_m = feet_m.max(controller.height_m - before.0);
                // At most two and a half metres a second, a step or a long
                // frame.
                eye_m = eye_m.max((radius(&controller) - before.1) / (dt * 60.0));
                assert!(frame < 600, "{}: never reached the stair", gone.name);
            }
            assert!(
                (controller.height_m - floor_m - BLOCK_M).abs() < 1e-6,
                "{}: not up",
                gone.name
            );
            assert!((feet_m - BLOCK_M).abs() < 1e-6, "{}: {feet_m}", gone.name);
            assert!(
                eye_m < 0.042,
                "{}: {eye_m} m a sixtieth of a second",
                gone.name
            );
        }
    }

    /// A horse galloping at a wall across its way, `cells` high, six metres
    /// ahead on a floor, jump pressed as it sets off when `jump`, under a
    /// roof `roof` cells over the floor when there is one: how far it got
    /// along, metres from where it set off, the highest its feet rose in
    /// the air over the ground it left, and whether it still gallops. Under
    /// a roof, every frame it rides, its back and the eye stay under it.
    fn gallop_at(cells: i32, jump: bool, roof: Option<i32>) -> (f64, f64, bool) {
        let generator = Generator::new(Recipe::new(1)).unwrap();
        let side = f64::from(generator.sphere().blocks().side());
        let near = SurfacePoint::new(Sector::new(4).unwrap(), side * 0.41, side * 0.37);
        let mut build = Cells::default();
        let [plot, y, top] = build.floor(&generator, near);
        let floor_m = f64::from(top) * BLOCK_M;
        // From the west of the floor's plot, the wall well inside it.
        let point = SurfacePoint::new(near.sector, f64::from(plot) + 4.5, near.v);
        let x = plot + 16;
        if cells > 0 {
            let wall = Gesture::Create {
                span: Span::between([x, y, top], [x, y + 63, top + cells - 1]),
                paint: 0,
            };
            assert!(build.apply(&generator, point.sector, &[wall]));
        }
        if let Some(roof) = roof {
            let over = Gesture::Create {
                span: Span::between([plot, y, top + roof], [plot + 40, y + 63, top + roof]),
                paint: 0,
            };
            assert!(build.apply(&generator, point.sector, &[over]));
        }
        let ceiling_m = roof.map(|roof| floor_m + f64::from(roof) * BLOCK_M);
        let built = Built {
            cells: &build,
            parts: &Parts::default(),
            trunks: &crate::Trunks::default(),
        };
        let paces = crate::paces::tests::horse();
        let mut controller = Controller::spawn(point, &generator);
        controller.lift_onto(floor_m);
        assert!(controller.take_way(
            "test.horse",
            &Profile::Paces(Box::new(paces.clone())),
            Steering::Steered,
            1,
            &generator
        ));
        if let Moves::Way {
            kept: Kept::Paced { pacing, .. },
            ..
        } = &mut controller.moves
        {
            pacing.pace = Some(2);
            pacing.speed_mps = paces.paces[2].speed_mps;
        }
        let from = controller.point().u;
        let mut highest_m = 0.0f64;
        for frame in 0..180 {
            let wish = Wish {
                up: jump && frame < 3,
                ..Wish::default()
            };
            controller.update(1.0 / 60.0, wish, &generator, built);
            controller.ride_at(1.4, 1.0 / 60.0);
            if !controller.grounded {
                highest_m = highest_m.max(controller.height_m - floor_m);
            }
            if let Some(ceiling_m) = ceiling_m {
                let under_m = ceiling_m - controller.height_m;
                assert!(under_m >= paces.room_m - 1e-9, "its back in the roof");
                assert!(controller.eye_m() < under_m, "the eye in the roof");
            }
        }
        let galloping = matches!(
            &controller.moves,
            Moves::Way {
                kept: Kept::Paced { pacing, .. },
                ..
            } if pacing.pace == Some(2)
        );
        (
            (controller.point().u - from) * BLOCK_M,
            highest_m,
            galloping,
        )
    }

    /// A galloping horse jumps as high as its gallop's jump on open ground
    /// and gallops on; it flies a wall a walker could not step and its arc
    /// clears, and balks at one it does not, or unasked.
    #[test]
    fn a_horse_clears_only_what_its_jump_clears() {
        let (_, highest_m, galloping) = gallop_at(0, true, None);
        assert!((highest_m - 1.2).abs() < 0.1, "{highest_m}");
        assert!(galloping);

        let (far_m, _, galloping) = gallop_at(2, true, None);
        assert!(far_m > 9.0 && galloping, "over a metre: {far_m}");
        let (far_m, _, galloping) = gallop_at(2, false, None);
        assert!(far_m < 6.0 && !galloping, "unasked: {far_m}");
        let (far_m, _, galloping) = gallop_at(4, true, None);
        assert!(far_m < 6.0 && !galloping, "two metres: {far_m}");
    }

    /// A body on a floor and a horse eight cells along `u` from it, facing
    /// along `u`, with walls two cells high on the columns `walls`, offsets
    /// from the column the horse stands on: what `then` is handed.
    fn by_a_horse(
        walls: &[[i32; 2]],
        then: impl FnOnce(&mut Controller, &Near, &Generator, Built<'_>),
    ) {
        let generator = Generator::new(Recipe::new(1)).unwrap();
        let side = f64::from(generator.sphere().blocks().side());
        let near = SurfacePoint::new(Sector::new(4).unwrap(), side * 0.41, side * 0.37);
        let mut build = Cells::default();
        let [plot, y, top] = build.floor(&generator, near);
        let floor_m = f64::from(top) * BLOCK_M;
        let [u, v] = [plot + 4, y + 20];
        let horse = [u + 4, v];
        let gestures: Vec<Gesture> = walls
            .iter()
            .map(|[du, dv]| {
                let [x, y] = [horse[0] + du, horse[1] + dv];
                Gesture::Create {
                    span: Span::between([x, y, top], [x, y, top + 1]),
                    paint: 0,
                }
            })
            .collect();
        if !gestures.is_empty() {
            assert!(build.apply(&generator, near.sector, &gestures));
        }
        let built = Built {
            cells: &build,
            parts: &Parts::default(),
            trunks: &crate::Trunks::default(),
        };
        let grid = generator.sphere().blocks();
        let at = |[u, v]: [i32; 2]| {
            SurfacePoint::new(near.sector, f64::from(u) + 0.5, f64::from(v) + 0.5)
        };
        let mut controller = Controller::spawn(at([u, v]), &generator);
        controller.lift_onto(floor_m);
        let radius_m = generator.sphere().radius_m() + floor_m;
        let animal = Near {
            number: 1,
            kind: &gaia::fauna::horse::HORSE,
            at: DVec3::from(grid.direction(at(horse))) * radius_m,
            heading: DVec3::from(grid.tangents(at(horse)).du).normalize(),
            ridden: false,
            keeping: crate::animals::Keeping::Wild,
            restless: 0.5,
            speed_share: 1.0,
            size: 1.0,
        };
        then(&mut controller, &animal, &generator, built);
    }

    /// A body on a floor in paces on a horse born with wings, numbered 1,
    /// under a roof `roof_m` over the floor if any: what `then` is handed.
    fn on_a_pegasus(
        roof_m: Option<f64>,
        then: impl FnOnce(&mut Controller, &Generator, Built<'_>),
    ) {
        let generator = Generator::new(Recipe::new(1)).unwrap();
        let side = f64::from(generator.sphere().blocks().side());
        let near = SurfacePoint::new(Sector::new(4).unwrap(), side * 0.41, side * 0.37);
        let mut build = Cells::default();
        let [x, y, top] = build.floor(&generator, near);
        if let Some(roof_m) = roof_m {
            let roof = top + (roof_m / BLOCK_M) as i32;
            let span = Span::between([x - 8, y - 8, roof], [x + 8, y + 8, roof]);
            assert!(build.apply(
                &generator,
                near.sector,
                &[Gesture::Create { span, paint: 0 }]
            ));
        }
        let built = Built {
            cells: &build,
            parts: &Parts::default(),
            trunks: &Trunks::default(),
        };
        let at = SurfacePoint::new(near.sector, f64::from(x) + 0.5, f64::from(y) + 0.5);
        let mut controller = Controller::spawn(at, &generator);
        controller.lift_onto(f64::from(top) * BLOCK_M);
        assert!(controller.take_way(
            "test.horse",
            &Profile::Paces(Box::new(crate::paces::tests::pegasus())),
            Steering::Steered,
            1,
            &generator
        ));
        then(&mut controller, &generator, built);
    }

    /// Runs `controller` for `seconds` at sixty frames a second.
    fn run(
        controller: &mut Controller,
        seconds: f64,
        wish: Wish,
        generator: &Generator,
        built: Built<'_>,
    ) {
        for _ in 0..(seconds * 60.0).round() as usize {
            controller.update(1.0 / 60.0, wish, generator, built);
        }
    }

    /// On a horse with wings, taking off climbs to the way's take-off
    /// height and says it flies from the press; flown, it goes faster than
    /// a body on its own at that height; landing, it comes down onto the
    /// floor, in the saddle still, at a pace.
    #[test]
    fn a_winged_mount_takes_off_flies_faster_than_a_body_and_lands() {
        on_a_pegasus(None, |controller, generator, built| {
            let floor_m = controller.height_m;
            assert_eq!(
                controller.take_off(false, generator, built),
                Err(Refused::Flightless)
            );
            assert!(!controller.aloft());
            assert_eq!(controller.take_off(true, generator, built), Ok(()));
            assert!(controller.aloft());
            assert_eq!(crate::Gait::of(controller), crate::Gait::Fly);
            run(controller, 3.0, Wish::default(), generator, built);
            let over_m = controller.height_m - floor_m;
            assert!((over_m - 3.0).abs() < 0.2, "{over_m} m over the floor");
            assert_eq!(
                controller.mount_aloft().map(|(_, aloft)| aloft),
                Some(Aloft::Flying)
            );

            let ahead = Wish {
                movement: [0.0, 1.0],
                ..Wish::default()
            };
            run(controller, 1.0, ahead, generator, built);
            let clearance = controller.clearance_m(generator);
            let body_mps = FLY_MIN_MPS + clearance * FLY_PER_ALTITUDE;
            assert!(
                controller.speed_mps() > body_mps * 1.2,
                "{} m/s",
                controller.speed_mps()
            );

            assert_eq!(controller.land(generator, built), Ok(()));
            run(controller, 6.0, Wish::default(), generator, built);
            assert!(!controller.aloft() && controller.grounded());
            assert_eq!(
                (controller.carrier(), controller.mode()),
                (1, Mode::Way("test.horse".into()))
            );
            let ground_m = controller.footing(generator, Some(built)).floor_m.unwrap();
            assert!((controller.height_m - ground_m).abs() < 0.01);
            assert!(controller.pace().is_some(), "down at a pace");
        });
    }

    /// Taken off, a pegasus gathers on its feet, on the floor, for its
    /// spring's gather, then springs off them: it rises slowing to its
    /// spring's height at the top, carried forward as fast as it went and
    /// at least its spring's least, and is on its wings from there. Its
    /// gait is the flight's from the press, as peers see it.
    #[test]
    fn a_pegasus_springs_off_its_legs_and_takes_wing_at_the_top() {
        let spring = crate::paces::tests::pegasus()
            .flight
            .expect("a pegasus flies")
            .spring;
        for trot in [false, true] {
            on_a_pegasus(None, |controller, generator, built| {
                let floor_m = controller.height_m;
                if trot {
                    let ahead = Wish {
                        movement: [0.0, 1.0],
                        ..Wish::default()
                    };
                    run(controller, 1.5, ahead, generator, built);
                }
                let went_mps = controller.speed_mps();
                let from = controller.position();
                assert_eq!(controller.take_off(true, generator, built), Ok(()));
                let mut over_m = Vec::new();
                for frame in 1..=((spring.top_s() + 0.1) * 60.0).round() as usize {
                    controller.update(1.0 / 60.0, Wish::default(), generator, built);
                    let s = frame as f64 / 60.0;
                    over_m.push((s, controller.height_m - floor_m));
                    assert!(controller.aloft(), "{s} s: aloft from the press");
                    assert_eq!(crate::Gait::of(controller), crate::Gait::Fly, "{s} s");
                    // On its wings from the frame its clock passes the top.
                    let past = s - spring.top_s();
                    if past.abs() > 0.5 / 60.0 {
                        assert_eq!(controller.flying(), past > 0.0, "{s} s");
                    }
                }
                for &(s, over) in &over_m {
                    if s <= spring.gather_s {
                        assert!(over.abs() < 0.01, "{s} s: {over} m, gathering");
                    }
                }
                let top = over_m
                    .iter()
                    .find(|(s, _)| (s - spring.top_s()).abs() < 0.5 / 60.0)
                    .expect("the top")
                    .1;
                assert!(
                    (top - spring.height_m).abs() < 0.05,
                    "{top} m over the floor at the top"
                );
                let rises = over_m.windows(2).map(|pair| pair[1].1 - pair[0].1);
                assert!(rises.clone().all(|rise| rise > -1e-9), "it never sinks");
                let gone_m = (controller.position() - from).length();
                let least_m = went_mps.max(spring.forward_mps) * spring.rise_s;
                assert!(gone_m > least_m, "{gone_m} m on from where it pressed");
            });
        }
    }

    /// Landed flying fast, a pegasus comes down at the fastest pace forward
    /// steps to, never at one sprint alone reaches.
    #[test]
    fn a_pegasus_landed_fast_gallops_on() {
        on_a_pegasus(None, |controller, generator, built| {
            assert_eq!(controller.take_off(true, generator, built), Ok(()));
            run(controller, 3.0, Wish::default(), generator, built);
            let ahead = Wish {
                movement: [0.0, 1.0],
                ..Wish::default()
            };
            run(controller, 2.0, ahead, generator, built);
            assert_eq!(controller.land(generator, built), Ok(()));
            let mut landed_mps = 0.0;
            for _ in 0..600 {
                landed_mps = controller.speed_mps();
                run(controller, 0.01, ahead, generator, built);
                if !controller.aloft() {
                    break;
                }
            }
            assert!(!controller.aloft(), "landed");
            assert!(landed_mps > 11.0, "{landed_mps} m/s as it touched");
            assert_eq!(controller.pace(), Some(2), "{} m/s", controller.speed_mps());
        });
    }

    /// Open land far off, on another sector: where a far move lands a
    /// rider.
    fn far_off(generator: &Generator) -> SurfacePoint {
        let side = f64::from(generator.sphere().blocks().side());
        SurfacePoint::new(Sector::new(0).unwrap(), side * 0.5, side * 0.5)
    }

    fn bearing(controller: &Controller) -> f64 {
        topology::bearing_deg(controller.up().to_array(), controller.facing.to_array()).unwrap()
    }

    /// Moved far at once, a rider arrives on what it rode, at the pace it
    /// went and facing the same way by the compass; turned there, the
    /// horse turns with it and goes on that way.
    #[test]
    fn a_rider_moved_far_arrives_mounted_at_its_pace_and_bearing() {
        on_a_pegasus(None, |controller, generator, built| {
            let ahead = Wish {
                movement: [0.0, 1.0],
                ..Wish::default()
            };
            run(controller, 1.0 / 60.0, ahead, generator, built);
            run(controller, 2.0, Wish::default(), generator, built);
            let (pace, speed_mps, facing) = (
                controller.pace(),
                controller.speed_mps(),
                bearing(controller),
            );
            assert_eq!(pace, Some(0), "walking");
            controller.teleport(far_off(generator), generator);
            controller.stand_at(None, generator);
            assert_eq!(
                (controller.way(), controller.carrier()),
                (Some("test.horse"), 1)
            );
            assert!(
                (bearing(controller) - facing).abs() < 0.5,
                "{} then {facing}",
                bearing(controller)
            );
            run(controller, 1.0, Wish::default(), generator, built);
            assert_eq!(controller.pace(), pace);
            assert!(
                (controller.speed_mps() - speed_mps).abs() < 0.1,
                "{} m/s",
                controller.speed_mps()
            );
            let turned = (facing + 90.0) % 360.0;
            controller.face(turned);
            run(controller, 0.5, Wish::default(), generator, built);
            assert!(
                (bearing(controller) - turned).abs() < 0.5,
                "{} not {turned}",
                bearing(controller)
            );
        });
    }

    /// A pegasus aloft arrives aloft where the place says a height, as a
    /// body on foot arrives flying, and lands where it says none. A horse on
    /// its feet sent to a height comes down onto what is under it, ridden.
    #[test]
    fn a_pegasus_aloft_arrives_aloft_at_a_height_and_lands_at_none() {
        on_a_pegasus(None, |controller, generator, built| {
            assert_eq!(controller.take_off(true, generator, built), Ok(()));
            run(controller, 3.0, Wish::default(), generator, built);
            controller.teleport(far_off(generator), generator);
            let ground_m = controller.ground_m(generator);
            controller.stand_at(Some(ground_m + 40.0), generator);
            run(controller, 1.0, Wish::default(), generator, built);
            assert!(controller.aloft(), "aloft");
            assert!(
                controller.height_m > ground_m + 35.0,
                "{} m up",
                controller.height_m - ground_m
            );

            controller.teleport(far_off(generator), generator);
            controller.stand_at(None, generator);
            assert!(!controller.aloft() && controller.grounded(), "landed");
            assert_eq!(controller.way(), Some("test.horse"));

            controller.teleport(far_off(generator), generator);
            controller.stand_at(Some(ground_m + 3.0), generator);
            assert!(!controller.aloft() && !controller.grounded());
            run(controller, 2.0, Wish::default(), generator, built);
            assert!(controller.grounded(), "down on the ground");
            assert_eq!(
                (controller.way(), controller.carrier()),
                (Some("test.horse"), 1)
            );
        });
    }

    /// Flown to its way's ceiling, a pegasus is at it; below, or rising
    /// from the ground, it is not.
    #[test]
    fn a_pegasus_at_its_ceiling_says_so() {
        on_a_pegasus(None, |controller, generator, built| {
            assert_eq!(controller.take_off(true, generator, built), Ok(()));
            assert!(!controller.at_ceiling(), "rising");
            run(controller, 3.0, Wish::default(), generator, built);
            let ceiling_m = controller.mount_aloft().unwrap().0.ceiling_m;
            controller.height_m = ceiling_m - 10.0;
            assert!(!controller.at_ceiling());
            controller.height_m = ceiling_m;
            assert!(controller.at_ceiling());
        });
    }

    /// Flown up, a pegasus climbs through the clouds to their top over the
    /// sea and no higher, and says there it is at its ceiling. Put higher
    /// by a far move, it arrives at its ceiling; landed, it is down in
    /// seconds, never under the ground.
    #[test]
    fn a_pegasus_flies_up_to_the_clouds_top_and_no_higher() {
        on_a_pegasus(None, |controller, generator, built| {
            let top_m = f64::from(scene::CLOUD_TOP_M);
            assert_eq!(controller.take_off(true, generator, built), Ok(()));
            run(controller, 3.0, Wish::default(), generator, built);
            let up = Wish {
                up: true,
                sprint: true,
                movement: [0.0, 1.0],
                ..Wish::default()
            };
            let mut highest_m = 0.0f64;
            for _ in 0..90 * 60 {
                controller.update(1.0 / 60.0, up, generator, built);
                highest_m = highest_m.max(controller.height_m);
            }
            assert!(
                controller.at_ceiling(),
                "{} m over the sea",
                controller.height_m
            );
            assert!(highest_m <= top_m + 1.0, "{highest_m} m over the sea");

            controller.stand_at(Some(top_m + 2000.0), generator);
            assert_eq!(controller.height_m, top_m);
            assert!(controller.aloft() && controller.at_ceiling());

            // From there it comes down to the ground in seconds, as from
            // anywhere: slower the nearer.
            let ground_m = controller.ground_m(generator);
            assert_eq!(controller.land(generator, built), Ok(()));
            let mut s = 0.0;
            while controller.aloft() && s < 20.0 {
                controller.update(1.0 / 60.0, Wish::default(), generator, built);
                assert!(
                    controller.height_m >= ground_m - 0.01,
                    "under the ground at {s} s"
                );
                s += 1.0 / 60.0;
            }
            assert!(controller.grounded(), "still aloft after {s} s");
        });
    }

    /// A rider with a build tool in hand still lands on a volume's cells,
    /// never among them: what holds a mount on its feet holds it aloft.
    #[test]
    fn a_mount_lands_on_cells_with_a_build_tool_in_hand() {
        on_a_pegasus(None, |controller, generator, built| {
            let floor_m = controller.height_m;
            controller.pass_cells(true);
            assert_eq!(controller.take_off(true, generator, built), Ok(()));
            run(controller, 3.0, Wish::default(), generator, built);
            assert_eq!(controller.land(generator, built), Ok(()));
            run(controller, 6.0, Wish::default(), generator, built);
            assert!(!controller.aloft() && controller.grounded());
            assert!(
                (controller.height_m - floor_m).abs() < 0.01,
                "{} m under the cells' top",
                floor_m - controller.height_m
            );
        });
    }

    /// Under the ground a pegasus has rock over it: no room to take off,
    /// never no wings.
    #[test]
    fn a_take_off_in_a_cave_is_refused_for_room() {
        on_a_pegasus(None, |controller, generator, built| {
            controller.height_m = controller.ground_m(generator) - 10.0;
            assert_eq!(
                controller.take_off(true, generator, built),
                Err(Refused::NoRoom)
            );
            assert!(!controller.aloft());
        });
    }

    /// A roof lower than the horse and its take-off over it refuses the
    /// take-off; the readout says why for a moment and forgets it.
    #[test]
    fn a_take_off_under_a_low_roof_is_refused_and_said() {
        on_a_pegasus(Some(4.0), |controller, generator, built| {
            assert_eq!(
                controller.take_off(true, generator, built),
                Err(Refused::NoRoom)
            );
            assert!(!controller.aloft());
            assert_eq!(controller.refused(), Some(Refused::NoRoom));
            run(
                controller,
                REFUSED_S + 0.1,
                Wish::default(),
                generator,
                built,
            );
            assert_eq!(controller.refused(), None);
        });
        on_a_pegasus(Some(8.0), |controller, generator, built| {
            assert_eq!(controller.take_off(true, generator, built), Ok(()));
        });
    }

    /// Over the deep sea a landing is refused and the body stays aloft;
    /// flown down, sprinting, at any frame rate, it is held over the water
    /// every frame and never goes in.
    #[test]
    fn over_the_deep_sea_a_winged_mount_stays_aloft() {
        let generator = Generator::new(Recipe::new(1)).unwrap();
        let grid = generator.sphere().blocks();
        let side = f64::from(grid.side());
        let sea = (1..40)
            .flat_map(|i| (1..40).map(move |j| (i, j)))
            .map(|(i, j)| {
                let (u, v) = (f64::from(i) / 40.0 * side, f64::from(j) / 40.0 * side);
                SurfacePoint::new(Sector::ALL[0], u, v)
            })
            .find(|point| generator.sample(grid.direction(*point)).height_m < -200.0)
            .expect("seed 1 has deep sea in sector 0");
        let (cells, parts) = (Cells::default(), Parts::default());
        let built = Built {
            cells: &cells,
            parts: &parts,
            trunks: &Trunks::default(),
        };
        for fps in [60.0, 30.0, 10.0] {
            let mut controller = Controller::spawn(sea, &generator);
            assert!(controller.take_way(
                "test.horse",
                &Profile::Paces(Box::new(crate::paces::tests::pegasus())),
                Steering::Steered,
                1,
                &generator
            ));
            controller.height_m = 30.0;
            if let Moves::Way {
                kept: Kept::Paced { aloft, .. },
                ..
            } = &mut controller.moves
            {
                *aloft = Some(Aloft::Flying);
            }
            assert_eq!(controller.land(&generator, built), Err(Refused::NoGround));
            assert!(controller.aloft());
            assert_eq!(controller.refused(), Some(Refused::NoGround));
            controller.set_pitch(-1.2);
            let dive = Wish {
                movement: [0.0, 1.0],
                sprint: true,
                ..Wish::default()
            };
            let mut lowest_m = f64::INFINITY;
            for _ in 0..(4.0 * fps) as usize {
                controller.update(1.0 / fps, dive, &generator, built);
                lowest_m = lowest_m.min(controller.height_m);
            }
            assert!(controller.aloft(), "at {fps} fps");
            assert!(lowest_m > SEA_OVER_M - 0.01, "{lowest_m} m at {fps} fps");
        }
    }

    /// Aloft, the eye rolls with the horse's bank by the share the way's
    /// flight says, eased; on the ground it stays level.
    #[test]
    fn the_eye_banks_with_a_winged_mount_only_aloft() {
        on_a_pegasus(None, |controller, generator, built| {
            for _ in 0..120 {
                controller.bank_at(0.5, 1.0 / 60.0);
            }
            assert_eq!(controller.banked.value(), 0.0);
            assert_eq!(controller.take_off(true, generator, built), Ok(()));
            for _ in 0..120 {
                controller.bank_at(0.5, 1.0 / 60.0);
            }
            assert!(
                (controller.banked.value() - 0.15).abs() < 0.01,
                "{}",
                controller.banked.value()
            );
        });
    }

    /// Getting on, the body walks to the horse: a wall between them, it
    /// stays on foot where it stood. Getting off, it steps to the horse's
    /// left, to its right where a wall stands at its left, and stays in the
    /// saddle's place with walls at both.
    #[test]
    fn a_body_gets_on_and_off_only_where_it_walks() {
        fn mount(
            controller: &mut Controller,
            animal: &Near,
            generator: &Generator,
            built: Built<'_>,
        ) -> bool {
            let paces = Profile::Paces(Box::new(crate::paces::tests::horse()));
            controller.mount(
                "test.horse",
                &paces,
                Steering::Steered,
                animal,
                generator,
                built,
            )
        }
        by_a_horse(&[[-2, 0]], |controller, animal, generator, built| {
            let stood = controller.point();
            assert!(!mount(controller, animal, generator, built));
            assert_eq!((controller.point(), controller.carrier()), (stood, 0));
        });
        // Which way is its left, in `v`, as the body steps off.
        let mut left = 0.0;
        by_a_horse(&[], |controller, animal, generator, built| {
            assert!(mount(controller, animal, generator, built));
            let sat = controller.point();
            assert_eq!(controller.carrier(), 1);
            controller.step_off(generator, built);
            assert_eq!(controller.carrier(), 0);
            left = controller.point().v - sat.v;
            assert!((left.abs() - 2.0).abs() < 0.1, "{left}");
            assert!((controller.point().u - sat.u).abs() < 0.1);
        });
        let side = |sign: f64| (left * sign / 2.0).round() as i32 * 3;
        let walled = |at: &[i32]| -> Vec<[i32; 2]> {
            at.iter()
                .flat_map(|&v| (-1..=1).map(move |u| [u, v]))
                .collect()
        };
        for (walls, went) in [
            (walled(&[side(1.0)]), -left),
            (walled(&[side(1.0), side(-1.0)]), 0.0),
        ] {
            by_a_horse(&walls, |controller, animal, generator, built| {
                assert!(mount(controller, animal, generator, built));
                let sat = controller.point();
                controller.step_off(generator, built);
                let stepped = controller.point().v - sat.v;
                assert!((stepped - went).abs() < 0.1, "{stepped} for {went}");
            });
        }
    }

    /// A large horse carries its rider faster than the way's paces, a small
    /// one slower, as much as its size says.
    #[test]
    fn a_body_mounted_goes_as_fast_as_the_animal_under_it() {
        for share in [0.8, 1.0, 1.2] {
            by_a_horse(&[], |controller, animal, generator, built| {
                let animal = Near {
                    speed_share: share,
                    ..*animal
                };
                let way = crate::paces::tests::horse();
                assert!(controller.mount(
                    "test.horse",
                    &Profile::Paces(Box::new(way.clone())),
                    Steering::Steered,
                    &animal,
                    generator,
                    built,
                ));
                let Moves::Way {
                    kept: Kept::Paced { paces, .. },
                    ..
                } = &controller.moves
                else {
                    panic!("not in paces");
                };
                let gallop = paces.paces[2].speed_mps;
                assert!((gallop - way.paces[2].speed_mps * share).abs() < 1e-9);
            });
        }
    }

    /// Under a roof three metres up, room for a horse and its rider, it
    /// gallops on; asked to jump, it balks, since its arc would put its
    /// back in the roof, and a metre wall there stops it all the same. The
    /// rider's eye, saddled over two and a half metres, stays under it.
    #[test]
    fn a_horse_under_a_roof_jumps_only_where_its_arc_has_room() {
        let (far_m, _, galloping) = gallop_at(0, false, Some(6));
        assert!(far_m > 9.0 && galloping, "{far_m}");
        let (_, highest_m, galloping) = gallop_at(0, true, Some(6));
        assert_eq!(highest_m, 0.0, "jumped under the roof");
        assert!(!galloping, "balks");
        let (far_m, highest_m, galloping) = gallop_at(2, true, Some(6));
        assert!(far_m < 6.0 && !galloping, "{far_m}");
        assert_eq!(highest_m, 0.0, "jumped under the roof");
    }

    /// In paces, ground too small to be a step leaves the eye as a step
    /// does, so it never flips between followed and eased; and the saddle's
    /// stride, rising and falling a decimetre twice a second, barely moves
    /// the eye that rides on it.
    #[test]
    fn a_rider_s_eye_is_eased_over_any_ground_and_never_rocks() {
        let generator = Generator::new(Recipe::new(1)).unwrap();
        let side = f64::from(generator.sphere().blocks().side());
        let point = SurfacePoint::new(Sector::new(4).unwrap(), side * 0.41, side * 0.37);
        let mut controller = Controller::spawn(point, &generator);
        assert!(controller.take_way(
            "test.horse",
            &Profile::Paces(Box::new(crate::paces::tests::horse())),
            Steering::Steered,
            1,
            &generator
        ));
        controller.stand_on(controller.height_m + 0.05);
        assert!((controller.eye.off + 0.05).abs() < 1e-9);
        assert_eq!(controller.figure, Spring::default());

        let saddle_m = 1.4;
        for _ in 0..120 {
            controller.ride_at(saddle_m, 1.0 / 60.0);
        }
        let mut lowest = f64::MAX;
        let mut highest = f64::MIN;
        for frame in 0..600 {
            let t = f64::from(frame) / 60.0;
            let stride_m = 0.1 * (core::f64::consts::TAU * 2.0 * t).sin();
            controller.ride_at(saddle_m + stride_m, 1.0 / 60.0);
            if frame > 120 {
                lowest = lowest.min(controller.riding.value());
                highest = highest.max(controller.riding.value());
            }
        }
        assert!(highest - lowest < 0.02, "{} m", highest - lowest);
    }

    #[test]
    fn a_platform_laid_under_a_body_takes_the_eye_with_it() {
        let generator = Generator::new(Recipe::new(1)).unwrap();
        let side = f64::from(generator.sphere().blocks().side());
        let point = SurfacePoint::new(Sector::new(4).unwrap(), side * 0.41, side * 0.37);
        let mut controller = Controller::spawn(point, &generator);
        controller.lift_onto(controller.height_m + 4.0);
        assert_eq!(controller.eye, Spring::default());
        assert_eq!(controller.figure, Spring::default());
    }

    #[test]
    fn ground_is_followed_at_once_and_a_step_is_eased() {
        let generator = Generator::new(Recipe::new(1)).unwrap();
        let side = f64::from(generator.sphere().blocks().side());
        let point = SurfacePoint::new(Sector::new(4).unwrap(), side * 0.41, side * 0.37);
        let mut controller = Controller::spawn(point, &generator);
        // A slope: a few centimetres a frame, up and down.
        for rise_m in [0.05, 0.12, -0.08, -0.2] {
            controller.stand_on(controller.height_m + rise_m);
        }
        assert_eq!(controller.figure, Spring::default());
        // A stair: one block, left behind whole.
        controller.stand_on(controller.height_m + BLOCK_M);
        assert!((controller.figure.off + BLOCK_M).abs() < 1e-9);
        assert!((controller.eye.off + BLOCK_M).abs() < 1e-9);
        // The world moving under the body: more than a step, followed at once.
        controller.figure = Spring::default();
        controller.stand_on(controller.height_m + 2.0 * BLOCK_M);
        assert_eq!(controller.figure, Spring::default());
    }

    #[test]
    fn a_way_is_taken_over_the_ground_and_never_in_a_cave() {
        let generator = Generator::new(Recipe::new(1)).unwrap();
        let side = f64::from(generator.sphere().blocks().side());
        let point = SurfacePoint::new(Sector::new(4).unwrap(), side * 0.41, side * 0.37);
        let mut controller = Controller::spawn(point, &generator);
        let ground_m = controller.ground_m(&generator);
        controller.height_m = ground_m - 6.0;
        assert!(!controller.take_way(
            "test.way",
            &Profile::Hover(BOARD),
            Steering::Steered,
            0,
            &generator
        ));
        assert_eq!(controller.mode(), Mode::Walk);
        assert_eq!(controller.height_m, ground_m - 6.0);
        controller.height_m = ground_m;
        assert!(controller.take_way(
            "test.way",
            &Profile::Hover(BOARD),
            Steering::Steered,
            0,
            &generator
        ));
    }

    #[test]
    fn in_first_person_the_camera_is_the_eye_over_the_ground_and_in_rock() {
        let generator = Generator::new(Recipe::new(1)).unwrap();
        let side = f64::from(generator.sphere().blocks().side());
        let point = SurfacePoint::new(Sector::new(4).unwrap(), side * 0.41, side * 0.37);
        let mut controller = Controller::spawn(point, &generator);
        controller.set_pitch(-0.4);
        let ground_m = controller.ground_m(&generator);
        // On the ground, then in rock: a body under it is in a cave, where
        // the boom is cut by the rock behind it, and here all of it is rock.
        for (feet_m, behind_m) in [(ground_m, 6.0), (ground_m - 6.0, 0.0)] {
            controller.height_m = feet_m;
            let up = controller.frame_up;
            let eye = controller.position() + up * EYE_M;
            let forward = controller.view * controller.pitch.cos() + up * controller.pitch.sin();

            controller.set_boom(6.0);
            let third = controller.camera(&generator);
            assert!(((third.position - eye).length() - behind_m).abs() < 0.1);

            controller.set_boom(0.0);
            let first = controller.camera(&generator);
            assert!((first.position - eye).length() < 1e-9, "{feet_m}");
            let look = first.rotation * DVec3::NEG_Z;
            assert!(look.dot(forward) > 1.0 - 1e-9, "{look} {forward}");
            assert!(!controller.sees_own_body());
        }
    }

    #[test]
    fn on_a_tall_view_the_third_person_camera_aims_at_the_body_s_middle() {
        let generator = Generator::new(Recipe::new(1)).unwrap();
        let side = f64::from(generator.sphere().blocks().side());
        let point = SurfacePoint::new(Sector::new(4).unwrap(), side * 0.41, side * 0.37);
        let mut controller = Controller::spawn(point, &generator);
        controller.set_pitch(-0.1);
        let up = controller.frame_up;
        let seen = |controller: &Controller| controller.camera(&generator).position;

        controller.set_boom(6.0);
        let wide = seen(&controller);
        controller.set_tall(true);
        let tall = seen(&controller);
        assert!(((wide - tall).dot(up) - TALL_AIM_UNDER_M).abs() < 1e-6);
        assert!((wide - tall - up * TALL_AIM_UNDER_M).length() < 1e-6);

        // At the eye, the eye, whatever the view.
        controller.set_boom(0.0);
        let eye = controller.position() + up * EYE_M;
        assert!((seen(&controller) - eye).length() < 1e-9);
    }

    #[test]
    fn a_flyer_is_stopped_by_a_wall_unless_it_is_let_pass() {
        // Its middle stops half a body short of the wall, as a walker's does.
        let short = fly_at_a_wall(false);
        assert!((0.6..1.2).contains(&short), "{short}");
        let past = fly_at_a_wall(true);
        assert!(past < -4.0, "{past}");
    }

    /// The spans of a wall, `[from, to]` cells from a column, `u` then `v`.
    type Walls<'a> = &'a [[[i32; 2]; 2]];

    /// A horse in paces at a halt on a floor, facing along `u`, and walls
    /// `high` cells high over `walls` from the column it stands on: what
    /// `then` is handed, with that column.
    fn paced_by(
        walls: Walls<'_>,
        high: i32,
        then: impl FnOnce(&mut Controller, &Generator, Built<'_>, [i32; 2]),
    ) {
        let generator = Generator::new(Recipe::new(1)).unwrap();
        let side = f64::from(generator.sphere().blocks().side());
        let near = SurfacePoint::new(Sector::new(4).unwrap(), side * 0.41, side * 0.37);
        let mut build = Cells::default();
        let [plot, y, top] = build.floor(&generator, near);
        let stand = [plot + 20, y + 32];
        let gestures: Vec<Gesture> = walls
            .iter()
            .map(|&[[u, v], [to_u, to_v]]| Gesture::Create {
                span: Span::between(
                    [stand[0] + u, stand[1] + v, top],
                    [stand[0] + to_u, stand[1] + to_v, top + high - 1],
                ),
                paint: 0,
            })
            .collect();
        if !gestures.is_empty() {
            assert!(build.apply(&generator, near.sector, &gestures));
        }
        let built = Built {
            cells: &build,
            parts: &Parts::default(),
            trunks: &crate::Trunks::default(),
        };
        let grid = generator.sphere().blocks();
        let at = SurfacePoint::new(
            near.sector,
            f64::from(stand[0]) + 0.5,
            f64::from(stand[1]) + 0.5,
        );
        let mut controller = Controller::spawn(at, &generator);
        controller.lift_onto(f64::from(top) * BLOCK_M);
        controller.face(topology::bearing_deg(grid.direction(at), grid.tangents(at).du).unwrap());
        let paces = Profile::Paces(Box::new(crate::paces::tests::horse()));
        assert!(controller.take_way("test.horse", &paces, Steering::Steered, 1, &generator));
        then(&mut controller, &generator, built, stand);
    }

    /// Sets a body in paces going at `pace`.
    fn set_going(controller: &mut Controller, pace: usize) {
        if let Moves::Way {
            kept: Kept::Paced { paces, pacing, .. },
            ..
        } = &mut controller.moves
        {
            pacing.pace = Some(pace);
            pacing.speed_mps = paces.paces[pace].speed_mps;
        }
    }

    /// Whether a column of `walls`, from `stand`, is under the body: its
    /// seat or a point of its spine.
    fn in_walls(controller: &Controller, stand: [i32; 2], walls: Walls<'_>) -> bool {
        let at = controller.point;
        let spine = controller.spine(controller.facing);
        std::iter::once([0.0, 0.0])
            .chain(spine.points())
            .map(|[du, dv]| SurfacePoint::new(at.sector, at.u + du, at.v + dv))
            .flat_map(collision::columns)
            .any(|[u, v]| {
                walls.iter().any(|&[[from_u, from_v], [to_u, to_v]]| {
                    (stand[0] + from_u..=stand[0] + to_u).contains(&u)
                        && (stand[1] + from_v..=stand[1] + to_v).contains(&v)
                })
            })
    }

    /// Where the body's nose and rump are along `u`, cells.
    fn nose_and_rump(controller: &Controller) -> [f64; 2] {
        let spine = controller.spine(controller.facing);
        let along = spine.points().map(|[du, _]| du);
        let (nose, rump) = along.fold((0.0f64, 0.0f64), |(n, r), du| (n.max(du), r.min(du)));
        [controller.point.u + nose, controller.point.u + rump]
    }

    /// A horse driven at a wall at every pace stops with its nose at it,
    /// never in it; backed at one, with its rump at it.
    #[test]
    fn a_horse_stops_with_its_nose_at_a_wall_and_backs_its_rump_to_one() {
        let ahead: Walls<'_> = &[[[8, -8], [8, 8]]];
        for pace in 0..4 {
            paced_by(ahead, 2, |controller, generator, built, stand| {
                set_going(controller, pace);
                for _ in 0..240 {
                    controller.update(1.0 / 60.0, Wish::default(), generator, built);
                    assert!(!in_walls(controller, stand, ahead), "pace {pace}: in it");
                }
                let [nose, _] = nose_and_rump(controller);
                let short = f64::from(stand[0] + 8) - (nose + collision::BODY_HALF);
                assert!(
                    (0.0..1.0).contains(&short),
                    "pace {pace}: {short} cells short"
                );
            });
        }
        let behind: Walls<'_> = &[[[-7, -8], [-7, 8]]];
        paced_by(behind, 2, |controller, generator, built, stand| {
            let back = Wish {
                movement: [0.0, -1.0],
                ..Wish::default()
            };
            for _ in 0..360 {
                controller.update(1.0 / 60.0, back, generator, built);
                assert!(!in_walls(controller, stand, behind), "backed into it");
            }
            let [_, rump] = nose_and_rump(controller);
            let short = rump - collision::BODY_HALF - f64::from(stand[0] - 6);
            assert!((0.0..1.0).contains(&short), "{short} cells short");
        });
    }

    /// A horse turned in place toward a wall beside it is held at the last
    /// facing its body fits; turned away, it comes round until its nose
    /// meets the wall from the other side.
    #[test]
    fn a_horse_turned_against_a_wall_is_held_at_the_last_facing_that_fits() {
        let beside: Walls<'_> = &[[[-8, 3], [8, 3]]];
        let turned = |turn: f32| {
            let mut turned = 0.0;
            paced_by(beside, 2, |controller, generator, built, stand| {
                let wish = Wish {
                    movement: [turn, 0.0],
                    ..Wish::default()
                };
                for _ in 0..180 {
                    let facing = controller.facing;
                    controller.update(1.0 / 60.0, wish, generator, built);
                    turned += facing.angle_between(controller.facing);
                    assert!(!in_walls(controller, stand, beside), "turned into it");
                }
            });
            turned
        };
        let mut both = [turned(-1.0), turned(1.0)];
        both.sort_by(f64::total_cmp);
        let [toward, away] = both;
        assert!(
            (0.2..std::f64::consts::FRAC_PI_2).contains(&toward),
            "toward it: {toward}"
        );
        assert!(away > std::f64::consts::PI, "away from it: {away}");
    }

    /// A rise of one block under the nose is a step to the whole body.
    #[test]
    fn a_horse_steps_up_a_block_under_its_nose() {
        paced_by(
            &[[[6, -8], [16, 8]]],
            1,
            |controller, generator, built, stand| {
                let floor_m = controller.height_m;
                set_going(controller, 0);
                for _ in 0..240 {
                    controller.update(1.0 / 60.0, Wish::default(), generator, built);
                }
                assert!(
                    controller.point.u > f64::from(stand[0] + 9),
                    "{}",
                    controller.point.u
                );
                assert!((controller.height_m - floor_m - BLOCK_M).abs() < 1e-6);
            },
        );
    }

    /// A horse ridden at a tree stops with its nose at the trunk.
    #[test]
    fn a_horse_stops_with_its_nose_at_a_trunk() {
        let generator = Generator::new(Recipe::new(1)).unwrap();
        let (plant, trunks, first) = crate::trunks::tests::a_tree(&generator);
        let root = plant.root.column;
        let from = SurfacePoint::new(
            root.sector,
            f64::from(root.u) - 7.5,
            f64::from(root.v) + 0.5,
        );
        let grid = generator.sphere().blocks();
        let (cells, parts) = (Cells::default(), Parts::default());
        let built = Built {
            cells: &cells,
            parts: &parts,
            trunks: &trunks,
        };
        let mut controller = Controller::spawn(from, &generator);
        controller.stand_at(None, &generator);
        let along = grid.tangents(from).du;
        controller.face(topology::bearing_deg(grid.direction(from), along).unwrap());
        let paces = Profile::Paces(Box::new(crate::paces::tests::horse()));
        assert!(controller.take_way("test.horse", &paces, Steering::Steered, 1, &generator));
        set_going(&mut controller, 1);
        for _ in 0..240 {
            controller.update(1.0 / 60.0, Wish::default(), &generator, built);
        }
        let [nose, _] = nose_and_rump(&controller);
        let short = f64::from(first) - (nose + collision::BODY_HALF);
        assert!((0.0..1.0).contains(&short), "{short} cells short");
    }

    /// A body on foot is where it stands: no point of a spine is looked at
    /// for it, so a step costs it what it did. In paces, a point a metre
    /// along its length at most, nose to rump.
    #[test]
    fn only_a_body_in_paces_has_a_spine() {
        paced_by(&[], 0, |controller, _, _, _| {
            assert_eq!(controller.spine(controller.facing).points().count(), 3);
            controller.walk();
            assert_eq!(controller.spine(controller.facing), Spine::NONE);
        });
    }
}
