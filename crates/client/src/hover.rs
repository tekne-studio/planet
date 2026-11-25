//! Hover: a way of moving a plugin gives a body, said as data, and how a body
//! moves in it. The plugin names the way and draws what it draws; the body,
//! its speed and its cushion are the core's.
//!
//! A body in a way glides over the surface under it, the smooth ground a
//! walker stands on or the sea, never the corners of the cubes. Steered, its
//! heading turns as it is told and carries the speed with it; it slips little
//! sideways, a slope pulls it, and it leans into a turn by the pull of the
//! turn against gravity.
//!
//! A cushion holds the body over the surface. It reads the surface smoothed
//! over an area, around the body and ahead of it, never one point, so the
//! body spans a hollow, rises before a step it is coming at, and floats over
//! a bump. The body's pitch and roll follow the slope it reads, and its
//! height the height it reads, critically damped: the body glides along its
//! own slope at once, and only what the surface does besides is eased.
//!
//! Sprint speeds it, and only. Jump pressed on the cushion hops it off, and
//! held lifts a way that lifts: the body climbs on from the hop, no faster
//! than its lift, to a ceiling over the surface that is lower the steeper
//! the surface is. Let go, or over a crest, it sinks back to its cushion, no
//! faster than its sink. The cushion pushes as hard as it must; the true
//! ground under the body is a floor it never goes through.
//!
//! Pure: the surface is handed in as heights, so this file knows no
//! generator, no sphere and no cell, and its tests need none.

use glam::{DQuat, DVec3};

use crate::spring::{Follow, Spring};

/// How a body hovers: a way of moving a plugin gives, as data.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Hover {
    /// How high the cushion holds the feet over the surface under them,
    /// metres.
    pub height_m: f64,
    /// Top speed on level ground, metres per second.
    pub top_mps: f64,
    /// Top speed with sprint held: the boost.
    pub sprint_mps: f64,
    /// How fast it gains speed, metres per second squared. It brakes half
    /// as fast again.
    pub accel_mps2: f64,
    /// How fast its heading turns, radians per second: steered, at full
    /// lock; going where it is pushed, at most.
    pub turn_per_s: f64,
    /// The most it leans into a turn, radians.
    pub lean_rad: f64,
    /// How fast it slows with nothing asked, and over its top speed: the
    /// fraction of its speed it keeps each second is `exp(-coast_per_s)`.
    pub coast_per_s: f64,
    /// How much of gravity along a slope pulls it: zero holds it on any
    /// slope, one is a sled on ice.
    pub slope: f64,
    /// Glides over the sea where there is one; otherwise over the sea
    /// floor.
    pub over_sea: bool,
    /// How far the cushion bobs, metres, and how often, per second: what is
    /// drawn, never where the body is.
    pub bob_m: f64,
    pub bob_hz: f64,
    /// How far around its middle the cushion reads the surface, metres:
    /// this far ahead, behind and to each side. Wider rides smoother.
    pub span_m: f64,
    /// The surface's detail finer than this many metres the cushion never
    /// feels: it reads the ground smoothed over this footprint.
    pub footprint_m: f64,
    /// How fast jump held lifts it, metres per second: zero lifts nothing.
    pub lift_mps: f64,
    /// How fast it sinks back to its cushion, at most, metres per second.
    pub sink_mps: f64,
    /// The most jump held lifts it over the surface, metres, on level
    /// ground. A slope of [`STEEP`] halves it, and steeper takes more.
    pub ceiling_m: f64,
    /// How high a body hops as it takes the way, metres over its cushion,
    /// and as it leaves it, over where it rode: zero steps on and off.
    pub hop_m: f64,
    /// How high jump pressed hops it off its cushion, metres: zero hops
    /// nothing.
    pub jump_m: f64,
}

/// The rise per metre that halves a way's ceiling.
pub const STEEP: f64 = 0.5;

/// A board, as a plugin would give it: what the tests of a body riding in
/// the world ride.
#[cfg(test)]
pub(crate) const BOARD: Hover = Hover {
    height_m: 0.35,
    top_mps: 14.0,
    sprint_mps: 22.0,
    accel_mps2: 7.0,
    turn_per_s: 2.4,
    lean_rad: 0.5,
    coast_per_s: 0.6,
    slope: 0.6,
    over_sea: true,
    bob_m: 0.03,
    bob_hz: 0.8,
    span_m: 1.5,
    footprint_m: 4.0,
    lift_mps: 10.0,
    sink_mps: 3.0,
    ceiling_m: 50.0,
    hop_m: 0.45,
    jump_m: 1.5,
};

impl Hover {
    /// How fast up a body standing on the ground leaves as it takes the
    /// way, under `gravity_mps2`, to hop its height over the cushion,
    /// metres per second.
    pub(crate) fn hop_on_mps(&self, gravity_mps2: f64) -> f64 {
        if self.hop_m > 0.0 {
            (2.0 * gravity_mps2 * (self.height_m + self.hop_m)).sqrt()
        } else {
            0.0
        }
    }

    /// How fast up jump sends it off its cushion, under `gravity_mps2`, to
    /// hop its jump's height, metres per second.
    pub(crate) fn jump_mps(&self, gravity_mps2: f64) -> f64 {
        (2.0 * gravity_mps2 * self.jump_m.max(0.0)).sqrt()
    }

    /// How fast up a body leaves the way, under `gravity_mps2`, metres per
    /// second.
    pub(crate) fn hop_off_mps(&self, gravity_mps2: f64) -> f64 {
        (2.0 * gravity_mps2 * self.hop_m.max(0.0)).sqrt()
    }
}

/// How fast the body's height closes on what the cushion asks, per second.
const CUSHION_PER_S: f64 = 7.0;
/// How fast its pitch and roll come after the slope under it, per second.
const TILT_PER_S: f64 = 6.0;
/// The cushion holds the body within this many of its heights over the
/// surface. Higher, the body is in the air.
const REACH: f64 = 3.0;
/// Over the true ground, the cushion holds the feet at least this much of
/// their height, however the smoothed surface it reads dips under it.
const LEAST: f64 = 0.5;
/// The feet never come nearer the true ground than this much of their
/// height: what the cushion's ease would let sink, this floor holds.
const FLOOR: f64 = 0.25;
/// Over this many metres past its cushion's reach the body stops rising
/// and falling with the slope under it: in the air, it keeps its height.
const ALOFT_M: f64 = 2.0;
/// How fast sideways slip dies, per second: the body grips.
const GRIP_PER_S: f64 = 5.0;
/// How fast the lean comes after what the turn asks, per second.
pub(crate) const LEAN_PER_S: f64 = 6.0;
/// Under this rise per metre a slope pulls nothing: a body at rest on
/// gentle ground stays at rest.
const LEVEL: f64 = 0.08;
/// Going where it is pushed, over this speed, pushed well behind the
/// heading brakes rather than turns; under it, the body turns on the spot.
const BRAKE_OVER_MPS: f64 = 1.5;
/// Where the cushion reads ahead of the body: where its middle will be in
/// these many seconds at its speed. What stands above the slope there
/// counts less the later it comes, and nothing from [`FADE_S`] on.
const AHEAD_S: [f64; 3] = [0.1, 0.2, 0.3];
const FADE_S: f64 = 0.4;

/// The surface under a body and ahead of it, as its cushion reads it.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) struct Reading {
    /// The surface the cushion holds the feet over: under the body's
    /// middle, raised by as much as anything around it or coming at it
    /// stands above the slope the body is on.
    pub(crate) clear_m: f64,
    /// The true ground under the body's middle, or what is built there:
    /// the floor of the cushion.
    pub(crate) ground_m: f64,
    /// Rise per metre along the heading, and across it to the right: the
    /// slope the body is on.
    pub(crate) along: f64,
    pub(crate) across: f64,
}

impl Reading {
    /// Over the true ground at `ground_m`, the surface read being smoothed.
    pub(crate) fn over(self, ground_m: f64) -> Reading {
        Reading { ground_m, ..self }
    }

    /// On a floor the surface does not know, what is built: the cushion
    /// holds the feet over it as over the surface.
    pub(crate) fn on(self, floor_m: f64) -> Reading {
        Reading {
            clear_m: self.clear_m.max(floor_m),
            ground_m: self.ground_m.max(floor_m),
            ..self
        }
    }

    /// How steep the surface read is: its rise per metre.
    fn steepness(&self) -> f64 {
        self.along.hypot(self.across)
    }
}

/// Reads the surface around a body pointing along `heading` and ahead of
/// it: `surface` gives the surface's height at an offset from the body,
/// world space. Around the body, its middle and the way's span ahead,
/// behind and to each side; ahead, where its middle will be at `speed_mps`
/// in each of [`AHEAD_S`]. The true ground is the surface under the
/// middle, until [`Reading::over`] says otherwise.
pub(crate) fn read(
    hover: &Hover,
    up: DVec3,
    heading: DVec3,
    speed_mps: f64,
    surface: impl Fn(DVec3) -> f64,
) -> Reading {
    let right = heading.cross(up);
    let (half, side) = (hover.span_m, hover.span_m);
    let middle = surface(DVec3::ZERO);
    let (nose, tail) = (surface(heading * half), surface(-heading * half));
    let (left, starboard) = (surface(-right * side), surface(right * side));
    let along = (nose - tail) / (2.0 * half).max(1e-6);
    let across = (starboard - left) / (2.0 * side).max(1e-6);
    // How far a height stands above the body's plane, read back to under
    // its middle: nothing anywhere on a plane, the depth of a hollow under
    // its ends.
    let above =
        |height: f64, ahead: f64, aside: f64| height - middle - along * ahead - across * aside;
    let mut raise = [
        above(nose, half, 0.0),
        above(tail, -half, 0.0),
        above(left, 0.0, -side),
        above(starboard, 0.0, side),
    ]
    .into_iter()
    .fold(0.0, f64::max);
    for seconds in AHEAD_S {
        let ahead = speed_mps.max(0.0) * seconds;
        if ahead > half {
            let fade = 1.0 - seconds / FADE_S;
            raise = raise.max(fade * above(surface(heading * ahead), ahead, 0.0));
        }
    }
    Reading {
        clear_m: middle + raise,
        ground_m: middle,
        along,
        across,
    }
}

/// What one step of a glide is taken in: the way the body moves in, its up,
/// gravity's pull along it, and how long the step lasts, seconds.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Step<'h> {
    pub(crate) hover: &'h Hover,
    pub(crate) up: DVec3,
    pub(crate) gravity_mps2: f64,
    pub(crate) dt: f64,
}

/// How a body glides in a way of moving: what the way's physics keeps
/// between steps.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) struct Glide {
    /// Along the surface, world space, metres per second.
    pub(crate) velocity: DVec3,
    /// Along up, metres per second: positive while rising.
    pub(crate) vertical_mps: f64,
    /// Where the body points: a unit tangent.
    pub(crate) heading: DVec3,
    /// Into the turn, radians: positive leans to the right of the heading.
    pub(crate) lean: f64,
    /// The body's up before the lean: the slope under it, as it follows.
    pub(crate) tilt: DVec3,
    /// Seconds in the way: the bob's clock.
    pub(crate) time_s: f64,
    /// How fast it climbs past what its slope lifts it, metres per second:
    /// up while it lifts, down while it sinks.
    climb_mps: f64,
    /// Jump was held last step: a hop is begun as it is pressed, never as
    /// it is held.
    jump: bool,
    /// How far the feet are off where the cushion or the lift holds them,
    /// and how fast that closes.
    cushion: Spring,
    /// The body's rise per metre along its heading and across it to the
    /// right: the slope it glides along and is pulled down.
    along: Follow,
    across: Follow,
}

impl Glide {
    /// Sends it up at `climb_mps`, slowed by gravity and caught by its
    /// cushion: a hop onto the way.
    pub(crate) fn hop(&mut self, climb_mps: f64) {
        self.climb_mps = climb_mps;
    }

    /// Jump as it is this step, `held` by the cushion or not: pressed on
    /// the cushion, it hops off it, as fast as the way jumps. Held on, the
    /// lift takes the climb from there, with no seam.
    pub(crate) fn jump(&mut self, hover: &Hover, gravity_mps2: f64, jump: bool, held: bool) {
        if jump && !self.jump && held {
            self.climb_mps = self.climb_mps.max(hover.jump_mps(gravity_mps2));
        }
        self.jump = jump;
    }

    /// At rest, pointing along `heading`, level under `up`.
    pub(crate) fn new(heading: DVec3, up: DVec3) -> Glide {
        Glide {
            velocity: DVec3::ZERO,
            vertical_mps: 0.0,
            heading,
            lean: 0.0,
            tilt: up,
            time_s: 0.0,
            climb_mps: 0.0,
            jump: false,
            cushion: Spring::default(),
            along: Follow::at(0.0),
            across: Follow::at(0.0),
        }
    }

    /// Steered one step, and returns the velocity along the surface. `turn`
    /// in `-1..=1` turns the heading, positive to the right, at the way's
    /// rate, with no end to it; `throttle` in `-1..=1` drives along the
    /// heading forward, and brakes backward.
    pub(crate) fn steered(&mut self, step: Step, turn: f64, throttle: f64, sprint: bool) -> DVec3 {
        let yaw = -turn.clamp(-1.0, 1.0) * step.hover.turn_per_s * step.dt;
        self.turn(step.up, yaw);
        let throttle = throttle.clamp(-1.0, 1.0);
        self.drive(step, throttle.max(0.0), (-throttle).max(0.0), sprint, yaw)
    }

    /// One step going where it is pushed, and returns the velocity along
    /// the surface. `wish` is where, world space, of length up to one: how
    /// hard. The heading turns toward it at the way's rate at most, and
    /// pushed well behind at speed it brakes.
    pub(crate) fn toward(&mut self, step: Step, wish: DVec3, sprint: bool) -> DVec3 {
        let push = wish.length().min(1.0);
        if push <= 1e-6 {
            return self.drive(step, 0.0, 0.0, sprint, 0.0);
        }
        let to = wish / wish.length();
        if self.velocity.dot(self.heading) > BRAKE_OVER_MPS && to.dot(self.heading) < -0.5 {
            return self.drive(step, 0.0, push, sprint, 0.0);
        }
        let wanted = self
            .heading
            .cross(to)
            .dot(step.up)
            .atan2(self.heading.dot(to));
        let most = step.hover.turn_per_s * step.dt;
        let yaw = wanted.clamp(-most, most);
        self.turn(step.up, yaw);
        // Pushed only as much as it points where it is pushed: turning hard
        // on the spot gains nothing until the body comes round.
        let thrust = push * to.dot(self.heading).max(0.0);
        self.drive(step, thrust, 0.0, sprint, yaw)
    }

    /// Turns the heading by `yaw` about `up`, and the speed with it: a carve
    /// keeps its speed, a skid would lose it.
    fn turn(&mut self, up: DVec3, yaw: f64) {
        let turn = DQuat::from_axis_angle(up, yaw);
        self.heading = (turn * self.heading).normalize();
        self.velocity = turn * self.velocity;
    }

    /// Drives along the heading with `thrust` and `brake`, each `0..=1`,
    /// having turned by `yaw` this step; the body's slope pulls it. Returns
    /// the velocity along the surface.
    ///
    /// Thrust fades as the speed nears the top, so a slope moves where the
    /// speed settles: lower up it, higher down it.
    fn drive(&mut self, step: Step, thrust: f64, brake: f64, sprint: bool, yaw: f64) -> DVec3 {
        let Step {
            hover,
            up,
            gravity_mps2,
            dt,
        } = step;
        let right = self.heading.cross(up);
        let pull = downhill(hover, self.slope(up), gravity_mps2);
        let along = pull.dot(self.heading);
        let forward = self.velocity.dot(self.heading);
        let side = (self.velocity.dot(right) + pull.dot(right) * dt) * (-GRIP_PER_S * dt).exp();
        let forward = if brake > 0.0 && forward > 0.0 {
            (forward + (along - 1.5 * hover.accel_mps2 * brake) * dt).max(0.0)
        } else if thrust > 0.0 {
            // v' = accel (thrust - v / top) + along, solved exactly.
            let top = if sprint {
                hover.sprint_mps
            } else {
                hover.top_mps
            };
            let rate = hover.accel_mps2 / top.max(1e-6);
            let settles = top * thrust + along / rate;
            settles + (forward - settles) * (-rate * dt).exp()
        } else {
            forward * (-hover.coast_per_s * dt).exp() + along * dt
        };
        self.velocity = self.heading * forward + right * side;

        let lean = lean(hover, forward, yaw / dt.max(1e-6), gravity_mps2);
        self.lean += (lean - self.lean) * (1.0 - (-LEAN_PER_S * dt).exp());
        self.time_s += dt;
        self.velocity
    }

    /// Follows the slope under the body with its pitch and its roll.
    pub(crate) fn tilt(&mut self, step: Step, reading: &Reading) {
        self.along.follow(reading.along, TILT_PER_S, step.dt);
        self.across.follow(reading.across, TILT_PER_S, step.dt);
        self.tilt = normal(step.up, self.slope(step.up));
    }

    /// The body's rise per metre as it is tilted, a tangent pointing uphill.
    fn slope(&self, up: DVec3) -> DVec3 {
        self.heading * self.along.value() + self.heading.cross(up) * self.across.value()
    }

    /// Floats one step: returns where the feet are, over the datum, given
    /// where they were, what the cushion reads now, how far the body went
    /// along the surface, `went_mps`, and whether jump held lifts it.
    ///
    /// Near its cushion the body rises along its own slope at once. What is
    /// asked besides, the cushion's height or, lifting, the ceiling, it
    /// comes to critically damped, its climb changing no faster than
    /// gravity: up as hard as the cushion must or as fast as the way lifts,
    /// down no faster than the way sinks. Never nearer the true ground than
    /// [`FLOOR`] of the height, whatever the speed or the step.
    pub(crate) fn float(
        &mut self,
        step: Step,
        height_m: f64,
        reading: &Reading,
        went_mps: DVec3,
        lifting: bool,
    ) -> f64 {
        let Step {
            hover,
            up,
            gravity_mps2,
            dt,
        } = step;
        if dt <= 0.0 {
            return height_m;
        }
        let carried_mps = self.climb_mps;
        let rest_m =
            (reading.clear_m + hover.height_m).max(reading.ground_m + hover.height_m * LEAST);
        let aloft_m = height_m - rest_m - hover.height_m * REACH;
        let near = (1.0 - aloft_m / ALOFT_M).clamp(0.0, 1.0);
        let coast_m = height_m + near * self.slope(up).dot(went_mps) * dt;
        let lifting = lifting && hover.lift_mps > 0.0;
        let target_m = if lifting {
            rest_m.max(reading.clear_m + ceiling_m(hover, reading.steepness()))
        } else {
            rest_m
        };
        // What the cushion alone asks: up to its height as hard as it must.
        let pushed_mps = {
            let mut cushion = Spring {
                off: coast_m - rest_m,
                rate: self.cushion.rate,
            };
            cushion.settle(CUSHION_PER_S, dt);
            (rest_m + cushion.off - coast_m) / dt
        };
        self.cushion.off = coast_m - target_m;
        self.cushion.settle(CUSHION_PER_S, dt);
        let mut climb = (target_m + self.cushion.off - coast_m) / dt;
        // Lifting, the cushion still pushes as hard as it must, and the
        // lift adds no more than its rate, eased off as the ceiling nears
        // so gravity stops the body there. Otherwise the cushion asks.
        let swing = gravity_mps2 * dt;
        let slowest = (self.climb_mps - swing).max(-hover.sink_mps);
        let fastest = if lifting {
            // The climb that slowing by `swing` a step brings to rest in
            // the room left: v^2 / 2g + v dt / 2 = room.
            let room_m = (target_m - coast_m).max(0.0);
            let stops_mps =
                (swing * swing / 4.0 + 2.0 * gravity_mps2 * room_m).sqrt() - swing / 2.0;
            let lift_mps = (self.climb_mps + swing).min(hover.lift_mps).min(stops_mps);
            lift_mps.max(pushed_mps).max(slowest)
        } else {
            f64::INFINITY
        };
        if !(slowest..=fastest).contains(&climb) {
            climb = climb.clamp(slowest, fastest);
            self.cushion = Spring {
                off: coast_m + climb * dt - target_m,
                rate: climb,
            };
        }
        self.climb_mps = climb;
        let mut next_m = coast_m + climb * dt;
        // The floor stops a fall, never a climb the body had: one hopping
        // up from under it keeps its hop, while what the cushion would have
        // pushed it up by, the floor has done.
        let floor_m = reading.ground_m + hover.height_m * FLOOR;
        if next_m < floor_m {
            next_m = floor_m;
            self.climb_mps = carried_mps.max(0.0);
            self.cushion = Spring {
                off: next_m - target_m,
                rate: self.climb_mps,
            };
        }
        self.vertical_mps = (next_m - height_m) / dt;
        next_m
    }

    /// The body's up as drawn: its tilt, leaned into the turn.
    pub(crate) fn up(&self) -> DVec3 {
        leaned(self.tilt, self.heading, self.lean)
    }

    /// Whether the cushion holds the body: false while it is higher than
    /// the cushion reaches.
    pub(crate) fn held(&self, hover: &Hover, height_m: f64, reading: &Reading) -> bool {
        height_m - reading.clear_m - hover.height_m <= hover.height_m * REACH
    }
}

/// How high jump held lifts a way over a surface `steepness` steep, metres:
/// its ceiling on level ground, half of it at [`STEEP`], less the steeper.
pub fn ceiling_m(hover: &Hover, steepness: f64) -> f64 {
    let steep = steepness / STEEP;
    hover.ceiling_m / (1.0 + steep * steep)
}

/// The surface a way glides over at ground `ground_m`: the sea where it
/// glides over the sea and the ground is under it, the ground otherwise.
pub fn surface_m(hover: &Hover, ground_m: f64) -> f64 {
    if hover.over_sea {
        ground_m.max(0.0)
    } else {
        ground_m
    }
}

/// The surface's normal from its rise per metre.
pub fn normal(up: DVec3, gradient: DVec3) -> DVec3 {
    (up - gradient).normalize_or(up)
}

/// What a slope pulls with, along the surface, metres per second squared:
/// gravity's share down it, past what level ground holds.
fn downhill(hover: &Hover, gradient: DVec3, gravity_mps2: f64) -> DVec3 {
    let rise = gradient.length();
    if rise <= LEVEL {
        return DVec3::ZERO;
    }
    // Gravity along a slope of this rise is g sin(theta); past the level
    // that holds still, it grows from nothing.
    let sin = rise / (1.0 + rise * rise).sqrt();
    let level = LEVEL / (1.0 + LEVEL * LEVEL).sqrt();
    -gradient / rise * gravity_mps2 * hover.slope * (sin - level)
}

/// How far a body leans into a turn at a speed and a rate of turn,
/// radians: the pull of the turn against gravity, as far as the way leans.
/// A turn to the left, positive about up, leans left, negative.
pub fn lean(hover: &Hover, speed_mps: f64, yaw_per_s: f64, gravity_mps2: f64) -> f64 {
    let pull = -speed_mps * yaw_per_s / gravity_mps2.max(1e-6);
    pull.atan().clamp(-hover.lean_rad, hover.lean_rad)
}

/// `up` leaned by `lean` radians about `heading`: positive to its right.
pub fn leaned(up: DVec3, heading: DVec3, lean: f64) -> DVec3 {
    (DQuat::from_axis_angle(heading, lean) * up).normalize()
}

/// How far the cushion bobs at `time_s`, metres.
pub fn bob_m(hover: &Hover, time_s: f64) -> f64 {
    hover.bob_m * (core::f64::consts::TAU * hover.bob_hz * time_s).sin()
}

#[cfg(test)]
mod tests {
    use super::*;

    const G: f64 = 14.0;
    const UP: DVec3 = DVec3::Z;
    const FRAME: f64 = 1.0 / 60.0;

    /// A step in `hover` of `dt` seconds, `z` up.
    fn step(hover: &Hover, dt: f64) -> Step<'_> {
        Step {
            hover,
            up: UP,
            gravity_mps2: G,
            dt,
        }
    }

    fn profile() -> Hover {
        Hover {
            height_m: 0.35,
            top_mps: 14.0,
            sprint_mps: 22.0,
            accel_mps2: 7.0,
            turn_per_s: 2.4,
            lean_rad: 0.5,
            coast_per_s: 0.6,
            slope: 0.6,
            over_sea: true,
            bob_m: 0.03,
            bob_hz: 0.8,
            span_m: 0.5,
            footprint_m: 0.0,
            lift_mps: 10.0,
            sink_mps: 3.0,
            ceiling_m: 50.0,
            hop_m: 0.0,
            jump_m: 1.5,
        }
    }

    /// A body gliding over a surface in a flat world, `z` up, from the
    /// origin with its feet `start_m` over the surface, for `seconds` in
    /// steps like `step`. Each step `control` drives the glide
    /// and returns its velocity and whether jump held lifts it; `each` then
    /// sees the glide, the feet's height and the surface under the body's
    /// middle. Returns the feet's height over that surface at the end.
    fn run(
        step: Step,
        surface: impl Fn(f64, f64) -> f64,
        mut glide: Glide,
        start_m: f64,
        seconds: f64,
        mut control: impl FnMut(&mut Glide) -> (DVec3, bool),
        mut each: impl FnMut(&Glide, f64, f64),
    ) -> f64 {
        let mut at = DVec3::ZERO;
        let mut height_m = surface(0.0, 0.0) + start_m;
        let mut over_m = start_m;
        for _ in 0..(seconds / step.dt).round() as usize {
            let (velocity, lifting) = control(&mut glide);
            at += velocity * step.dt;
            let under = |offset: DVec3| surface(at.x + offset.x, at.y + offset.y);
            let speed = velocity.dot(glide.heading);
            let reading = read(step.hover, UP, glide.heading, speed, under);
            glide.tilt(step, &reading);
            height_m = glide.float(step, height_m, &reading, velocity, lifting);
            over_m = height_m - surface(at.x, at.y);
            each(&glide, height_m, surface(at.x, at.y));
        }
        over_m
    }

    /// Steered flat out along its heading, sprinting or not.
    fn flat_out(sprint: bool, dt: f64) -> impl FnMut(&mut Glide) -> (DVec3, bool) {
        move |glide| (glide.steered(step(&profile(), dt), 0.0, 1.0, sprint), false)
    }

    /// Asking nothing, with sprint held or not.
    fn still(lifting: bool, dt: f64) -> impl FnMut(&mut Glide) -> (DVec3, bool) {
        move |glide| {
            (
                glide.steered(step(&profile(), dt), 0.0, 0.0, false),
                lifting,
            )
        }
    }

    /// Already at `speed_mps` along `x`.
    fn moving(speed_mps: f64) -> Glide {
        Glide {
            velocity: DVec3::X * speed_mps,
            ..Glide::new(DVec3::X, UP)
        }
    }

    #[test]
    fn it_settles_at_its_height_from_above_and_below() {
        for start_m in [0.2, 0.9] {
            let glide = Glide::new(DVec3::X, UP);
            let over_m = run(
                step(&profile(), FRAME),
                |_, _| 3.0,
                glide,
                start_m,
                2.0,
                still(false, FRAME),
                |_, _, _| {},
            );
            assert!((over_m - 0.35).abs() < 0.005, "{start_m}: {over_m}");
        }
    }

    #[test]
    fn from_high_up_it_sinks_slowly_and_lands_on_its_cushion() {
        let mut fastest: f64 = 0.0;
        let glide = Glide::new(DVec3::X, UP);
        let over_m = run(
            step(&profile(), FRAME),
            |_, _| 0.0,
            glide,
            8.0,
            4.0,
            still(false, FRAME),
            |glide, _, _| {
                fastest = fastest.min(glide.vertical_mps);
            },
        );
        assert!((-3.0 - 1e-9..-2.9).contains(&fastest), "{fastest}");
        assert!((over_m - 0.35).abs() < 0.01, "{over_m}");
    }

    /// Jump tapped on the cushion hops it its jump's height and lets it
    /// back down onto the cushion; held from the tap, the lift takes the
    /// climb from the hop on, never slower, up to the ceiling.
    #[test]
    fn jump_tapped_hops_it_and_held_lifts_it_on_from_the_hop() {
        let hover = profile();
        for held_s in [FRAME, 10.0] {
            let (mut top_m, mut climb_least, mut climbing) = (0.0_f64, f64::MAX, true);
            let over_m = run(
                step(&hover, FRAME),
                |_, _| 2.0,
                Glide::new(DVec3::X, UP),
                0.35,
                held_s + 25.0,
                |glide| {
                    let jump = glide.time_s < held_s - FRAME / 2.0;
                    glide.jump(&hover, G, jump, true);
                    (glide.steered(step(&hover, FRAME), 0.0, 0.0, false), jump)
                },
                |glide, height_m, under_m| {
                    top_m = top_m.max(height_m - under_m);
                    if held_s > 1.0 && climbing && height_m - under_m < 40.0 {
                        climb_least = climb_least.min(glide.vertical_mps);
                    }
                    climbing &= height_m - under_m < 40.0;
                },
            );
            if held_s < 1.0 {
                // The frame it is pressed it is held too, so the lift
                // adds a frame of gravity to the hop; then it flies its
                // arc a frame at a time.
                let hop_mps = hover.jump_mps(G) + G * FRAME;
                let arc_m = hop_mps * hop_mps / (2.0 * G) + hop_mps * FRAME / 2.0;
                assert!((top_m - (0.35 + arc_m)).abs() < 0.01, "hopped to {top_m} m");
            } else {
                let hop_mps = hover.jump_mps(G);
                assert!(
                    climb_least > hop_mps * 0.9,
                    "{climb_least} m/s, hopped at {hop_mps}"
                );
                assert!((top_m - 50.0).abs() < 0.5, "lifted to {top_m} m");
            }
            assert!(
                (over_m - 0.35).abs() < 0.01,
                "{over_m} m: back on its cushion"
            );
        }
    }

    /// A jump begins as it is pressed, on the cushion: held, or pressed off
    /// it, it hops no more.
    #[test]
    fn a_jump_is_pressed_on_the_cushion_never_held() {
        let hover = profile();
        let mut glide = Glide::new(DVec3::X, UP);
        glide.jump(&hover, G, true, false);
        assert_eq!(glide.climb_mps, 0.0, "off the cushion");
        glide.jump(&hover, G, true, true);
        assert_eq!(glide.climb_mps, 0.0, "held, not pressed");
        glide.jump(&hover, G, false, true);
        glide.jump(&hover, G, true, true);
        assert!(glide.climb_mps > 5.0, "pressed: {}", glide.climb_mps);
    }

    #[test]
    fn jump_held_lifts_it_to_its_ceiling_and_let_go_it_sinks_slowly() {
        let (mut fastest_up, mut fastest_down): (f64, f64) = (0.0, 0.0);
        let mut lifted_m = 0.0;
        let lift_s = 8.0;
        let over_m = run(
            step(&profile(), FRAME),
            |_, _| 2.0,
            Glide::new(DVec3::X, UP),
            0.35,
            lift_s + 20.0,
            |glide| {
                let lifting = glide.time_s < lift_s - FRAME / 2.0;
                (
                    glide.steered(step(&profile(), FRAME), 0.0, 0.0, false),
                    lifting,
                )
            },
            |glide, height_m, under_m| {
                fastest_up = fastest_up.max(glide.vertical_mps);
                fastest_down = fastest_down.min(glide.vertical_mps);
                if (glide.time_s - lift_s).abs() < FRAME / 2.0 {
                    lifted_m = height_m - under_m;
                }
            },
        );
        assert!(
            fastest_up <= 10.0 + 1e-9 && fastest_up > 9.9,
            "{fastest_up}"
        );
        assert!((lifted_m - 50.0).abs() < 0.3, "{lifted_m}");
        assert!(fastest_down >= -3.0 - 1e-9, "{fastest_down}");
        assert!((over_m - 0.35).abs() < 0.01, "{over_m}");
    }

    #[test]
    fn lifted_from_the_ground_it_climbs_no_faster_than_its_lift_nor_past_its_ceiling() {
        // Jump held from the ground, hopping on or stepping on, at any
        // frame rate: the cushion brings the body up to its height, and
        // past it the lift is the most it climbs and the ceiling the
        // highest it goes.
        let hover = profile();
        let hopping = Hover {
            hop_m: 0.45,
            ..hover
        };
        for dt in [FRAME, 1.0 / 30.0, 0.05, 0.1] {
            for hop_mps in [0.0, hopping.hop_on_mps(G)] {
                let mut glide = Glide::new(DVec3::X, UP);
                glide.hop(hop_mps);
                let (mut fastest, mut highest) = (0.0_f64, 0.0_f64);
                run(
                    step(&hover, dt),
                    |_, _| 0.0,
                    glide,
                    0.0,
                    12.0,
                    |glide| (glide.steered(step(&hover, dt), 0.0, 0.0, false), true),
                    |glide, height_m, _| {
                        fastest = fastest.max(glide.vertical_mps);
                        highest = highest.max(height_m);
                    },
                );
                let case = format!("a frame of {dt} s, a hop at {hop_mps} m/s");
                assert!(fastest <= hover.lift_mps + 1e-9, "{case}: {fastest} m/s");
                assert!(highest <= hover.ceiling_m + 0.05, "{case}: {highest} m");
                assert!(highest > hover.ceiling_m - 0.05, "{case}: {highest} m");
            }
        }
    }

    #[test]
    fn the_ceiling_is_lower_the_steeper_the_surface() {
        assert_eq!(ceiling_m(&profile(), 0.0), 50.0);
        assert_eq!(ceiling_m(&profile(), STEEP), 25.0);
        assert!(ceiling_m(&profile(), 1.0) < 11.0);
        // Held still on a slope, lifting a long while: half the ceiling.
        let held = Hover {
            slope: 0.0,
            ..profile()
        };
        let over_m = run(
            step(&held, FRAME),
            |x, _| STEEP * x,
            Glide::new(DVec3::X, UP),
            0.35,
            12.0,
            move |glide: &mut Glide| (glide.steered(step(&held, FRAME), 0.0, 0.0, false), true),
            |_, _, _| {},
        );
        assert!((over_m - 25.0).abs() < 0.3, "{over_m}");
    }

    #[test]
    fn it_climbs_a_steep_mountainside_on_its_cushion() {
        // A slope of 45 degrees, flat out up it: it keeps going up, held
        // along the slope at its height.
        let (mut slowest, mut most_off, mut frame) = (f64::MAX, 0.0_f64, 0);
        run(
            step(&profile(), FRAME),
            |x, _| x,
            Glide::new(DVec3::X, UP),
            0.35,
            12.0,
            flat_out(false, FRAME),
            |glide, height_m, under_m| {
                frame += 1;
                if frame > 240 {
                    slowest = slowest.min(glide.velocity.x);
                    most_off = most_off.max((height_m - under_m - 0.35).abs());
                }
            },
        );
        assert!(slowest > 3.0, "{slowest}");
        assert!(most_off < 0.02, "{most_off}");
    }

    #[test]
    fn a_steered_full_circle_ends_where_the_heading_says_with_no_end_to_the_turn() {
        // Full lock to the left for a circle and a half: the heading goes
        // round by the way's rate, through every direction, never held at
        // some angle off where it started.
        let hover = profile();
        let mut glide = Glide::new(DVec3::X, UP);
        let circle_s = core::f64::consts::TAU / hover.turn_per_s;
        let steps = (1.5 * circle_s / FRAME).round() as usize;
        let mut turned = 0.0;
        for _ in 0..steps {
            let before = glide.heading;
            glide.steered(step(&hover, FRAME), -1.0, 1.0, false);
            turned += before
                .cross(glide.heading)
                .dot(UP)
                .atan2(before.dot(glide.heading));
        }
        let expected = hover.turn_per_s * steps as f64 * FRAME;
        assert!((turned - expected).abs() < 1e-9, "{turned} {expected}");
        let heading = DVec3::new(expected.cos(), expected.sin(), 0.0);
        assert!(glide.heading.distance(heading) < 1e-6, "{}", glide.heading);
        assert!(
            glide.heading.x < -0.99,
            "half a turn past a circle: {}",
            glide.heading
        );
        // Right lock turns the other way.
        let mut glide = Glide::new(DVec3::X, UP);
        glide.steered(step(&hover, 0.5), 1.0, 0.0, false);
        assert!(glide.heading.y < -0.9, "{}", glide.heading);
    }

    #[test]
    fn steered_forward_is_throttle_and_back_is_brake_along_the_heading() {
        let hover = profile();
        let mut glide = Glide::new(DVec3::Y, UP);
        for _ in 0..600 {
            glide.steered(step(&hover, FRAME), 0.0, 1.0, false);
        }
        assert!(
            (13.5..14.0).contains(&glide.velocity.y),
            "{}",
            glide.velocity
        );
        assert!(glide.velocity.x.abs() < 1e-9);
        for _ in 0..30 {
            glide.steered(step(&hover, FRAME), 0.0, -1.0, false);
        }
        assert!(glide.velocity.y < 9.0, "{}", glide.velocity);
        // Braked to a stop, it never goes back.
        for _ in 0..600 {
            glide.steered(step(&hover, FRAME), 0.0, -1.0, false);
        }
        assert_eq!(glide.velocity, DVec3::ZERO);
        assert_eq!(glide.heading, DVec3::Y);
    }

    #[test]
    fn it_follows_a_slope_and_gains_speed_down_it() {
        let speed = |surface: fn(f64, f64) -> f64| {
            let (mut last, mut most_off, mut frame) = (0.0, 0.0_f64, 0);
            let glide = Glide::new(DVec3::X, UP);
            run(
                step(&profile(), FRAME),
                surface,
                glide,
                0.35,
                8.0,
                flat_out(false, FRAME),
                |glide, height_m, under_m| {
                    last = glide.velocity.length();
                    // The body starts level and settles onto the slope as it
                    // gains speed.
                    frame += 1;
                    if frame > 120 {
                        most_off = most_off.max((height_m - under_m - 0.35).abs());
                    }
                },
            );
            (last, most_off)
        };
        let level = speed(|_, _| 0.0);
        let down = speed(|x, _| -0.3 * x);
        let up = speed(|x, _| 0.3 * x);
        assert!((13.5..14.0).contains(&level.0), "{level:?}");
        assert!(down.0 > level.0 + 2.0, "{down:?}");
        assert!(up.0 < level.0 - 2.0, "{up:?}");
        // Over a steady slope the rise is taken at once: the cushion stays
        // within a centimetre.
        assert!(down.1.max(up.1) < 0.01, "{down:?} {up:?}");
    }

    #[test]
    fn over_a_step_at_top_speed_the_body_moves_a_few_centimetres_a_frame() {
        // A 25 cm step up crossed flat out, sprinting: the cushion reads it
        // coming and rises before it, smoothly, and the body never comes
        // nearer the step than the cushion's floor.
        let ledge = |x: f64, _: f64| if x > 30.0 { 0.25 } else { 0.0 };
        let (mut last, mut most, mut least): (Option<f64>, f64, f64) = (None, 0.0, f64::MAX);
        let over_m = run(
            step(&profile(), FRAME),
            ledge,
            moving(22.0),
            0.35,
            3.0,
            flat_out(true, FRAME),
            |_, height_m, under_m| {
                if let Some(last) = last {
                    most = most.max((height_m - last).abs());
                }
                last = Some(height_m);
                least = least.min(height_m - under_m);
            },
        );
        assert!(most < 0.02, "{most}");
        assert!(least > 0.35 * FLOOR, "{least}");
        assert!((over_m - 0.35).abs() < 0.01, "{over_m}");
    }

    #[test]
    fn it_floats_over_ripples_shorter_than_its_span() {
        // Ripples 8 cm high, 70 cm apart: the body spans them and rides
        // over their tops, nearly level, never following each.
        let ripples = |x: f64, _: f64| 0.08 * (x * core::f64::consts::TAU / 0.7).sin();
        let (mut low, mut high) = (f64::MAX, f64::MIN);
        let mut t = 0;
        run(
            step(&profile(), FRAME),
            ripples,
            moving(14.0),
            0.43,
            3.0,
            flat_out(false, FRAME),
            |_, height_m, _| {
                t += 1;
                if t > 60 {
                    low = low.min(height_m);
                    high = high.max(height_m);
                }
            },
        );
        assert!(high - low < 0.03, "{low} {high}");
        assert!(low > 0.08 + 0.35 * LEAST, "{low}");
    }

    #[test]
    fn it_never_sinks_into_the_ground_at_top_speed_with_a_long_frame() {
        // Hills a few metres high a few tens of metres apart, steep where
        // they turn, crossed flat out at a tenth of a second a frame.
        let hills =
            |x: f64, y: f64| 4.0 * (x / 9.0).sin() + 2.5 * (y / 6.0).cos() + (x / 2.3).sin();
        let mut lowest = f64::MAX;
        let glide = Glide::new(DVec3::new(1.0, 0.4, 0.0).normalize(), UP);
        run(
            step(&profile(), 0.1),
            hills,
            glide,
            0.35,
            30.0,
            flat_out(true, 0.1),
            |_, height_m, under_m| {
                lowest = lowest.min(height_m - under_m);
            },
        );
        assert!(lowest >= 0.35 * FLOOR - 1e-9, "{lowest}");
    }

    #[test]
    fn it_leans_into_a_turn_and_brakes_when_pushed_back() {
        let hover = profile();
        let mut glide = moving(12.0);
        // Steered left at speed: it turns left and leans left.
        for _ in 0..20 {
            glide.steered(step(&hover, FRAME), -1.0, 1.0, false);
        }
        assert!(glide.heading.y > 0.1, "{}", glide.heading);
        assert!(glide.lean < -0.2, "{}", glide.lean);
        assert!(glide.up().dot(DVec3::Y) > 0.1, "{}", glide.up());
        // Going where it is pushed, pushed straight back at speed: it slows
        // and keeps its heading.
        let heading = glide.heading;
        let before = glide.velocity.length();
        glide.toward(step(&hover, 0.1), -heading, false);
        assert!(glide.velocity.length() < before - 0.5);
        assert!(glide.heading.dot(heading) > 0.999);
        // Pushed to its side, it turns toward it at the way's rate at most.
        let mut glide = Glide::new(DVec3::X, UP);
        glide.toward(step(&hover, 0.1), DVec3::Y, false);
        let turned = glide.heading.y.atan2(glide.heading.x);
        assert!((turned - hover.turn_per_s * 0.1).abs() < 1e-9, "{turned}");
    }

    #[test]
    fn level_ground_holds_a_body_at_rest_and_the_sea_is_a_surface() {
        let hover = profile();
        let mut glide = Glide::new(DVec3::X, UP);
        glide.along = Follow::at(0.05);
        glide.steered(step(&hover, 1.0), 0.0, 0.0, false);
        assert_eq!(glide.velocity, DVec3::ZERO);
        assert_eq!(surface_m(&hover, -30.0), 0.0);
        let sinks = Hover {
            over_sea: false,
            ..hover
        };
        assert_eq!(surface_m(&sinks, -30.0), -30.0);
    }

    #[test]
    fn the_cushion_spans_a_hollow_and_floors_on_the_true_ground() {
        // A trench 40 cm deep, narrower than the span: the body rests on
        // its rims, over the trench's floor.
        let trench = |x: f64, _: f64| if x.abs() < 0.3 { -0.4 } else { 0.0 };
        let reading = read(&profile(), UP, DVec3::X, 0.0, |o: DVec3| trench(o.x, o.y));
        assert_eq!(reading.clear_m, 0.0);
        assert_eq!(reading.ground_m, -0.4);
        assert_eq!((reading.along, reading.across), (0.0, 0.0));
        // Smoothed under a bump the true ground has, the cushion holds the
        // feet over the bump all the same.
        let bumped = read(&profile(), UP, DVec3::X, 0.0, |_| 0.0).over(0.3);
        let mut glide = Glide::new(DVec3::X, UP);
        let mut height_m = 0.35;
        for _ in 0..120 {
            height_m = glide.float(
                step(&profile(), FRAME),
                height_m,
                &bumped,
                DVec3::ZERO,
                false,
            );
        }
        assert!(
            (height_m - (0.3 + 0.35 * LEAST)).abs() < 0.005,
            "{height_m}"
        );
    }
}
