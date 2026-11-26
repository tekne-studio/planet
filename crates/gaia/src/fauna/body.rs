//! One animal's body as it is drawn: where it stands, how far along its
//! stride, how its body rides its feet, and its boxes for a frame.
//!
//! [`Animal::step`] does all that asks the ground: it takes where the animal
//! is now, plants its feet and eases its body after the ground. Everything
//! else only reads what that left.

use core::f64::consts::{PI, TAU};

use glam::{DAffine3, DMat3, DQuat, DVec3, Vec3};
use scene::BoxPart;

use crate::fauna::ease::{Follow, smoothstep};
use crate::fauna::gait::{Gait, STILL_MPS};
use crate::fauna::kind::{Kind, Spring};
use crate::fauna::look::{Look, Traits};
use crate::fauna::rig::{LEGS, Leg, Rig};
use crate::fauna::wing::Wings;
use crate::fauna::{Ground, ik};

/// How much longer a leg may grow to keep its foot on the ground.
const STRETCH: f64 = 1.08;
/// The upper bone runs on into the body this far, so no gap opens at the
/// root as it swings.
const UPPER_INTO_M: f64 = 0.12;
/// How far a foot steps for a radian of a turn in place: what makes the
/// legs walk an animal round where it stands.
const TURN_STEP_M: f64 = 0.7;
/// How stiffly the body comes after the ground, per second, critically
/// damped: there in a third of a second, so the legs take a stair and the
/// body reads its weight.
const BODY_SETTLE_PER_S: f64 = 16.0;
/// The neck comes after the body slower still, so the head rides steadier.
const NECK_SETTLE_PER_S: f64 = 8.0;
/// A foot down whose ground moved more than this, metres at size 1, is on
/// ground that is gone: it lifts and swings to the ground there now.
const GONE_M: f64 = 0.15;
/// A foot down further than this from where its gait has it, metres at
/// size 1, steps there.
const SLIP_M: f64 = 0.45;
/// Halted, a foot down further than this from where it stands, metres at
/// size 1, steps there once every other foot is down: it stands square.
const SQUARE_M: f64 = 0.05;
/// Moved further than this in a step past what its speed carries it,
/// metres, the animal is set anew where it is: its feet and its body leave
/// what they stood on.
const LEAP_M: f64 = 2.0;
/// How much further than its speed says an animal may go in a step and
/// still be going there, as a share: what a speed eased up adds to it.
const CARRIED: f64 = 1.5;
/// Slower than this, metres a second at size 1, an animal does not travel:
/// it steps, and is not heard.
const TRAVEL_MPS: f64 = 0.7;
/// How long it travels before its beats are heard, seconds, on the ground
/// or carried through the air: a step or two of a herd's shuffle never is,
/// and one that comes down from a jump or a flight at a pace is heard at
/// its next beat.
const HEARD_AFTER_S: f64 = 0.8;
/// The most the body is left over where the ground puts it, metres at
/// size 1: what its legs reach down by stretching.
const HANG_M: f64 = 0.06;
/// How far under its root a foot comes at least, in leg lengths, and over
/// how much more it eases into that: a foot lifted fast slows into its
/// tuck, never stops dead in it.
const TUCK: f64 = 0.3;
const TUCK_SOFT: f64 = 0.15;
/// How far the body sinks to let its longest leg down reach, at most, at
/// size 1.
const SINK_M: f64 = 0.4;
/// How far the body sinks at most, at size 1, for a foot down behind it at
/// the end of a long stride: past it the hoof peels off the ground.
const PEEL_M: f64 = 0.05;
/// How far behind its leg's root a foot down peels all it must, at size 1:
/// one under the body peels nothing.
const PEEL_BEHIND_M: f64 = 0.25;
/// From this far through its swing a foot in the air is reached for, all
/// of it as it lands.
const LANDS_FROM: f64 = 0.5;
/// The steepest slope the body takes along and across it, radians.
const PITCH_MOST: f64 = 0.5;

/// How long a balk is seen, seconds: the head thrown up and shaken twice.
pub const BALK_S: f64 = 1.0;
/// How far a balk throws the neck and the head up, and turns the head from
/// side to side, radians.
const BALK_NECK: f64 = 0.45;
const BALK_HEAD: f64 = 0.5;
const BALK_SHAKE: f64 = 0.3;
/// Stepping backward, the neck and the head come up this far, radians.
const BACK_NECK: f64 = 0.3;
const BACK_HEAD: f64 = 0.2;
/// The neck bends into a turn before the body follows: as far as the body
/// turns in this long, seconds, up to this far, radians. The head looks
/// into the turn this much further than the neck.
const NECK_LEAD_S: f64 = 0.08;
const NECK_BEND_MOST: f64 = 0.25;
const HEAD_LEADS: f64 = 1.5;
const ROLL_MOST: f64 = 0.6;
/// The mane's pieces along the crest and the tail's down its length.
const MANE_PIECES: usize = 5;
const TAIL_PIECES: usize = 3;
/// How far the mane leans back in the air of the fastest gait, radians.
const MANE_STREAM: f64 = 0.9;
/// How far the stride's ripple bends a piece of hair, radians at the
/// fastest gait, and how far behind the piece before it each bends,
/// strides.
const HAIR_RIPPLE: f64 = 0.28;
const HAIR_LAG: f64 = 0.12;
/// On the wing the hair keeps no stride: it trails behind, a ripple this
/// far, radians, running down it this many times a second, and the tail
/// sways this far side to side, radians, slow.
const AIR_RIPPLE: f64 = 0.12;
const AIR_RIPPLE_HZ: f64 = 0.7;
const AIR_SWAY: f64 = 0.05;
/// A jump. Gathering, the body sinks this far at size 1 and lifts its
/// nose this far, radians, rocked back onto its haunches. Standing, the
/// fore feet leave the ground from this far through it; going, they keep
/// the stride until it springs; taking off, they leave from its start.
const GATHER_SINK_M: f64 = 0.12;
const GATHER_PITCH: f64 = 0.12;
const GATHER_FORE: f64 = 0.5;
/// In the air the body pitches this far, radians: nose up as it rises,
/// down as it falls, as fast as it rises against how fast it left.
const LEAP_PITCH: f64 = 0.35;
/// Leaving the ground, how long the body and its feet take to come away
/// from where they left it into their pose in the air, seconds; landing,
/// how long its feet take to come down from their pose in the air onto
/// the ground.
const LEAVE_S: f64 = 0.12;
/// How high a tucked foot is lifted, metres at size 1: a leg tucks no
/// higher than under the body, whatever this asks.
const TUCK_LIFT_M: f64 = 0.7;
/// Coming down from a jump, how long before it lands its legs have reached
/// down for the ground, seconds, by how fast it falls to it.
const REACH_S: f64 = 0.2;
/// Landing: how long it lasts, seconds, coming level from the pitch it
/// flew at, and how long after it lands the body has given its deepest
/// under the speed it came down at, seconds.
const LAND_S: f64 = 0.35;
const LAND_GIVE_S: f64 = 0.1;
/// The least time in the air a landing is heard after, seconds: a jump's,
/// a fall's off a ledge or a flight's, never a body lifted a moment off a
/// bump.
const LANDED_AFTER_S: f64 = 0.15;
/// How a rider goes with a jump: leaning forward this far, radians, and
/// keeping this share of the body's pitch off their own back.
const RIDER_LEAN: f64 = 0.4;
const RIDER_COUNTER: f64 = 0.8;
/// On the wing: the steepest it pitches with its climb, and banks into a
/// turn, radians; how stiffly its climb is believed, per second.
const CLIMB_PITCH_MOST: f64 = 0.45;
const BANK_MOST: f64 = 0.6;
const CLIMB_PER_S: f64 = 4.0;
/// What a turn's bank is reckoned by, metres a second squared: an earth's
/// gravity, whatever the world's.
const BANK_G: f64 = 9.81;
/// On the wing, its wings beat harder the slower it flies under the
/// faster of these, metres a second; and the nearer the ground under the
/// higher of these, metres, so it labours taking off and landing.
const SLOW_MPS: (f64, f64) = (10.0, 30.0);
const LABOUR_M: (f64, f64) = (0.5, 3.0);
/// The climb that has its wings beat as hard as they can, metres a second.
const CLIMB_HARD_MPS: f64 = 4.0;
/// On the wing, its legs reach down for the ground as it comes down to it:
/// tucked over the higher of these, metres at size 1, down under the lower,
/// and in full once it comes down this fast, metres a second.
const REACH_M: (f64, f64) = (0.5, 3.0);
const REACH_DESCENT_MPS: f64 = 0.5;
/// Taking off, a body gathering this far over the ground under it, metres
/// at size 1, has sprung, whatever its clock says: a peer's spring is drawn
/// as its body is seen to leave.
const SPRUNG_M: f64 = 0.1;

/// How the hair of a mane and a tail moves in a frame: how far it streams
/// back and the tail lifts, 0 to 1, how far the tail swishes side to side,
/// radians, and the ripples running down it, the stride's and the air's,
/// each how far it bends a piece, radians, and where it is, in turns.
#[derive(Clone, Copy, Debug)]
struct Hair {
    stream: f64,
    swish: f64,
    ripples: [(f64, f64); 2],
}

impl Hair {
    /// How far the ripples bend the piece of hair `piece` pieces from its
    /// root, radians: each after the one before it.
    fn ripple(&self, piece: f64) -> f64 {
        self.ripples
            .iter()
            .map(|&(wave, phase)| wave * libm::sin(TAU * (phase - piece * HAIR_LAG)))
            .sum()
    }
}

/// One animal, as drawn.
#[derive(Clone, Debug)]
pub struct Animal {
    pub kind: &'static Kind,
    pub look: Look,
    /// Where it is, world space: the ground under its middle.
    position: DVec3,
    /// Which way it faces, world space, along the ground.
    heading: DVec3,
    /// Metres a second along its heading, negative backward, and radians a
    /// second it turns, left positive.
    speed_mps: f64,
    turn_rps: f64,
    /// How far its head is down in the grass, 0 to 1.
    graze: f64,
    /// Seconds since it balked at what it was asked into; none when it has
    /// not.
    balk_s: Option<f64>,
    /// Where it is in a jump, as what moves it says; none on the ground.
    leap: Option<Leap>,
    /// How it goes in the air, as its body saw it.
    air: Air,
    /// Its wings, for one born with them, and whether they fly it now.
    wings: Option<Wings>,
    winging: bool,
    /// Seconds since its wings were asked for, for one born with them: what
    /// its take-off is drawn by. None while they are not.
    asked_s: Option<f64>,
    /// How fast it climbs, metres a second, eased: what pitches it on the
    /// wing and has its wings beat.
    climb_mps: f64,
    /// Where a downstroke struck this step, world: at its withers.
    beat: Option<DVec3>,
    /// Its wings held out to glide this step, for one born with them.
    glide: Option<Glide>,
    /// Where it sprang this step, world: at its withers, as the ground let
    /// it go from a gathering.
    sprang: Option<DVec3>,
    /// How it came back down this step, as its feet met the ground after a
    /// jump, a fall or a flight.
    landed: Option<Landing>,
    /// Where its withers were this step, world: what it is heard from.
    withers: DVec3,
    /// How far along its strides it is: the fraction is where in a stride.
    stride: f64,
    /// How long it has travelled at a pace, seconds: what its beats are
    /// heard after.
    travelled_s: f64,
    /// Seconds it has been drawn: what a still tail and a grazing head sway
    /// by.
    clock_s: f64,
    /// How its body rides the ground, eased; none until it is first set.
    carriage: Option<Carriage>,
    feet: [Foot; LEGS],
}

/// Where an animal is now and what it does: what moves it, a herd's path or
/// the body a rider rides.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Going {
    /// On the ground, world space.
    pub position: DVec3,
    /// Along the ground, unit.
    pub heading: DVec3,
    /// Metres a second along its heading: negative, it steps backward, its
    /// legs in its walk's order reversed and its head up.
    pub speed_mps: f64,
    pub turn_rps: f64,
    pub graze: f64,
    /// Seconds since it balked: what it was ridden into it refused, and it
    /// throws its head up and shakes it for [`BALK_S`].
    pub balk_s: Option<f64>,
    /// Where it is in a jump; none on the ground.
    pub leap: Option<Leap>,
    /// Its wings asked for, to fly it. Asked standing, it takes off: it
    /// gathers and springs off its legs, as its kind's [`Spring`] times it
    /// from the step they were first asked, and opens them at the top.
    /// Asked in the air, they open at once. Open, its legs are tucked, it
    /// pitches with its climb and banks into its turn. One born without
    /// wings ignores it.
    pub wings: bool,
}

/// Where an animal is in a jump, as what moves it says.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Leap {
    /// Gathering itself, 0 to 1: it sinks onto its haunches, lifts its nose
    /// and its fore feet leave the ground.
    Gathering(f64),
    /// In the air, at the place it is given, over the ground: it tucks its
    /// legs, its nose up as it rises and down as it falls, fore feet
    /// reaching for the ground as it comes back to it.
    Flying,
}

/// How an animal goes in the air, as its body saw it: what it reads to
/// pose a jump whoever moves it, its rider or a peer's.
#[derive(Clone, Copy, Debug, Default)]
struct Air {
    /// How high its middle is over the ground under it and how high up,
    /// metres, last step.
    lift_m: f64,
    high_m: f64,
    /// How fast it rises off the ground under it, metres a second: coming
    /// down, how fast it closes on it.
    rise_mps: f64,
    /// How fast it rises up, metres a second, and the fastest it rose this
    /// jump: what it left the ground at. Its arc, whatever the ground.
    climb_mps: f64,
    top_mps: f64,
    flying: bool,
    /// Seconds in the air this time, and whether its wings flew it: a
    /// landing from the wing is set down, never thrown.
    aloft_s: f64,
    winged: bool,
    /// Seconds since it landed, while it lands, and how fast it sank onto
    /// the ground, metres a second.
    landed_s: Option<f64>,
    sank_mps: f64,
    /// The pitch it last flew at, radians: what it lands from.
    pitch: f64,
}

impl Air {
    /// Takes how high it is now over the ground and up, whether it flies
    /// and whether on its wings, `dt` after last. Says whether it came down
    /// this step from a jump or a fall long enough to be heard, or from a
    /// flight on its wings: how fast it sank onto the ground, and whether
    /// it flew.
    fn track(
        &mut self,
        (lift_m, high_m): (f64, f64),
        flying: bool,
        winging: bool,
        dt: f64,
    ) -> Option<(f64, bool)> {
        if flying && self.flying && dt > 0.0 {
            self.rise_mps = (lift_m - self.lift_m) / dt;
            self.climb_mps = (high_m - self.high_m) / dt;
            self.top_mps = self.top_mps.max(self.climb_mps);
        }
        let landed = self.flying && !flying;
        if landed {
            self.sank_mps = self.rise_mps.min(0.0).abs();
        }
        let heard =
            (landed && self.aloft_s >= LANDED_AFTER_S).then_some((self.sank_mps, self.winged));
        self.landed_s = match (self.flying, flying) {
            (true, false) => Some(0.0),
            (_, false) => self.landed_s.map(|s| s + dt).filter(|&s| s < LAND_S),
            (_, true) => None,
        };
        if flying {
            self.aloft_s += dt;
            self.winged |= winging;
        } else {
            (self.rise_mps, self.climb_mps, self.top_mps) = (0.0, 0.0, 0.0);
            (self.aloft_s, self.winged) = (0.0, false);
        }
        self.flying = flying;
        (self.lift_m, self.high_m) = (lift_m, high_m);
        heard
    }

    /// How fast it rises against how fast it left the ground: 1 leaving
    /// it, 0 at the top of its arc, -1 coming back down.
    fn rising(&self) -> f64 {
        match self.top_mps > 0.0 {
            true => (self.climb_mps / self.top_mps).clamp(-1.0, 1.0),
            false => 1.0,
        }
    }

    /// How far it is on its feet, 0 to 1: off them in the air, coming away
    /// as it leaves the ground and back as it lands, as its feet do.
    fn footed(&self) -> f64 {
        match (self.flying, self.landed_s) {
            (true, _) => 1.0 - smoothstep(0.0, LEAVE_S, self.aloft_s),
            (false, Some(s)) => smoothstep(0.0, LEAVE_S, s),
            (false, None) => 1.0,
        }
    }

    /// How far through landing it is, 0 to 1; none unless it lands.
    fn landing(&self) -> Option<f64> {
        self.landed_s.map(|s| s / LAND_S)
    }

    /// How far the body gives under its landing now, metres: on down as
    /// fast as it sank onto the ground, slowing to its deepest
    /// [`LAND_GIVE_S`] in, and back up by the landing's end.
    fn give_m(&self) -> f64 {
        let Some(s) = self.landed_s else {
            return 0.0;
        };
        let t = s / LAND_GIVE_S;
        let back = 1.0 - smoothstep(0.5, 1.0, s / LAND_S);
        self.sank_mps * LAND_GIVE_S * t * libm::exp(-t) * back
    }

    /// A leg in the air: how high its foot is tucked, metres at size 1,
    /// how far ahead of rest it reaches, and how far it folds, 0 to 1. The
    /// fore legs leave tucked and reach forward and down for the ground as
    /// it falls; the hind legs leave stretched back from the push, then
    /// tuck under. Both have reached down before it comes down onto the
    /// ground. On the wing, `wing` is how far its legs reach for the
    /// ground, 0 to 1: both pairs tucked as at a jump's top, reaching
    /// down as it comes down to land.
    fn leg(&self, hind: bool, wing: Option<f64>) -> (f64, f64, f64) {
        if let Some(reach) = wing {
            let (along, held) = match hind {
                false => (0.3, 1.0),
                true => (-0.1, 0.8),
            };
            let tuck = 1.0 - reach;
            return (TUCK_LIFT_M * held * tuck, along * reach, tuck);
        }
        let rising = self.rising();
        let near_s = match self.rise_mps < 0.0 {
            true => self.lift_m.max(0.0) / -self.rise_mps,
            false => f64::INFINITY,
        };
        let held = smoothstep(0.0, REACH_S, near_s);
        match hind {
            false => {
                let tuck = smoothstep(-0.7, -0.2, rising) * held;
                (TUCK_LIFT_M * tuck, 0.3 * (1.0 - tuck), tuck)
            }
            true => {
                let push = smoothstep(0.4, 0.9, rising);
                let tuck = (1.0 - push) * held;
                (TUCK_LIFT_M * 0.8 * tuck, -0.45 * push, tuck)
            }
        }
    }
}

/// One box of an animal's side under its saddle, metres: from `top_m`
/// down to `bottom_m` below the seat, reaching `out_m` to each side.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Flank {
    pub top_m: f64,
    pub bottom_m: f64,
    pub out_m: f64,
}

/// Wings held out to glide, this step: what a client sustains the wind of.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Glide {
    /// Where, world: at its withers, as a beat.
    pub at: DVec3,
    /// How far held out, 0 to 1 (`Wings::held_out`).
    pub held_out: f64,
    /// How fast it flies, metres a second.
    pub speed_mps: f64,
}

/// How an animal came back down onto the ground: once a jump, a fall off a
/// ledge or a flight on its wings, as its feet met it.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Landing {
    /// Where, world: at its withers, as a beat.
    pub at: DVec3,
    /// How fast it sank onto the ground, metres a second, as its body saw
    /// the ground under it come up: the ground rising ahead of it adds to
    /// its own descent.
    pub sink_mps: f64,
    /// How fast it went along the ground, metres a second.
    pub speed_mps: f64,
    /// Whether its wings flew it: set down from a flight, never thrown from
    /// a jump.
    pub winged: bool,
}

/// A beat of its gait struck: where the body was and how hard. Only an
/// animal that travels at a pace strikes one, by its gait's beats, never a
/// foot set down as it turns, shuffles or settles. Where it falls is asked
/// of the ground only for a beat heard: [`Beat::fall`].
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Beat {
    /// The body's middle, world space.
    pub at: DVec3,
    /// How hard, 0 to 1: its gait's force.
    pub force: f64,
}

impl Beat {
    /// Where the beat falls: on the ground under the body's middle.
    pub fn fall(self, ground: &impl Ground) -> Footfall {
        let under = ground.under(self.at);
        Footfall {
            at: under.point,
            surface: under.surface,
            force: self.force,
        }
    }
}

/// A beat of its gait heard: where, on what, how hard.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Footfall {
    /// On the ground under its middle, world space.
    pub at: DVec3,
    /// What the ground there is, as the ground said it.
    pub surface: u8,
    /// How hard, 0 to 1: its gait's force.
    pub force: f64,
}

/// How the body rides the ground this frame.
#[derive(Clone, Copy, Debug)]
struct Carriage {
    /// Where the body stands, world: over the ground its feet stand on,
    /// eased along up.
    at: DVec3,
    /// The ground's slope along the heading: where a body on it goes next,
    /// which is followed at once. Only what the ground does besides is
    /// eased.
    slope: f64,
    height: Follow,
    pitch: Follow,
    roll: Follow,
    /// The neck's own pitch, world: it comes after the body's.
    neck: Follow,
    /// The pitch a jump gives, over the ground's.
    leap: Follow,
    /// How far the neck bends into a turn, radians, left positive, and how
    /// far up stepping backward holds it, 0 to 1.
    bend: Follow,
    raised: Follow,
    /// In the air, how far over where its arc has it the body was as it
    /// left the ground, metres: under, crouched to spring, it rises out.
    left_m: Option<f64>,
    up: DVec3,
    forward: DVec3,
}

/// A foot: where it is and how it got there.
#[derive(Clone, Copy, Debug, Default)]
struct Foot {
    /// Where it was put down, world: it stays there until it lifts.
    planted: Option<DVec3>,
    /// Where it was lifted from, less where the gait had it then: what its
    /// swing closes, so a turn never slides it.
    slip: DVec3,
    /// How far it is peeled off the ground at the end of a stride past its
    /// leg's reach, metres: down, what it peels now; swinging, what it
    /// peeled as it left, closed by its swing as its slip is.
    peel_m: f64,
    /// This frame: the ground it is on or over, world; how high off it; how
    /// far ahead of rest, metres at size 1; how far through its swing.
    at: DVec3,
    lift_m: f64,
    along_m: f64,
    swing: f64,
    /// Where it comes away from into its pose aloft as the body leaves the
    /// ground: a hind foot down, where it is now, on the ground; any other,
    /// where it is under the body, which it leaves with; in the air, where
    /// it was as it left. `at`, `lift_m` and `along_m`.
    left: Option<(Spot, f64, f64)>,
    /// In the air and landing, where it is posed in the air, which it
    /// comes down from onto the ground.
    came: Option<Came>,
    /// Stepping to where its gait has it down.
    step: Option<Step>,
}

/// A foot stepping to where its gait has it down, as a swing does.
#[derive(Clone, Copy, Debug)]
struct Step {
    /// How long since it lifted, seconds.
    s: f64,
    /// Where from, which it leaves going with the body, and how high it was
    /// there, metres.
    from: Spot,
    from_m: f64,
    /// How high it lifts on the way, metres: as high as its gait lifts a
    /// foot, less for a step shorter than [`SLIP_M`].
    arc_m: f64,
}

impl Step {
    /// A step from `at`, `lift_m` over the ground, to `to`, by a gait that
    /// lifts a foot `gait_lift_m`: it lifts in proportion to how far it
    /// goes, all of it from [`SLIP_M`] on, at `size`.
    fn from(
        at: Spot,
        lift_m: f64,
        to: DVec3,
        gait_lift_m: f64,
        size: f64,
        frame: DAffine3,
    ) -> Step {
        let far_m = (at.at(frame) - to).length() + lift_m;
        Step {
            s: 0.0,
            from: at,
            from_m: lift_m,
            arc_m: gait_lift_m * (far_m / (SLIP_M * size)).min(1.0),
        }
    }
}

/// A foot's pose in the air, which it comes down from as the body lands.
#[derive(Clone, Copy, Debug)]
struct Came {
    at: Spot,
    lift_m: f64,
    along_m: f64,
    /// How long it takes to come down, seconds: none until it lands.
    over_s: Option<f64>,
}

/// Where a foot was, which it moves on from: one down stays on the ground
/// it stood on, as a hoof driving the body off does; one in the air goes
/// with the body, in its frame.
#[derive(Clone, Copy, Debug)]
enum Spot {
    Ground(DVec3),
    Body(DVec3),
}

impl Spot {
    /// A foot at `at`, world, on the ground or not, under a body drawn by
    /// `frame`.
    fn of(at: DVec3, down: bool, frame: DAffine3) -> Spot {
        match down {
            true => Spot::Ground(at),
            false => Spot::Body(frame.inverse().transform_point3(at)),
        }
    }

    /// Where it is now, world, under a body drawn by `frame`.
    fn at(self, frame: DAffine3) -> DVec3 {
        match self {
            Spot::Ground(at) => at,
            Spot::Body(at) => frame.transform_point3(at),
        }
    }
}

/// A leg as drawn this frame, world space.
#[derive(Clone, Copy, Debug)]
pub struct LegPose {
    /// Where it meets the body.
    pub root: DVec3,
    /// The knee of a fore leg, the hock of a hind one.
    pub joint: DVec3,
    pub fetlock: DVec3,
    /// What the fetlock reached for.
    pub target: DVec3,
    /// The way the joint was asked to bend.
    pub pole: DVec3,
}

/// The body as drawn this frame, and what drew it.
#[derive(Clone, Copy)]
struct Drawn {
    /// Rig space to world.
    body: DAffine3,
    body_forward: DVec3,
    /// Out of the ground, and to the right along it.
    up: DVec3,
    right: DVec3,
    gait: Gait,
    pace: f64,
    moving: f64,
    /// How much its stride rocks and nods it: none in the air.
    striding: f64,
}

impl Going {
    /// Standing still at a place, facing a way.
    pub fn standing(position: DVec3, heading: DVec3) -> Going {
        Going {
            position,
            heading,
            speed_mps: 0.0,
            turn_rps: 0.0,
            graze: 0.0,
            balk_s: None,
            leap: None,
            wings: false,
        }
    }
}

impl Animal {
    pub fn new(kind: &'static Kind, look: Look) -> Animal {
        let wings = match look.traits.wings {
            true => kind.wings.map(|winged| Wings::new(winged.wing)),
            false => None,
        };
        Animal {
            kind,
            look,
            position: DVec3::ZERO,
            heading: DVec3::X,
            speed_mps: 0.0,
            turn_rps: 0.0,
            graze: 0.0,
            balk_s: None,
            leap: None,
            air: Air::default(),
            wings,
            winging: false,
            asked_s: None,
            climb_mps: 0.0,
            beat: None,
            glide: None,
            sprang: None,
            landed: None,
            withers: DVec3::ZERO,
            stride: 0.0,
            travelled_s: 0.0,
            clock_s: 0.0,
            carriage: None,
            feet: [Foot::default(); LEGS],
        }
    }

    /// Where it is, world: the ground under its middle.
    pub fn position(&self) -> DVec3 {
        self.position
    }

    pub fn heading(&self) -> DVec3 {
        self.heading
    }

    /// Its wings, for one born with them.
    pub fn wings(&self) -> Option<&Wings> {
        self.wings.as_ref()
    }

    /// Where a downstroke of its wings struck this step, world.
    pub fn wingbeat(&self) -> Option<DVec3> {
        self.beat
    }

    /// What it was born with.
    pub fn traits(&self) -> &Traits {
        &self.look.traits
    }

    /// Where its withers were this step, world: where a voice of its body
    /// sounds.
    pub fn withers(&self) -> DVec3 {
        self.withers
    }

    /// Where it sprang into a jump this step, world: once a jump, as it
    /// leaves the ground from its gathering. Never on a balk, which never
    /// leaves it, nor on the wing or a fall, which gather nothing.
    pub fn sprang(&self) -> Option<DVec3> {
        self.sprang
    }

    /// How it came back down this step: once a jump, a fall off a ledge or
    /// a flight on its wings, as its feet meet the ground. Never off a bump
    /// it left a moment.
    pub fn landed(&self) -> Option<Landing> {
        self.landed
    }

    /// How hard it springs into a jump or lands from one, 0 to 1: its
    /// gait's force at its pace, by the square root, so a jump from a halt
    /// is strong too and one from the gallop the strongest.
    pub fn leap_force(&self) -> f64 {
        libm::sqrt(self.kind.gaits.at(self.pace()).force)
    }

    /// Its wings held out to glide this step: none while folded or beating.
    pub fn glide(&self) -> Option<Glide> {
        self.glide
    }

    /// How far it banks into a turn on its wings, radians, right side up
    /// positive; level on the ground.
    pub fn bank(&self) -> f64 {
        match (&self.carriage, self.winging) {
            (Some(carriage), true) => carriage.roll.value(),
            _ => 0.0,
        }
    }

    /// Whether it travels on the ground at a pace, and so is heard.
    pub fn travels(&self) -> bool {
        self.travelled_s >= HEARD_AFTER_S
    }

    /// Whether its body has been set on the ground: it has stepped.
    pub fn is_set(&self) -> bool {
        self.carriage.is_some()
    }

    /// Its pace as an animal of size 1's: the speed it moves at in the same
    /// gait, since a smaller one changes gait at a lower speed (dynamic
    /// similarity, by the square root of its size). A turn in place counts,
    /// so the legs step it round.
    fn pace(&self) -> f64 {
        let size = self.look.traits.size;
        let turning = self.turn_rps.abs() * TURN_STEP_M * size;
        self.speed_mps.abs().max(turning) / size.sqrt()
    }

    /// Whether it is off the ground this step, leaping or on its wings,
    /// once it has been set.
    fn flies(&self) -> bool {
        (self.leap == Some(Leap::Flying) || self.winging) && self.carriage.is_some()
    }

    /// Which way its strides run: backward while it steps backward, so its
    /// gait is played in reverse.
    fn stride_way(&self) -> f64 {
        match self.speed_mps < 0.0 {
            true => -1.0,
            false => 1.0,
        }
    }

    /// Whether going so in `dt` takes it further than a step past what its
    /// speed carries it: [`Animal::step`] sets it anew there, feet and all.
    /// On its wings it is never set anew.
    pub fn leaps(&self, dt: f64, going: &Going) -> bool {
        let moved_m = (going.position - self.position).length();
        let on_wings = self.winging && going.wings && self.carriage.is_some();
        let carried_m = CARRIED * dt.max(0.0) * going.speed_mps.abs().max(self.speed_mps.abs());
        !on_wings && moved_m > LEAP_M + carried_m
    }

    /// Takes where it is now and `dt` seconds of its stride, then plants
    /// its feet and eases its body after the ground. A move further than a
    /// step past what its speed carries it in `dt` sets it anew there. On
    /// its wings nothing of it stands on anything, and it is where it is
    /// given however far that is: a frame of flight, or of a dive to land
    /// from high up, covers metres. Says the beat of its gait heard, if one
    /// was.
    pub fn step(&mut self, dt: f64, going: Going, ground: &impl Ground) -> Option<Beat> {
        let dt = dt.max(0.0);
        if self.leaps(dt, &going) {
            self.carriage = None;
            self.feet = [Foot::default(); LEGS];
        } else if dt > 0.0 {
            let climb = (going.position.length() - self.position.length()) / dt;
            self.climb_mps += (climb - self.climb_mps) * (1.0 - libm::exp(-CLIMB_PER_S * dt));
        }
        self.asked_s = self.asked(dt, going.wings);
        let taking_off = self.taking_off();
        self.winging = going.wings && self.wings.is_some() && taking_off.is_none();
        if !self.winging && self.carriage.is_none() {
            self.climb_mps = 0.0;
        }
        self.position = going.position;
        self.heading = going.heading;
        self.speed_mps = going.speed_mps;
        self.turn_rps = going.turn_rps;
        self.graze = going.graze;
        self.balk_s = going.balk_s;
        let leap = taking_off.or(going.leap);
        let springs = matches!(self.leap, Some(Leap::Gathering(_))) && leap == Some(Leap::Flying);
        self.leap = leap;
        let pace = self.pace();
        let gait = self.kind.gaits.at(pace);
        // Coming down out of the air, it lands on a fore foot: its stride
        // goes on from where the nearer one lands.
        if self.air.flying && !self.flies() {
            self.stride += (0..LEGS)
                .filter(|&leg| !Rig::hind(leg))
                .map(|leg| (gait.lands[leg] - self.stride).rem_euclid(1.0))
                .fold(f64::INFINITY, f64::min);
        }
        let hz = gait.hz_at(pace) / self.look.traits.size.sqrt();
        let was = self.stride;
        self.stride += dt * hz * smoothstep(STILL_MPS, 0.25, pace) * self.stride_way();
        self.clock_s += dt;
        self.settle(dt, ground, springs);
        self.struck(dt, was, &gait)
    }

    /// Seconds since its wings were asked for, `dt` on, while `wings` asks
    /// for them. Asked standing, it takes off from the step they were;
    /// asked in the air, or before it was first set, they are open at once.
    /// Gathering, a body that has left the ground has sprung.
    fn asked(&self, dt: f64, wings: bool) -> Option<f64> {
        let spring = self.spring().filter(|_| wings)?;
        let standing = self.carriage.is_some() && !self.air.flying;
        let s = match self.asked_s {
            Some(s) => s + dt,
            None if standing => dt,
            None => spring.top_s(),
        };
        let sprung = self.air.lift_m > SPRUNG_M * self.look.traits.size;
        Some(match s < spring.gather_s && sprung {
            true => spring.gather_s,
            false => s,
        })
    }

    /// How it leaves the ground on its wings, for one born with them.
    fn spring(&self) -> Option<Spring> {
        self.wings.as_ref()?;
        self.kind.wings.map(|winged| winged.flight.spring)
    }

    /// Where it is in taking off, by the time since its wings were asked
    /// for: gathering, then in the air off its legs. None before they were
    /// asked for and from the top of its spring, where they open.
    fn taking_off(&self) -> Option<Leap> {
        let (s, spring) = (self.asked_s?, self.spring()?);
        match s {
            _ if s < spring.gather_s => Some(Leap::Gathering(s / spring.gather_s)),
            _ if s < spring.top_s() => Some(Leap::Flying),
            _ => None,
        }
    }

    /// The beat of its gait it struck since `was` strides along, if it
    /// travels on the ground: forward, at a pace, for a while. Carried at a
    /// pace through the air it travels on, unheard, so it comes down
    /// travelling.
    fn struck(&mut self, dt: f64, was: f64, gait: &Gait) -> Option<Beat> {
        let aloft = self.winging || matches!(self.leap, Some(Leap::Flying));
        let travels = self.speed_mps / self.look.traits.size.sqrt() >= TRAVEL_MPS;
        self.travelled_s = match travels && self.carriage.is_some() {
            true => self.travelled_s + dt,
            false => 0.0,
        };
        let struck = gait
            .beats
            .iter()
            .any(|&beat| libm::floor(self.stride - beat) > libm::floor(was - beat));
        if !struck || aloft || self.travelled_s < HEARD_AFTER_S {
            return None;
        }
        Some(Beat {
            at: self.position,
            force: gait.force,
        })
    }

    /// Plants the feet the gait puts down and lifts the others, and eases
    /// the body after the ground they stand on. The first time, or with no
    /// time passed, the body is set where the ground puts it. In the air it
    /// is where it is given, its legs tucked as far through the jump as it
    /// is.
    fn settle(&mut self, dt: f64, ground: &impl Ground, springs: bool) {
        let rig = &self.kind.rig;
        let (size, build) = (self.look.traits.size, self.look.traits.build);
        let foot = ground.under(self.position);
        let up = foot.up;
        let flying = self.flies();
        let lift_m = (self.position - foot.point).dot(up);
        // How high up its arc was last step: where it leaves the ground from.
        let was_m = self.air.high_m;
        let landed = self
            .air
            .track((lift_m, self.position.dot(up)), flying, self.winging, dt);
        // Its wings beat hard to climb, to fly slow and near the ground, and
        // are held out to glide when it flies fast and level.
        let winging = self.winging;
        let speed_mps = self.speed_mps;
        let (climb_mps, at) = (
            self.climb_mps,
            self.position + up * rig.pivot.y * 1.35 * size,
        );
        self.beat = self.wings.as_mut().and_then(|wings| {
            let slow = 1.0 - smoothstep(SLOW_MPS.0, SLOW_MPS.1, speed_mps);
            let climbing = climb_mps / CLIMB_HARD_MPS;
            let labour = 1.0 - smoothstep(LABOUR_M.0, LABOUR_M.1, lift_m);
            let want = (0.7 * slow).max(climbing).max(labour);
            wings.update(dt, winging, want).then_some(at)
        });
        self.withers = at;
        self.sprang = springs.then_some(at);
        self.landed = landed.map(|(sink_mps, winged)| Landing {
            at,
            sink_mps,
            speed_mps: speed_mps.abs(),
            winged,
        });
        self.glide = self.wings.as_ref().and_then(|wings| {
            let held_out = wings.held_out();
            (held_out > 0.0).then_some(Glide {
                at,
                held_out,
                speed_mps,
            })
        });
        let base = match flying {
            true => self.position,
            false => foot.point,
        };
        let forward = along(self.heading, up);
        let right = forward.cross(up);
        let frame =
            DAffine3::from_mat3_translation(DMat3::from_cols(right, up, -forward) * size, base);
        let wide = |p: DVec3| DVec3::new(p.x * build, p.y, p.z);

        // The ground under each foot at rest: what the body stands over,
        // whatever the stride. In the air it stands over none: it flies at
        // its arc's pitch, level across.
        let rest: [f64; LEGS] = core::array::from_fn(|leg| {
            let at = frame.transform_point3(wide(rig.leg(leg).foot));
            match flying {
                true => 0.0,
                false => (ground.under(at).point - base).dot(up),
            }
        });
        let [lf, rf, lh, rh] = rest;
        let (span_z, span_x) = rig.span();
        let slope =
            libm::atan((lf + rf - lh - rh) / 2.0 / (span_z * size)).clamp(-PITCH_MOST, PITCH_MOST);
        let across = libm::atan((rf + rh - lf - lh) / 2.0 / (span_x * size * build))
            .clamp(-ROLL_MOST, ROLL_MOST);
        let pitch = slope;
        let bank = libm::atan(self.speed_mps * self.turn_rps / BANK_G).clamp(-BANK_MOST, BANK_MOST);
        let roll = match winging {
            true => bank,
            false => across * rig.roll_follow,
        };
        let mut height = match flying {
            true => 0.0,
            false => (lf + rf + lh + rh) / 4.0,
        };
        // A jump pitches the body over that, and sinks it as it gathers and
        // as it lands. Landing, it comes from the pitch it flew at to the
        // ground's, never the one on the other.
        let gathered = match self.leap {
            Some(Leap::Gathering(gathered)) => gathered.clamp(0.0, 1.0),
            _ => 0.0,
        };
        let landing = self.air.landing();
        let climb = libm::atan2(self.climb_mps, self.speed_mps.abs().max(4.0))
            .clamp(-CLIMB_PITCH_MOST, CLIMB_PITCH_MOST);
        let leap_pitch = match (flying, landing) {
            (true, _) if winging => climb,
            (true, _) => LEAP_PITCH * self.air.rising(),
            (false, Some(t)) => (self.air.pitch - pitch) * (1.0 - smoothstep(0.0, 1.0, t)),
            (false, None) => GATHER_PITCH * smoothstep(0.0, 1.0, gathered),
        };
        if flying {
            self.air.pitch = leap_pitch;
        }
        height -=
            size * (GATHER_SINK_M * smoothstep(0.0, 1.0, gathered) + self.air.give_m() / size);

        // The feet: down, each stays where it was put, unless the ground
        // under it is gone; up, it swings to where the gait lands it,
        // closing what a turn left.
        let pace = self.pace();
        let gait = self.kind.gaits.at(pace);
        let reach = winging.then(|| {
            let near = 1.0 - smoothstep(REACH_M.0 * size, REACH_M.1 * size, lift_m);
            near * smoothstep(0.0, REACH_DESCENT_MPS, -climb_mps)
        });
        // Halted, it stands square: once every foot is down, the one
        // furthest from where it stands steps there, a foot at a time.
        let halted = self.carriage.is_some() && pace < STILL_MPS && self.leap.is_none();
        let down = self
            .feet
            .iter()
            .all(|foot| foot.planted.is_some() && foot.step.is_none() && foot.came.is_none());
        let squaring = (halted && down)
            .then(|| {
                (0..LEGS)
                    .filter_map(|leg| {
                        let stands = frame.transform_point3(wide(rig.leg(leg).foot));
                        let far_m = (self.feet[leg].planted? - ground.under(stands).point).length();
                        (far_m > SQUARE_M * size).then_some((leg, far_m))
                    })
                    .max_by(|a, b| a.1.total_cmp(&b.1))
                    .map(|(leg, _)| leg)
            })
            .flatten();
        for leg in 0..LEGS {
            let hind = Rig::hind(leg);
            if flying {
                let (lift_m, along_m, tuck) = self.air.leg(hind, reach);
                let foot = &mut self.feet[leg];
                let aloft = frame.transform_point3(wide(rig.leg(leg).foot) - DVec3::Z * along_m);
                // A foot comes away from where it left the ground: a hind
                // one down there drives the body off before it tucks; a fore
                // one leaves with the body, folding. One further from it
                // than a leap was never on that ground.
                let near = (foot.at - aloft).length() <= LEAP_M;
                let left = (foot.left.filter(|_| near)).unwrap_or((
                    Spot::of(aloft, false, frame),
                    lift_m * size,
                    along_m,
                ));
                let away = smoothstep(0.0, LEAVE_S, self.air.aloft_s);
                let at = left.0.at(frame).lerp(aloft, away);
                // Reaching down, it meets the ground under it, a slope's
                // ahead, and goes no lower; ground higher than its tuck it
                // reaches no higher than that.
                let ground_m = (ground.under(at).point - at).dot(up);
                let lift_m = (left.1 + (lift_m * size - left.1) * away)
                    .max(ground_m.min(TUCK_LIFT_M * size));
                let along_m = left.2 + (along_m - left.2) * away;
                *foot = Foot {
                    planted: None,
                    slip: DVec3::ZERO,
                    peel_m: 0.0,
                    at,
                    lift_m,
                    along_m,
                    swing: 0.5 * tuck,
                    left: Some(left),
                    came: Some(Came {
                        at: Spot::of(at, false, frame),
                        lift_m,
                        along_m,
                        over_s: None,
                    }),
                    step: None,
                };
                continue;
            }
            let hoof = gait.hoof(leg, self.stride, pace);
            let planned = frame.transform_point3(wide(rig.leg(leg).foot) - DVec3::Z * hoof.along_m);
            // Gathered far enough, the fore feet leave the ground first:
            // all of them taking off, and only as still as it stands for a
            // jump, which it may gather at a gallop.
            let still = 1.0 - smoothstep(0.0, TRAVEL_MPS, self.speed_mps.abs() / size.sqrt());
            let leaving = match (hind, self.asked_s) {
                (true, _) => 0.0,
                (false, Some(_)) => smoothstep(0.0, 1.0, gathered),
                (false, None) => smoothstep(GATHER_FORE, 1.0, gathered) * still,
            };
            let foot = &mut self.feet[leg];
            if leaving > 0.0 {
                foot.planted = None;
                foot.slip = DVec3::ZERO;
                foot.peel_m = 0.0;
                foot.at = ground.under(planned).point;
                foot.lift_m = TUCK_LIFT_M * leaving * size;
                foot.along_m = hoof.along_m;
                foot.swing = 0.5 * leaving;
                foot.came = None;
                continue;
            }
            foot.along_m = hoof.along_m;
            // A foot down on ground that is gone, or left further behind than
            // a stride, as a sudden change of pace leaves one, steps to
            // where its gait has it down: it swings there as its gait swings
            // a foot, lifted as high.
            let lift_m = gait.lift_m * size;
            if let Some(planted) = foot.planted {
                let gone = (ground.under(planted).point - planted).length() > GONE_M * size;
                let left = (planted - planned).length() > SLIP_M * size;
                if gone || (hoof.down && left) || squaring == Some(leg) {
                    foot.planted = None;
                    foot.slip = planted - planned;
                    foot.step = hoof.down.then(|| {
                        let from = Spot::of(planted, false, frame);
                        Step::from(from, foot.peel_m, planned, lift_m, size, frame)
                    });
                }
            }
            // Halted, it stands on every foot: one the halt caught in the
            // air, wherever its swing had it, steps down to where it stands.
            // One never set is put there.
            if halted && foot.planted.is_none() && foot.step.is_none() {
                let from = Spot::of(foot.at, false, frame);
                foot.step = Some(Step::from(from, foot.lift_m, planned, lift_m, size, frame));
            }
            let swing_s = (1.0 - gait.duty) * size.sqrt() / gait.hz_at(pace);
            match (hoof.down, foot.step) {
                (true, Some(step)) if step.s < swing_s => {
                    let swing = step.s / swing_s;
                    let ease = smoothstep(0.0, 1.0, swing);
                    foot.at = ground.under(step.from.at(frame).lerp(planned, ease)).point;
                    foot.lift_m = step.arc_m * libm::sin(PI * swing) + step.from_m * (1.0 - ease);
                    foot.swing = swing;
                    foot.step = Some(Step {
                        s: step.s + dt,
                        ..step
                    });
                    continue;
                }
                // Lifted by its gait as it steps, it swings on from where
                // the step has it.
                (false, Some(_)) => {
                    foot.slip = foot.at - planned;
                    foot.step = None;
                }
                _ => foot.step = None,
            }
            if hoof.down {
                let at = match foot.planted {
                    Some(at) => at,
                    None => {
                        let under = ground.under(planned);
                        foot.planted = Some(under.point);
                        foot.slip = DVec3::ZERO;
                        foot.peel_m = 0.0;
                        under.point
                    }
                };
                foot.at = at;
                foot.lift_m = foot.peel_m;
                foot.swing = 0.0;
            } else {
                if let Some(planted) = foot.planted.take() {
                    foot.slip = planted - planned;
                }
                let fade = 1.0 - smoothstep(0.0, 1.0, hoof.swing);
                foot.at = ground.under(planned + foot.slip * fade).point;
                foot.lift_m = hoof.lift_m * size + foot.peel_m * fade;
                foot.swing = hoof.swing;
            }
        }

        // Lowered as far as the longest leg needs to reach its foot: one
        // down where it stands, one swinging where it is lifted to as it
        // comes down, so the body sinks to a foot as it lands, never all at
        // once when it does. A foot down behind the body past what its leg
        // reaches peels off the ground instead, as a hoof leaves at the end
        // of a long stride: the body keeps its height. In the air no foot
        // is reached for.
        let raised = frame
            * DAffine3::from_translation(DVec3::Y * height / size + rig.pivot)
            * DAffine3::from_quat(
                DQuat::from_rotation_x(pitch + leap_pitch) * DQuat::from_rotation_z(roll),
            )
            * DAffine3::from_translation(-rig.pivot);
        let mut lower = 0.0f64;
        // A foot coming down from the air: where it was, and how far down
        // it has come, 0 to 1. It comes onto where its gait has it no
        // faster than the gait swings a foot, within its landing.
        if !flying {
            let swing_mps = gait.swing_mps(pace) * size.sqrt();
            for foot in &mut self.feet {
                if let Some(came) = &mut foot.came {
                    let to = foot.at + up * foot.lift_m;
                    let far_m = (came.at.at(frame) + up * came.lift_m - to).length();
                    (came.over_s).get_or_insert(
                        (far_m / swing_mps.max(f64::EPSILON)).clamp(LEAVE_S, LAND_S),
                    );
                }
            }
        }
        let landed_s = self.air.landed_s;
        let coming = |foot: &Foot| {
            let came = foot.came?;
            let down = smoothstep(0.0, came.over_s?, landed_s?);
            (down < 1.0).then_some((came, down))
        };
        if !flying {
            for (leg, foot) in self.feet.iter_mut().enumerate() {
                let rest = rig.leg(leg);
                let reach = (rest.upper_m + rest.lower_m) * size;
                // A foot down is reached for on the ground, whatever it peels.
                let lifted = match foot.planted {
                    Some(_) => 0.0,
                    None => foot.lift_m,
                };
                let fetlock = foot.at + up * (lifted + (rig.pastern_m + rig.hoof_m) * size);
                let to = fetlock - raised.transform_point3(root(&rest, foot, build));
                let high = -to.dot(up);
                let flat = to.length_squared() - high * high;
                let needed = match flat < reach * reach {
                    true => high - (reach * reach - flat).sqrt(),
                    false => high,
                };
                // Lifting a foot lowers what it needs by as much, so a hoof
                // peeling and the body sinking share it without a seam.
                let needed = match foot.planted {
                    Some(_) => {
                        let behind = smoothstep(0.0, PEEL_BEHIND_M * size, -to.dot(forward));
                        foot.peel_m = behind * (needed - PEEL_M * size).max(0.0);
                        foot.lift_m = foot.peel_m;
                        needed - foot.peel_m
                    }
                    // A foot in the air bears nothing until it comes down: the
                    // body sinks to it only as it lands, never for one left
                    // behind as it runs on.
                    None => needed * smoothstep(LANDS_FROM, 1.0, foot.swing),
                };
                lower = lower.max(needed * coming(foot).map_or(1.0, |(_, down)| down));
            }
            // Landed, each foot comes down from where it was in the air onto
            // where its gait has it, bearing the body as it does. Carried
            // down with the body, it meets the ground on the way and goes no
            // lower, as it does in the air.
            for (leg, foot) in self.feet.iter_mut().enumerate() {
                match coming(foot) {
                    Some((came, down)) => {
                        foot.at = came.at.at(frame).lerp(foot.at, down);
                        let ground_m = (ground.under(foot.at).point - foot.at).dot(up);
                        foot.lift_m =
                            (came.lift_m + (foot.lift_m - came.lift_m) * down).max(ground_m);
                        foot.along_m = came.along_m + (foot.along_m - came.along_m) * down;
                    }
                    None => foot.came = None,
                }
                let drives = foot.planted.is_some() && Rig::hind(leg);
                foot.left = Some((Spot::of(foot.at, drives, frame), foot.lift_m, foot.along_m));
            }
        }
        height -= lower.min(SINK_M * size);

        // The body comes after that with ease. A body on a slope rises with
        // it as it goes, at once: only what the ground does besides is left
        // behind and closed.
        let target = base + up * height;
        let bend = (self.turn_rps * NECK_LEAD_S).clamp(-NECK_BEND_MOST, NECK_BEND_MOST);
        let raised = match self.speed_mps < 0.0 {
            true => 1.0,
            false => 0.0,
        };
        // Ground that moved further than a leap under the body is another
        // ground: the body is set on it, never eased toward it with its legs
        // reaching up. In the air it stands on none.
        let kept = self
            .carriage
            .filter(|c| flying || (c.at - target).dot(up).abs() <= LEAP_M);
        let carriage = match kept {
            Some(mut c) if dt > 0.0 => {
                let ahead = self.speed_mps * dt * libm::tan(c.slope);
                c.height.off = (c.at + up * ahead - target).dot(up);
                c.height.follow(0.0, BODY_SETTLE_PER_S, dt);
                // Lagging low, the legs fold; lagging high, they would hang
                // short of the ground. Down a step it drops as far as they
                // reach at once. In the air it is where its arc has it, once
                // it has come away from where it left the ground; further
                // from it than a leap, it was never on that ground.
                c.height.off = match flying {
                    true => {
                        let left = *c.left_m.get_or_insert_with(|| {
                            let over_m = c.at.dot(up) - was_m;
                            match over_m.abs() <= LEAP_M {
                                true => over_m,
                                false => 0.0,
                            }
                        });
                        left * (1.0 - smoothstep(0.0, LEAVE_S, self.air.aloft_s))
                    }
                    // Landed this step, it is where its arc brought it
                    // down, and gives under it from there.
                    false if c.left_m.take().is_some() => {
                        c.height = Follow::at(0.0);
                        0.0
                    }
                    false => c.height.off.min(HANG_M * size),
                };
                c.at = target + up * c.height.off;
                c.pitch.follow(pitch, BODY_SETTLE_PER_S, dt);
                c.leap.follow(leap_pitch, BODY_SETTLE_PER_S, dt);
                c.roll.follow(roll, BODY_SETTLE_PER_S, dt);
                c.neck.follow(pitch, NECK_SETTLE_PER_S, dt);
                c.bend.follow(bend, NECK_SETTLE_PER_S, dt);
                c.raised.follow(raised, NECK_SETTLE_PER_S, dt);
                Carriage {
                    slope,
                    up,
                    forward,
                    ..c
                }
            }
            Some(c) => Carriage {
                slope,
                up,
                forward,
                ..c
            },
            None => Carriage {
                at: target,
                slope,
                height: Follow::at(0.0),
                pitch: Follow::at(pitch),
                roll: Follow::at(roll),
                neck: Follow::at(pitch),
                leap: Follow::at(leap_pitch),
                bend: Follow::at(bend),
                raised: Follow::at(raised),
                left_m: None,
                up,
                forward,
            },
        };
        self.carriage = Some(carriage);
    }

    /// How its mane and tail move this frame, at `gait`, `moving` from 0
    /// to 1. On its feet the stride stirs them, and through a leap they
    /// stream as it ran, unstirred; on the wing, where a stride would run
    /// as fast as it flies, they trail behind it, calm, eased from one to
    /// the other as its wings grow out and fold away.
    fn hair(&self, gait: &Gait, moving: f64) -> Hair {
        let air = self
            .wings
            .as_ref()
            .map_or(0.0, |wings| smoothstep(0.0, 1.0, wings.grown));
        let ground = 1.0 - air;
        let stream = gait.tail * moving;
        let stirred = self.air.footed();
        let swish = match moving > 0.5 {
            true => 0.15 * stirred * libm::sin(TAU * self.stride),
            false => 0.3 * libm::sin(self.clock_s * 1.1) + 0.12 * libm::sin(self.clock_s * 2.9),
        };
        let sway = AIR_SWAY * (libm::sin(self.clock_s * 0.9) + 0.4 * libm::sin(self.clock_s * 2.3));
        Hair {
            stream: stream * ground + air,
            swish: swish * ground + sway * air,
            ripples: [
                (
                    HAIR_RIPPLE * stream * ground * stirred,
                    self.stride - gait.sink,
                ),
                (AIR_RIPPLE * air, self.clock_s * AIR_RIPPLE_HZ),
            ],
        }
    }

    /// The body as drawn this frame: none until it is set.
    fn drawn(&self) -> Option<Drawn> {
        let c = self.carriage.as_ref()?;
        let rig = &self.kind.rig;
        let size = self.look.traits.size;
        let (up, forward) = (c.up, c.forward);
        let right = forward.cross(up);
        let pace = self.pace();
        let gait = self.kind.gaits.at(pace);
        let moving = smoothstep(0.05, 0.6, pace);
        // In the air no stride rocks it.
        let striding = moving * self.air.footed();
        let rock = -gait.rock * striding * libm::cos(TAU * (self.stride - gait.sink));
        // The bounce dips the body under where its legs hold it, never lifts
        // it over: lifted, the legs down would hang short of the ground.
        let bounce = -gait.bounce_m
            * striding
            * (1.0 + libm::cos(TAU * gait.bounces * (self.stride - gait.sink)));
        let heave = self
            .wings
            .as_ref()
            .map_or(0.0, |wings| match self.air.flying {
                true => wings.heave(),
                false => 0.0,
            });
        let frame = DAffine3::from_mat3_translation(
            DMat3::from_cols(right, up, -forward) * size,
            c.at + up * (bounce + heave) * size,
        );
        let body = frame
            * DAffine3::from_translation(rig.pivot)
            * DAffine3::from_quat(
                DQuat::from_rotation_x(c.pitch.value() + c.leap.value() + rock)
                    * DQuat::from_rotation_z(c.roll.value()),
            )
            * DAffine3::from_translation(-rig.pivot);
        Some(Drawn {
            body,
            body_forward: -body.matrix3.z_axis.normalize(),
            up,
            right,
            gait,
            pace,
            moving,
            striding,
        })
    }

    /// Where a rider sits this frame: the saddle's place on the back, world,
    /// its columns right, up and back as the body's, rising and rocking with
    /// the stride. Through a jump the rider leans forward with it, and keeps
    /// most of its pitch off their back. None for a kind that carries no
    /// one, or until it is set.
    pub fn saddle(&self) -> Option<DAffine3> {
        let carry = self.kind.carry?;
        let c = self.carriage.as_ref()?;
        let d = self.drawn()?;
        let at = d.body.transform_point3(carry.saddle);
        let basis = DMat3::from_cols(
            d.body.matrix3.x_axis.normalize(),
            d.body.matrix3.y_axis.normalize(),
            d.body.matrix3.z_axis.normalize(),
        );
        let lean = RIDER_LEAN * self.leaning() + RIDER_COUNTER * c.leap.value();
        Some(DAffine3::from_mat3_translation(
            basis * DMat3::from_rotation_x(-lean),
            at,
        ))
    }

    /// Its side under the saddle, what a rider's legs go round: each box of
    /// its trunk there, at its size and build, top first. Empty for a kind
    /// that carries none.
    pub fn flanks(&self) -> Vec<Flank> {
        let Some(carry) = self.kind.carry else {
            return Vec::new();
        };
        let Traits { size, build, .. } = self.look.traits;
        let mut flanks: Vec<Flank> = (self.kind.rig.trunk.iter())
            .filter(|section| (section.from_z..=section.to_z).contains(&carry.saddle.z))
            .map(|section| Flank {
                top_m: (carry.saddle.y - section.top) * size,
                bottom_m: (carry.saddle.y - section.bottom) * size,
                out_m: section.width / 2.0 * build * size,
            })
            .collect();
        flanks.sort_by(|a, b| a.top_m.total_cmp(&b.top_m));
        flanks
    }

    /// How fast it goes under a rider, as a share of its kind's paces: by
    /// its size ([`crate::fauna::BySize`]). One for a kind that carries none.
    pub fn speed_share(&self) -> f64 {
        self.kind
            .carry
            .map_or(1.0, |carry| carry.by_size.share(self.look.traits.size))
    }

    /// How far into a jump it is, for its rider, 0 to 1: up as it gathers,
    /// all of it in the air, and down again as it lands.
    fn leaning(&self) -> f64 {
        if self.air.flying {
            return 1.0;
        }
        if let Some(t) = self.air.landing() {
            return 1.0 - smoothstep(0.0, 1.0, t);
        }
        match self.leap {
            Some(Leap::Gathering(gathered)) => smoothstep(0.0, 1.0, gathered),
            _ => 0.0,
        }
    }

    /// One leg solved from its root to its fetlock, a fore knee bent
    /// forward, a hind hock back. Also how far its pastern and foot fold
    /// back, radians.
    fn leg(&self, leg: usize, d: &Drawn) -> (LegPose, f64) {
        let rig = &self.kind.rig;
        let (size, build) = (self.look.traits.size, self.look.traits.build);
        let rest = rig.leg(leg);
        let foot = &self.feet[leg];
        let root = d.body.transform_point3(root(&rest, foot, build));
        let mut target = foot.at + d.up * (foot.lift_m + (rig.pastern_m + rig.hoof_m) * size);
        // A foot swung up over a step or tucked comes no higher than a
        // tuck under the body, eased into: a leg never points up.
        let leg_m = (rest.upper_m + rest.lower_m) * size;
        let (tuck, soft) = (TUCK * leg_m, TUCK_SOFT * leg_m);
        let over = (target - root).dot(d.up) + tuck + soft;
        if over > 0.0 {
            target -= d.up * (over - soft * (1.0 - libm::exp(-over / soft)));
        }
        // A leg bends in the plane the body walks in, square to the leg
        // itself, so however far ahead or behind it reaches it has one side
        // to bend to and never flips to the other: a knee forward, a hock
        // back.
        let along = (target - root).normalize_or(-d.up);
        let side = d.body.matrix3.x_axis.normalize();
        let pole = match Rig::hind(leg) {
            true => along.cross(side),
            false => side.cross(along),
        }
        .normalize_or(d.body_forward);
        let most = (rig.fold + rig.fold_per_lift * d.gait.lift_m)
            * if Rig::hind(leg) { 0.85 } else { 1.0 };
        let fold = most * libm::sin(PI * foot.swing) * smoothstep(0.05, 0.6, d.pace);
        let reached = ik::solve(
            root,
            target,
            rest.upper_m * size,
            rest.lower_m * size,
            pole,
            STRETCH,
        );
        let pose = LegPose {
            root,
            joint: reached.joint,
            fetlock: reached.end,
            target,
            pole,
        };
        (pose, fold)
    }

    /// Each leg as drawn this frame, and the body's frame, rig space to
    /// world: none until it is set.
    pub fn legs(&self) -> Option<(DAffine3, [LegPose; LEGS])> {
        let d = self.drawn()?;
        Some((d.body, core::array::from_fn(|leg| self.leg(leg, &d).0)))
    }

    /// Where each foot stands or swings over this frame, world, on the
    /// ground: what a check of the legs reads.
    pub fn feet(&self) -> [DVec3; LEGS] {
        core::array::from_fn(|leg| self.feet[leg].at)
    }

    /// Which feet are down this frame: one coming down from the air is
    /// not, until it is there.
    pub fn planted(&self) -> [bool; LEGS] {
        core::array::from_fn(|leg| {
            let foot = &self.feet[leg];
            foot.planted.is_some() && foot.came.is_none()
        })
    }

    /// Its boxes this frame, appended to `out`: none until it is set.
    pub fn boxes(&self, out: &mut Vec<BoxPart>) {
        let (Some(c), Some(d)) = (&self.carriage, self.drawn()) else {
            return;
        };
        let rig = &self.kind.rig;
        let look = &self.look;
        let paint = &look.paint;
        let (size, build) = (look.traits.size, look.traits.build);
        let Drawn {
            body,
            body_forward,
            up,
            right,
            gait,
            moving,
            striding,
            ..
        } = d;

        let mut put = |transform: DAffine3, color: Vec3| {
            out.push(BoxPart {
                transform,
                color,
                shine: 0.0,
                shadow_only: false,
                sheet: false,
            });
        };
        let wide = |p: DVec3| DVec3::new(p.x * build, p.y, p.z);
        for cell in &paint.trunk {
            put(body * placed(cell.centre, wide(cell.size)), cell.color);
        }

        // The neck and head: carried by the gait, nodding with it, down in
        // the grass when grazing. The neck comes after the body's pitch with
        // ease.
        let neck = &rig.neck;
        let nod = gait.nod * striding * libm::cos(TAU * gait.nods * (self.stride - gait.sink));
        let graze = smoothstep(0.0, 1.0, self.graze);
        let mix = |a: f64, b: f64, t: f64| a + (b - a) * t;
        let lag = c.neck.value() - c.pitch.value();
        // A balk: up at once, shaken, eased back down.
        let balk = self
            .balk_s
            .filter(|&s| (0.0..BALK_S).contains(&s))
            .map_or(0.0, |s| s / BALK_S);
        let thrown = libm::sin(PI * libm::sqrt(balk));
        let raised = c.raised.value();
        let neck_angle = mix(gait.neck - nod, neck.graze_neck, graze)
            + lag
            + BALK_NECK * thrown
            + BACK_NECK * raised;
        let head_angle = mix(gait.head - nod * 0.5, neck.graze_head, graze)
            + BALK_HEAD * thrown
            + BACK_HEAD * raised;
        let sway = 0.3 * libm::sin(self.clock_s * 0.5) * graze
            + BALK_SHAKE * thrown * libm::sin(TAU * 2.0 * balk);
        let bend = c.bend.value();
        let neck_q = DQuat::from_rotation_y(sway + bend) * DQuat::from_rotation_x(neck_angle);
        let neck_frame = body * DAffine3::from_rotation_translation(neck_q, neck.root);
        let neck_w = neck.width_m * build.sqrt();
        let cuts = paint.neck.len() as f64;
        let whole = neck.length_m + neck.into_m;
        for (cut, &color) in paint.neck.iter().enumerate() {
            let from = neck.into_m - whole * cut as f64 / cuts;
            let to = neck.into_m - whole * (cut + 1) as f64 / cuts;
            put(
                neck_frame
                    * placed(
                        DVec3::new(0.0, 0.0, (from + to) / 2.0),
                        DVec3::new(neck_w, neck.depth_m, from - to),
                    ),
                color,
            );
        }
        let hair = self.hair(&gait, moving);
        let (stream, ripple) = (hair.stream, |piece: f64| hair.ripple(piece));
        if look.traits.mane_m > 0.0 {
            // The mane in pieces along the crest, from the withers up to the
            // poll: each leans back as the air takes it, rippling.
            let crest = neck.depth_m / 2.0 - 0.01;
            let span = neck.length_m * 0.95;
            let piece_m = span / MANE_PIECES as f64;
            for piece in 0..MANE_PIECES {
                let z = 0.04 - piece_m * (piece as f64 + 0.5);
                let from_withers = (MANE_PIECES - 1 - piece) as f64;
                let lean = MANE_STREAM * stream + ripple(from_withers);
                let root = neck_frame * DAffine3::from_translation(DVec3::new(0.0, crest, z));
                put(
                    root * DAffine3::from_quat(DQuat::from_rotation_x(lean))
                        * placed(
                            DVec3::Y * look.traits.mane_m / 2.0,
                            DVec3::new(0.08, look.traits.mane_m, piece_m * 1.6),
                        ),
                    paint.mane,
                );
                if look.traits.mane_falls {
                    let fall = look.traits.mane_m * 2.4;
                    put(
                        root * DAffine3::from_translation(DVec3::X * -(neck_w / 2.0 + 0.02))
                            * DAffine3::from_quat(
                                DQuat::from_rotation_z(-lean * 0.5)
                                    * DQuat::from_rotation_x(-lean * 0.6),
                            )
                            * placed(
                                -DVec3::Y * fall / 2.0,
                                DVec3::new(0.04, fall, piece_m * 0.95),
                            ),
                        paint.mane,
                    );
                }
            }
        }
        let head_q =
            DQuat::from_rotation_y(sway + bend * HEAD_LEADS) * DQuat::from_rotation_x(head_angle);
        let poll = neck.root + neck_q * DVec3::new(0.0, 0.0, -neck.length_m);
        let head = body * DAffine3::from_rotation_translation(head_q, poll);
        for (part, color) in &paint.head {
            put(head * placed(part.centre, part.size), *color);
        }
        let ears = &rig.ears;
        for side in [-1.0, 1.0] {
            let ear =
                DAffine3::from_translation(DVec3::new(side * ears.at.x, ears.at.y, ears.at.z))
                    * DAffine3::from_quat(DQuat::from_rotation_x(ears.back));
            put(
                head * ear * placed(DVec3::Y * ears.size.y / 2.0, ears.size),
                paint.ear,
            );
            let eye = rig.eye;
            put(
                head * placed(
                    DVec3::new(side * eye.centre.x, eye.centre.y, eye.centre.z),
                    eye.size,
                ),
                paint.eye,
            );
        }

        // The tail: hanging and swishing standing still, swinging with the
        // stride and lifted the faster it goes, trailing on the wing.
        let tail = &rig.tail;
        let (swish, lift) = (hair.swish, hair.stream);
        let dock_q = DQuat::from_rotation_y(swish) * DQuat::from_rotation_x(tail.hang - 0.5 * lift);
        let dock = body * DAffine3::from_rotation_translation(dock_q, tail.root);
        put(
            dock * placed(
                DVec3::new(0.0, 0.0, tail.dock_m / 2.0 - 0.04),
                DVec3::new(tail.dock_width_m, tail.dock_width_m, tail.dock_m + 0.08),
            ),
            paint.tail,
        );
        if look.traits.tail_m > 0.0 {
            // The hair in pieces, each bending from the one before after it,
            // so a wave runs down it: streaming out behind at a gallop.
            let hair_q = DQuat::from_rotation_y(swish * 1.7)
                * DQuat::from_rotation_x(tail.hair_hang - 0.8 * lift);
            let dock_end = tail.root + dock_q * DVec3::new(0.0, 0.0, tail.dock_m);
            let mut hair = body * DAffine3::from_rotation_translation(hair_q, dock_end);
            let [w, d] = tail.hair;
            let piece_m = look.traits.tail_m / TAIL_PIECES as f64;
            for piece in 0..TAIL_PIECES {
                let taper = 1.0 - 0.15 * piece as f64;
                hair *= DAffine3::from_quat(
                    DQuat::from_rotation_x(ripple(piece as f64 + 1.0))
                        * DQuat::from_rotation_y(swish * 0.4),
                );
                put(
                    hair * placed(
                        DVec3::new(0.0, 0.0, piece_m / 2.0 - 0.03),
                        DVec3::new(w * taper, d * taper, piece_m + 0.03),
                    ),
                    paint.tail,
                );
                hair *= DAffine3::from_translation(DVec3::Z * (piece_m - 0.03));
            }
        }

        // The legs: below the fetlock the pastern and foot fold back in the
        // swing.
        let width = size * build;
        for leg in 0..LEGS {
            let rest = rig.leg(leg);
            let (pose, fold) = self.leg(leg, &d);
            let toe = (-up * libm::cos(fold) - body_forward * libm::sin(fold)).normalize();
            let up_upper = (pose.root - pose.joint).normalize_or(up);
            let colors = paint.legs[leg];
            let [w, dp] = rest.upper;
            put(
                bone(
                    pose.joint,
                    pose.root + up_upper * UPPER_INTO_M * size,
                    right,
                    w * width,
                    dp * size,
                ),
                colors[0],
            );
            let [w, dp] = rest.lower;
            put(
                bone(pose.fetlock, pose.joint, right, w * width, dp * size),
                colors[1],
            );
            let coronet = pose.fetlock + toe * rig.pastern_m * size;
            put(
                bone(
                    coronet,
                    pose.fetlock,
                    right,
                    w * 0.9 * width,
                    dp * 0.9 * size,
                ),
                colors[2],
            );
            let [hw, hd] = rig.hoof;
            put(
                bone(
                    coronet + toe * rig.hoof_m * size,
                    coronet,
                    right,
                    hw * width,
                    hd * size,
                ),
                paint.hooves[leg],
            );
            if look.traits.feathers {
                let cannon = (pose.joint - pose.fetlock).normalize_or(up);
                put(
                    bone(
                        pose.fetlock + toe * 0.1 * size,
                        pose.fetlock + cannon * 0.14 * size,
                        right,
                        w * 1.7 * width,
                        dp * 1.7 * size,
                    ),
                    paint.feathers[leg],
                );
            }
        }
        if let Some(wings) = &self.wings {
            wings.boxes(body, build, paint.wings, out);
        }
    }
}

/// Where a leg meets the body, rig space: swung along with its foot.
fn root(rest: &Leg, foot: &Foot, build: f64) -> DVec3 {
    DVec3::new(rest.root.x * build, rest.root.y, rest.root.z) - DVec3::Z * foot.along_m * rest.sweep
}

/// A direction along the ground: `heading` with its part along `up` taken
/// out.
fn along(heading: DVec3, up: DVec3) -> DVec3 {
    (heading - up * heading.dot(up)).normalize_or(up.any_orthonormal_vector())
}

/// A box of `size` centred at `centre`.
fn placed(centre: DVec3, size: DVec3) -> DAffine3 {
    DAffine3::from_translation(centre) * DAffine3::from_scale(size)
}

/// A box from `bottom` to `top`, `width` across `right` and `depth` across
/// the other way.
fn bone(bottom: DVec3, top: DVec3, right: DVec3, width: f64, depth: f64) -> DAffine3 {
    let axis = top - bottom;
    let length = axis.length().max(1e-6);
    let y = axis / length;
    let x = (right - y * right.dot(y)).normalize_or(y.any_orthonormal_vector());
    let z = x.cross(y);
    DAffine3::from_mat3_translation(
        DMat3::from_cols(x * width, y * length, z * depth),
        (top + bottom) / 2.0,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fauna::horse::HORSE;
    use crate::fauna::{Footing, Ground};

    /// Flat ground far under a pegasus on the wing.
    struct Far;

    impl Ground for Far {
        fn under(&self, at: DVec3) -> Footing {
            Footing {
                point: DVec3::new(at.x, 0.0, at.z),
                up: DVec3::Y,
                surface: 0,
            }
        }
    }

    /// A pegasus flown with forward held, at each speed a way flies it, on
    /// a line and turning: its tail trails calm behind it, never swinging
    /// more than a little in a frame, at any frame rate.
    #[test]
    fn on_the_wing_the_tail_trails_calm() {
        for fps in [30.0, 60.0, 144.0] {
            for speed_mps in [18.0, 45.0, 90.0, 150.0] {
                for turn_rps in [0.0, 0.8] {
                    let mut animal = Animal::new(&HORSE, Look::new(&HORSE, 0, Some("white")));
                    let dt = 1.0 / fps;
                    let (mut position, mut heading) = (DVec3::new(0.0, 60.0, 0.0), DVec3::X);
                    let mut last: Option<[f64; TAIL_PIECES + 1]> = None;
                    for frame in 0..(fps as usize * 4) {
                        heading = DQuat::from_rotation_y(turn_rps * dt) * heading;
                        position += heading * speed_mps * dt;
                        let going = Going {
                            position,
                            heading,
                            speed_mps,
                            turn_rps,
                            graze: 0.0,
                            balk_s: None,
                            leap: None,
                            wings: true,
                        };
                        animal.step(dt, going, &Far);
                        let gait = HORSE.gaits.at(animal.pace());
                        let hair = animal.hair(&gait, 1.0);
                        let now: [f64; TAIL_PIECES + 1] =
                            core::array::from_fn(|piece| match piece {
                                0 => hair.swish,
                                piece => hair.ripple(piece as f64),
                            });
                        let swing = last.map_or(0.0, |last| {
                            now.iter()
                                .zip(last)
                                .map(|(now, last)| (now - last).abs())
                                .fold(0.0, f64::max)
                        });
                        // A calm trail moves a few hundredths of a radian a
                        // frame at 30 fps, never a snap.
                        let most = 1.2 * dt;
                        // Its wings open first, and the trail eases in
                        // with them.
                        let wing = HORSE.wings.expect("a horse has wings").wing;
                        let opening = (wing.open_s + 0.2) * fps;
                        assert!(
                            (frame as f64) < opening || swing <= most,
                            "{speed_mps} m/s, turning {turn_rps}, {fps} fps, frame \
                             {frame}: the tail swung {swing} rad"
                        );
                        last = Some(now);
                    }
                }
            }
        }
    }
}
