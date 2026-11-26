//! Paces: a way of moving a plugin gives a body that goes on feet, said as
//! data, and how a body is paced in it.
//!
//! A body in paces goes at one of a few speeds, slowest first: forward
//! steps it up a pace, as far as the way says, back steps it down one, and
//! back from the slowest halts it. Sprint held, it goes at the pace the way
//! says sprint means, which may be past any forward steps to, and comes
//! back to its own pace when let go; held from a halt with forward, it sets
//! off at the slowest and goes on up without stopping. Back pressed and
//! held at a halt steps it backward at the way's backing, if it has one,
//! slow at first and faster while held, until let go. The speed eases
//! between paces as the way says, and the faster it goes the slower it
//! turns. Left and right turn it; it is always steered.
//!
//! Its feet are a walker's: it climbs what a walker climbs, the ground
//! holds it, a step up is taken and a wall stops it. On a slope it goes at
//! the fastest of its paces that keeps to ground so steep, and back at its
//! own as the ground eases. A body in paces is bigger than a walker, so it
//! keeps off water deeper than it wades and a roof lower than it stands.
//! What it refuses, a wall among them, it balks at: it halts and the figure
//! under it shows it.
//!
//! Jump pressed at a pace the way says it jumps from, or at a halt where it
//! has a standing jump, it gathers itself while jump is held, on at its
//! pace. Let go, it leaves the ground once it has gathered for the least
//! jump of the pace its speed has reached, or of a halt: tapped, it jumps
//! that least; held longer, higher, up to the way's most ([`Jump`]). Held
//! past that, it waits gathered. Its arc rises as high as that and falls
//! back as the world pulls, at the speed it went or, slower, the jump's
//! forward speed ([`Arc`]): it turns no more and changes no pace until it
//! lands. Where it would land refused, it balks instead. A wall its arc
//! does not clear stops it as one stops a walker, and it falls.
//!
//! A stick held paces it by its reach instead: the reach forward is cut
//! into a band a pace, from a halt through the paces forward steps up to,
//! and past [`crate::input`]'s run extent the pace sprint means, as a
//! walker runs there. A band is left only past its edge by a slack, so a
//! thumb resting on an edge keeps the gait. The thumb is the reins: the
//! stick let go halts it, pulled back halts it and, held, backs it, and
//! after a balk it stands until the stick eases off the band it balked at.
//! Without a stick the keys step it, as above.
//!
//! A way in paces may fly ([`Flight`]): mounted on an animal born with
//! wings, the body takes off and lands by the core's own toggle between
//! walking and flying, and aloft it flies as a body on its own does, where
//! one looks, at the way's speeds (`crate::controller`).
//!
//! Pure: the ground ahead is handed in as numbers, so this file knows no
//! generator, no sphere and no cell, and its tests need none.

use glam::{DQuat, DVec3};

use crate::input::{band, edges};

/// How a body goes in paces: a way of moving a plugin gives, as data.
#[derive(Clone, PartialEq, Debug)]
pub struct Paces {
    /// The paces it goes at, slowest first.
    pub paces: Vec<Pace>,
    /// The fastest of them forward steps it up to; those past it only
    /// sprint held reaches.
    pub stepped: usize,
    /// Which of them sprint held means.
    pub sprint: usize,
    /// How fast it gains speed and loses it, metres per second squared.
    pub accel_mps2: f64,
    pub decel_mps2: f64,
    /// The deepest water it goes into, metres.
    pub wade_m: f64,
    /// The lowest roof it goes under, metres over its feet.
    pub room_m: f64,
    /// How far its body reaches ahead of where it stands, to its nose, and
    /// behind it, to its rump, metres. Nothing it is stopped by comes
    /// inside either, and what it refuses is looked at as far along the
    /// way it goes.
    pub ahead_m: f64,
    pub behind_m: f64,
    /// How stiffly the eye comes after the ground under it, per second:
    /// every rise up to a step is eased, so rough ground at a gallop sways
    /// the view and never jolts it.
    pub eye_per_s: f64,
    /// The least jump it takes from a halt, springing forward; none, it
    /// jumps only from a pace that does.
    pub standing: Option<Least>,
    /// How a jump from a halt or any pace rises with how long jump is held.
    pub jump: Jump,
    /// How it steps backward; none, it does not.
    pub backing: Option<Backing>,
    /// How it flies, on an animal born with wings; none, it does not.
    pub flight: Option<Flight>,
}

/// How a body in paces flies, on an animal born with wings: taken off and
/// landed by the toggle between walking and flying, and flown where one
/// looks, as a body on its own flies.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Flight {
    /// Words the plugin's strings say, as a pace's: what the mode's readout
    /// shows while it flies, and while it flies at `ceiling_m`; for a
    /// moment after a take-off refused for want of room over it or of
    /// wings on the animal ridden, and after a landing refused for want of
    /// ground a body in paces stands on.
    pub name: &'static str,
    pub ceiling: &'static str,
    pub no_room: &'static str,
    pub wingless: &'static str,
    pub no_ground: &'static str,
    /// How fast it flies, metres per second, near the ground, and how much
    /// faster for each metre over it.
    pub speed_mps: f64,
    pub per_m: f64,
    /// How many times faster sprint held flies it.
    pub sprint: f64,
    /// The fastest it flies, metres a second, however high and with sprint
    /// held.
    pub top_mps: f64,
    /// The highest it flies, metres over the sea, wherever the ground under
    /// it is.
    pub ceiling_m: f64,
    /// How high it climbs taking off, metres over the ground: the room it
    /// needs over its head, past its own.
    pub takeoff_m: f64,
    /// How it leaves the ground for that.
    pub spring: Spring,
    /// How fast it comes down to land, metres per second, near the ground,
    /// and how much faster for each metre over it.
    pub land_mps: f64,
    pub land_per_m: f64,
    /// How far the eye rolls with the animal's bank in a turn, as a share
    /// of it: zero holds the horizon level.
    pub bank_eye: f64,
}

/// How a body in paces leaves the ground taking off: it gathers for
/// `gather_s`, carried on as fast as it went, then springs `height_m` over
/// the ground where it tops, slowing to that top in `rise_s`, carried at
/// least `forward_mps`; there the wings take it. Peers draw it by the same
/// times, from when its gait turned to flight.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Spring {
    pub gather_s: f64,
    pub rise_s: f64,
    pub height_m: f64,
    pub forward_mps: f64,
}

impl Spring {
    /// Seconds from the press to the top.
    pub fn top_s(&self) -> f64 {
        self.gather_s + self.rise_s
    }

    /// How high over where it left the ground it is `s` seconds from the
    /// press, metres, springing to `top_m` over it: on the ground through
    /// its gathering, then up, slowing to a stop at the top. Read where it
    /// is rather than stepped, so a frame of any length lands on the arc.
    pub fn height_at(&self, s: f64, top_m: f64) -> f64 {
        let t = (s - self.gather_s).clamp(0.0, self.rise_s);
        let rise_mps = 2.0 * top_m / self.rise_s;
        rise_mps * t - rise_mps / self.rise_s * t * t / 2.0
    }
}

/// A kind's spring, as its way carries it.
impl From<gaia::fauna::Spring> for Spring {
    fn from(spring: gaia::fauna::Spring) -> Spring {
        Spring {
            gather_s: spring.gather_s,
            rise_s: spring.rise_s,
            height_m: spring.height_m,
            forward_mps: spring.forward_mps,
        }
    }
}

impl Flight {
    /// How fast it flies `clearance_m` over the ground, metres per second,
    /// never past its top.
    pub(crate) fn speed_at(&self, clearance_m: f64, sprint: bool) -> f64 {
        let speed = self.speed_mps + self.per_m * clearance_m;
        let speed = if sprint { speed * self.sprint } else { speed };
        speed.min(self.top_mps)
    }

    /// How fast it comes down `clearance_m` over the ground to land,
    /// metres per second: slower the nearer, so it touches down softly.
    pub(crate) fn sink_at(&self, clearance_m: f64) -> f64 {
        self.land_mps.min(TOUCH_PER_S * clearance_m).max(TOUCH_MPS) + self.land_per_m * clearance_m
    }
}

/// Coming down to land, it touches down no faster than this, metres per
/// second, slowing to it from as far over the ground as it falls in a
/// second at this rate, per second.
const TOUCH_MPS: f64 = 0.6;
const TOUCH_PER_S: f64 = 2.0;

/// How a body in paces steps backward from a halt, back held: slow at
/// first, then faster the longer it is held.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Backing {
    /// Its name, a word the plugin's strings say, as a pace's.
    pub name: &'static str,
    /// How fast it sets off backward, metres per second, how much faster
    /// each second held, and the fastest it goes.
    pub from_mps: f64,
    pub gain_mps2: f64,
    pub top_mps: f64,
    /// How fast it turns at full lock, radians per second.
    pub turn_per_s: f64,
}

impl Backing {
    /// How fast it is asked to go backward `dt` seconds on from going
    /// `speed_mps` backward, metres per second: its first step, then a
    /// little faster each step held, never past its top.
    fn speed_after(&self, speed_mps: f64, dt: f64) -> f64 {
        (speed_mps.max(self.from_mps) + self.gain_mps2 * dt).min(self.top_mps)
    }
}

/// One pace of a body in paces.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Pace {
    /// Its name, a word the plugin's strings say: what the mode's readout
    /// shows while the body goes at it.
    pub name: &'static str,
    /// How fast it goes, metres per second.
    pub speed_mps: f64,
    /// How fast it turns at full lock, radians per second.
    pub turn_per_s: f64,
    /// The steepest ground it keeps this pace on, rise per metre: steeper,
    /// it goes at the fastest slower pace that keeps it. None for a pace
    /// kept on any ground a walker climbs; the slowest is gone at whatever
    /// it says.
    pub steep: Option<f64>,
    /// The least jump it takes from this pace; none, it does not jump
    /// from it.
    pub jump: Option<Least>,
}

/// How a body in paces jumps: the longer jump is held from the press, the
/// deeper it gathers and the higher it goes, from the least of the pace it
/// jumps from up to `most_m` its feet rise, held for `hold_s` seconds. In
/// the air it goes along the ground as fast as it went and at least
/// `forward_mps`, so a jump from a halt or a walk springs forward.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Jump {
    pub most_m: f64,
    pub hold_s: f64,
    pub forward_mps: f64,
}

/// The least jump a body in paces takes from a pace or a halt, jump
/// tapped: how long it gathers itself first, seconds, and how high its feet
/// rise, metres.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Least {
    pub gather_s: f64,
    pub height_m: f64,
}

impl Jump {
    /// How far it has gathered itself `s` seconds from the press, 0 to 1:
    /// fast at first, easing into all of it at `hold_s`. Read from the time
    /// alone, so every eye that saw the press draws the same gather.
    pub fn gathered(self, s: f64) -> f64 {
        let held = (s / self.hold_s.max(f64::EPSILON)).clamp(0.0, 1.0);
        1.0 - (1.0 - held) * (1.0 - held)
    }

    /// How high it jumps let go `s` seconds from the press, from a pace or
    /// a halt whose least is `least`: that least until it has gathered for
    /// it, then higher as it gathers deeper, the most from `hold_s` on.
    pub fn height_m(self, least: Least, s: f64) -> f64 {
        let from = self.gathered(least.gather_s);
        let charge = (self.gathered(s) - from) / (1.0 - from).max(f64::EPSILON);
        least.height_m + (self.most_m - least.height_m).max(0.0) * charge.clamp(0.0, 1.0)
    }

    /// The arc of a jump `height_m` high taken at `speed_mps`, falling back
    /// at `fall_mps2`.
    pub fn arc(self, height_m: f64, speed_mps: f64, fall_mps2: f64) -> Arc {
        Arc {
            height_m,
            speed_mps: speed_mps.max(self.forward_mps),
            fall_mps2,
        }
    }

    /// The same jump for a carrier `share` times as fast: as high, sprung
    /// forward as much faster.
    pub fn scaled(self, share: f64) -> Jump {
        Jump {
            forward_mps: self.forward_mps * share,
            ..self
        }
    }
}

/// A kind's jump and its least, as its way carries them.
impl From<gaia::fauna::Jump> for Jump {
    fn from(jump: gaia::fauna::Jump) -> Jump {
        Jump {
            most_m: jump.most_m,
            hold_s: jump.hold_s,
            forward_mps: jump.forward_mps,
        }
    }
}

impl From<gaia::fauna::Least> for Least {
    fn from(least: gaia::fauna::Least) -> Least {
        Least {
            gather_s: least.gather_s,
            height_m: least.height_m,
        }
    }
}

/// A jump's arc: a parabola its feet rise `height_m` on, going along the
/// ground at `speed_mps` and falling back at `fall_mps2`, the world's pull.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Arc {
    pub height_m: f64,
    pub speed_mps: f64,
    pub fall_mps2: f64,
}

impl Arc {
    /// How fast it leaves the ground, metres a second.
    pub fn rise_mps(self) -> f64 {
        libm::sqrt(2.0 * self.fall_mps2 * self.height_m)
    }

    /// How long it is in the air, seconds.
    pub fn air_s(self) -> f64 {
        2.0 * self.rise_mps() / self.fall_mps2.max(f64::EPSILON)
    }

    /// How far it goes from leaving the ground to landing, metres.
    pub fn length_m(self) -> f64 {
        self.speed_mps * self.air_s()
    }

    /// How high the feet are over where they left, `along` the arc from 0
    /// where it leaves to 1 where it lands.
    pub fn height_at(self, along: f64) -> f64 {
        4.0 * self.height_m * along * (1.0 - along)
    }
}

/// Where a body in paces is in a jump.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum Leaping {
    /// Gathering itself to leave the ground, `s` seconds from the press;
    /// jump let go `s` seconds from it, none while held.
    Gathering { s: f64, let_go: Option<f64> },
    /// Off the ground in its arc.
    Flying,
}

/// Under this a body in paces is asked to go nowhere, metres a second.
const STILL_MPS: f64 = 1e-6;

/// What a body in paces finds ahead of it, where it would go next.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) struct Ahead {
    /// How far the ground rises, metres, over the way's reach: negative
    /// falls.
    pub(crate) rise_m: f64,
    /// How deep the water is there, metres: zero on dry ground.
    pub(crate) water_m: f64,
    /// How high the roof is over the ground there, metres; none in the open.
    pub(crate) room_m: Option<f64>,
}

/// How a body in paces goes now: what the way keeps between steps.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) struct Pacing {
    /// The pace it was stepped to; none at a halt.
    pub(crate) pace: Option<usize>,
    /// Along its heading, metres per second: negative backing.
    pub(crate) speed_mps: f64,
    /// Where the body points: a unit tangent.
    pub(crate) heading: DVec3,
    /// How fast it turned this step, radians per second, left positive.
    pub(crate) turn_rps: f64,
    /// How steep the ground ahead is, rise per metre, up or down, eased
    /// over a moment so a block's edge does not throw the pace.
    slope: f64,
    /// Seconds since it balked at what it was asked into; none since it
    /// last set off.
    pub(crate) balked_s: Option<f64>,
    /// Where it is in a jump; none on the ground.
    pub(crate) leaping: Option<Leaping>,
    /// Back was pressed at a halt and is held: it steps backward.
    backing: bool,
    /// Forward, back and jump as they were last step: a pace is stepped and
    /// a jump begun as one is pressed, never as it is held.
    forward: bool,
    back: bool,
    jump: bool,
    /// How long forward has been held at the fastest pace it steps up to,
    /// seconds: past [`URGE_S`] it urges it on as sprint does.
    urged_s: f32,
    /// The band of its reach a stick asked last step, 0 the halt; none
    /// while no stick reins it. A byte, a band a pace: a body's way of
    /// moving holds its pacing inline.
    reined: Option<u8>,
    /// The stick asks the pace sprint means: it is urged on, as by sprint.
    spurred: bool,
    /// The band a stick asked as it balked: it asks nothing more until it
    /// eases off below it.
    checked: Option<u8>,
}

/// How long forward is held at the fastest pace it steps up to before it
/// urges it on to the pace sprint means, seconds: long enough that the
/// press that stepped it up there, still down, does not.
const URGE_S: f32 = 0.5;

/// How long the slope ahead takes to be believed, seconds.
const SLOPE_S: f64 = 0.3;
/// A wall that takes more than this share of the speed asked stops a body
/// in paces dead, and it balks; less, it goes on slower along it.
const BALKED: f64 = 0.75;

/// A push of a stick past this is a press.
const PRESSED: f64 = 0.5;

impl Paces {
    /// The same way for a carrier that goes `share` times as fast: every
    /// speed it goes, backs and springs forward at, so a jump keeps its
    /// height and goes as much further, and the turn at a pace its rate;
    /// and `size` times as long.
    pub fn scaled(&self, share: f64, size: f64) -> Paces {
        Paces {
            paces: self
                .paces
                .iter()
                .map(|pace| Pace {
                    speed_mps: pace.speed_mps * share,
                    ..*pace
                })
                .collect(),
            jump: self.jump.scaled(share),
            backing: self.backing.map(|backing| Backing {
                from_mps: backing.from_mps * share,
                gain_mps2: backing.gain_mps2 * share,
                top_mps: backing.top_mps * share,
                ..backing
            }),
            ahead_m: self.ahead_m * size,
            behind_m: self.behind_m * size,
            ..self.clone()
        }
    }

    /// The least jump it takes from the pace `from`, or from a halt for
    /// none.
    fn jump_from(&self, from: Option<usize>) -> Option<Least> {
        match from {
            Some(pace) => self.paces.get(pace)?.jump,
            None => self.standing,
        }
    }

    /// The fastest pace a carrier going `speed_mps` has reached, whatever
    /// it is asked to: what it jumps from. None below the slowest.
    fn reached(&self, speed_mps: f64) -> Option<usize> {
        // A hair under: a pace eased to is reached.
        let reached = |pace: &Pace| pace.speed_mps <= speed_mps + 1e-6;
        self.paces.iter().rposition(reached)
    }

    /// The pace sprint means, as an index that is one.
    fn sprint_pace(&self) -> usize {
        self.sprint.min(self.paces.len().saturating_sub(1))
    }

    /// How fast it turns at `speed_mps`: each pace's rate at its speed,
    /// eased between, the slowest's at a halt.
    fn turn_per_s(&self, speed_mps: f64) -> f64 {
        let mut last: Option<&Pace> = None;
        for pace in &self.paces {
            if speed_mps <= pace.speed_mps {
                return match last {
                    None => pace.turn_per_s,
                    Some(last) => {
                        let s = (speed_mps - last.speed_mps) / (pace.speed_mps - last.speed_mps);
                        last.turn_per_s + (pace.turn_per_s - last.turn_per_s) * s
                    }
                };
            }
            last = Some(pace);
        }
        last.map_or(0.0, |pace| pace.turn_per_s)
    }

    /// Whether it goes where `ahead` says: not into deep water or under a
    /// roof too low.
    pub(crate) fn takes(&self, ahead: Ahead) -> bool {
        ahead.water_m <= self.wade_m && ahead.room_m.is_none_or(|room_m| room_m >= self.room_m)
    }

    /// The fastest pace kept on ground `slope` steep: the slowest when none
    /// is.
    fn kept(&self, slope: f64) -> usize {
        self.paces
            .iter()
            .rposition(|pace| pace.steep.is_none_or(|steep| slope <= steep))
            .unwrap_or(0)
    }
}

/// What a body asks of a paced way in a step.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Reins {
    /// Steers, left negative.
    pub turn: f64,
    /// Steps the pace, forward positive.
    pub throttle: f64,
    /// Forward and back went down since the last step, however briefly: a
    /// tap steps the pace even when it was let go before the step.
    pub struck: [bool; 2],
    /// Sprint is held.
    pub sprint: bool,
    /// Jump is held.
    pub jump: bool,
    /// How far forward a stick held is pushed, of its reach, negative
    /// pulled back: it paces the way by its bands instead of the keys. None
    /// with no stick.
    pub reach: Option<f64>,
}

impl Pacing {
    /// At a halt, pointing along `heading`.
    pub(crate) fn new(heading: DVec3) -> Pacing {
        Pacing {
            pace: None,
            speed_mps: 0.0,
            heading,
            turn_rps: 0.0,
            slope: 0.0,
            balked_s: None,
            leaping: None,
            backing: false,
            forward: false,
            back: false,
            jump: false,
            urged_s: 0.0,
            reined: None,
            spurred: false,
            checked: None,
        }
    }

    /// The same pace at a new place, pointing along `heading`: a jump in
    /// the air or a balk where it was ends there.
    pub(crate) fn arrived(self, heading: DVec3) -> Pacing {
        Pacing {
            heading,
            turn_rps: 0.0,
            slope: 0.0,
            balked_s: None,
            leaping: None,
            ..self
        }
    }

    /// Whether it is asked on past its own pace: sprint held, or forward
    /// held at the fastest pace it steps up to.
    pub(crate) fn urged(&self, sprint: bool) -> bool {
        sprint || self.urged_s >= URGE_S || self.spurred
    }

    /// The pace it goes at now, as the way numbers them: the one sprint
    /// means while it is urged on and the body goes, else its own, held
    /// down to what the slope keeps. None at a halt.
    pub(crate) fn going(&self, paces: &Paces, sprint: bool) -> Option<usize> {
        let asked = match (self.urged(sprint), self.pace) {
            (true, Some(pace)) => Some(pace.max(paces.sprint_pace())),
            (_, pace) => pace,
        };
        asked.map(|pace| pace.min(paces.kept(self.slope)))
    }

    /// One step of `dt` seconds about `up`, held by `reins`. `ahead` says
    /// what it would go into, looked at along its heading. Returns the
    /// velocity along the surface; refused, it balks where it is.
    pub(crate) fn step(
        &mut self,
        paces: &Paces,
        up: DVec3,
        dt: f64,
        reins: Reins,
        ahead: Ahead,
    ) -> DVec3 {
        let Reins {
            turn,
            throttle,
            struck,
            sprint,
            jump,
            reach,
        } = reins;
        let last = paces.paces.len().checked_sub(1);
        let top = last.map(|last| paces.stepped.min(last));
        let (forward, back) = (throttle > PRESSED, throttle < -PRESSED);
        let up_a_pace = struck[0] || forward && !self.forward;
        let down_a_pace = struck[1] || back && !self.back;
        let leap = jump && !self.jump;
        (self.forward, self.back, self.jump) = (forward, back, jump);
        // In the air it goes on as it left the ground.
        if let Some(Leaping::Flying) = self.leaping {
            self.turn_rps = 0.0;
            return self.heading * self.speed_mps;
        }
        match &mut self.leaping {
            Some(Leaping::Gathering { s, let_go }) => {
                *s += dt;
                if !jump && let_go.is_none() {
                    *let_go = Some(*s);
                }
            }
            // Backing, it jumps from nothing.
            _ if leap && self.speed_mps >= 0.0 && !self.backing => {
                self.leaping = self.least(paces).map(|_| Leaping::Gathering {
                    s: 0.0,
                    let_go: None,
                });
            }
            _ => {}
        }
        match reach {
            Some(reach) => self.rein(paces, reach, back),
            None => {
                // The stick let go halts it, as the thumb leaves the reins.
                if self.reined.take().is_some() {
                    (self.pace, self.spurred, self.checked) = (None, false, None);
                }
                if up_a_pace {
                    self.pace = match self.pace {
                        None => last.map(|_| 0),
                        Some(pace) => Some(pace.saturating_add(1).min(top.unwrap_or(0))),
                    };
                }
                // Back pressed at a halt backs it; pressed going, it steps
                // down.
                if down_a_pace {
                    match self.pace {
                        None if self.speed_mps <= 0.0 => self.backing = paces.backing.is_some(),
                        pace => self.pace = pace.and_then(|pace| pace.checked_sub(1)),
                    }
                }
                self.backing &= back && self.pace.is_none();
                self.urged_s = match forward && self.pace.is_some() && self.pace == top {
                    true => self.urged_s + dt as f32,
                    false => 0.0,
                };
            }
        }
        // Halted as it gathers, it springs from where it stands, or not at
        // all.
        if let Some(Leaping::Gathering { .. }) = self.leaping
            && self.least(paces).is_none()
        {
            self.leaping = None;
        }
        // Sprint held with forward sets off from a halt.
        if sprint && forward && self.pace.is_none() && reach.is_none() {
            self.pace = last.map(|_| 0);
        }
        self.balked_s = match self.pace {
            Some(_) => None,
            None => self.balked_s.map(|s| s + dt),
        };
        let slope = ahead.rise_m.abs() / self.reach_m(paces).max(f64::EPSILON);
        self.slope += (slope - self.slope) * (1.0 - (-dt / SLOPE_S).exp());

        let backing = paces.backing.filter(|_| self.backing);
        let target = match (self.going(paces, sprint), backing) {
            (Some(pace), _) => paces.paces[pace].speed_mps,
            (None, Some(backing)) => -backing.speed_after(-self.speed_mps, dt),
            (None, None) => 0.0,
        };
        let rate = match target > self.speed_mps {
            true => paces.accel_mps2,
            false => paces.decel_mps2,
        };
        let change = (target - self.speed_mps).clamp(-rate * dt, rate * dt);
        self.speed_mps += change;

        let turn_per_s = match backing {
            Some(backing) => backing.turn_per_s,
            None => paces.turn_per_s(self.speed_mps),
        };
        self.turn_rps = -turn.clamp(-1.0, 1.0) * turn_per_s;
        self.heading = (DQuat::from_axis_angle(up, self.turn_rps * dt) * self.heading).normalize();

        if self.speed_mps != 0.0 && !paces.takes(ahead) {
            self.balk();
        }
        self.heading * self.speed_mps
    }

    /// The pace a stick pushed `reach` forward asks, `back` pulled back past
    /// a press: the band it falls in, a halt first, then a pace each up to
    /// the fastest forward steps up to, then the pace sprint means; pulled
    /// back, a halt that backs it once it stands, as back held does.
    fn rein(&mut self, paces: &Paces, reach: f64, back: bool) {
        let top = paces
            .paces
            .len()
            .checked_sub(1)
            .map(|last| paces.stepped.min(last));
        let stepped = top.map_or(0, |top| top + 1);
        let spurs = usize::from(top.is_some_and(|top| paces.sprint_pace() > top));
        let edges = core::iter::once(0.0).chain(edges(stepped + spurs));
        let band = band(reach as f32, edges, self.reined.map_or(0, usize::from));
        let band = u8::try_from(band).unwrap_or(u8::MAX);
        self.reined = Some(band);
        // Balked, it stands until the stick eases off to ask again.
        self.checked = self.checked.filter(|&balked| band >= balked);
        let asked = usize::from(band)
            .checked_sub(1)
            .filter(|_| self.checked.is_none());
        self.pace = asked.map(|pace| pace.min(top.unwrap_or(0)));
        self.spurred = asked.is_some_and(|pace| pace >= stepped);
        self.urged_s = 0.0;
        self.backing =
            back && self.pace.is_none() && self.speed_mps <= 0.0 && paces.backing.is_some();
    }

    /// A turn held: it keeps `heading`, turning nothing, and goes along it
    /// at its speed.
    pub(crate) fn held(&mut self, heading: DVec3) -> DVec3 {
        self.heading = heading;
        self.turn_rps = 0.0;
        self.heading * self.speed_mps
    }

    /// The way it goes along the ground, unit: its heading, or behind it
    /// while it backs. What it would go into is looked at along it.
    pub(crate) fn way(&self) -> DVec3 {
        match self.backing || self.speed_mps < 0.0 {
            true => -self.heading,
            false => self.heading,
        }
    }

    /// How far its body reaches along the way it goes, metres: as far as
    /// what it would go into is looked at.
    pub(crate) fn reach_m(&self, paces: &Paces) -> f64 {
        match self.way() == self.heading {
            true => paces.ahead_m,
            false => paces.behind_m,
        }
    }

    /// Whether it steps backward: what the readout names.
    pub(crate) fn backs(&self) -> bool {
        self.backing
    }

    /// The least jump it takes now: from the pace its speed has reached,
    /// easing up to a faster one, or from a halt, halted or not yet at its
    /// slowest pace. None where it does not jump.
    fn least(&self, paces: &Paces) -> Option<Least> {
        paces.jump_from(self.pace.and(paces.reached(self.speed_mps)))
    }

    /// The arc it leaves the ground in now, once jump is let go and it has
    /// gathered for its least, falling back at `fall_mps2`: it leaves the
    /// ground in it, or balks.
    pub(crate) fn gathered(&self, paces: &Paces, fall_mps2: f64) -> Option<Arc> {
        let Some(Leaping::Gathering {
            s,
            let_go: Some(let_go),
        }) = self.leaping
        else {
            return None;
        };
        let least = self.least(paces)?;
        let height_m = paces.jump.height_m(least, let_go);
        (s >= least.gather_s).then(|| paces.jump.arc(height_m, self.speed_mps, fall_mps2))
    }

    /// Whether it is in a jump: gathering itself, or in the air.
    pub(crate) fn leaping(&self) -> bool {
        self.leaping.is_some()
    }

    /// Leaves the ground in `arc`, going along it as the arc does: returns
    /// how fast it rises, metres a second.
    pub(crate) fn fly(&mut self, arc: Arc) -> f64 {
        self.speed_mps = arc.speed_mps;
        self.leaping = Some(Leaping::Flying);
        arc.rise_mps()
    }

    /// Whether it is in the air on a jump.
    pub(crate) fn flies(&self) -> bool {
        self.leaping == Some(Leaping::Flying)
    }

    /// Back on the ground: the jump is over.
    pub(crate) fn land(&mut self) {
        self.leaping = None;
    }

    /// Where it would land refused, it does not jump: it balks.
    pub(crate) fn refuse(&mut self) {
        self.balk();
    }

    /// What a wall took: the body went `went_mps` along its [`way`] where it
    /// asked for more. Stopped dead, it balks; glancing, it goes on slower.
    /// In the air it goes on as slow as the wall left it, and falls.
    ///
    /// [`way`]: Pacing::way
    pub(crate) fn stopped(&mut self, went_mps: f64) {
        let asked_mps = self.speed_mps.abs();
        // Easing through a halt, from backing to a pace, its speed passes a
        // hair from zero: asked to go nowhere, nothing stopped it.
        if asked_mps < STILL_MPS {
            return;
        }
        let went_mps = went_mps.clamp(0.0, asked_mps);
        if !self.flies() && went_mps < asked_mps * (1.0 - BALKED) {
            self.balk();
        } else {
            self.speed_mps = went_mps.copysign(self.speed_mps);
        }
    }

    /// Halts at what it refuses, and shows it.
    fn balk(&mut self) {
        self.pace = None;
        self.spurred = false;
        self.checked = self.reined;
        self.leaping = None;
        self.backing = false;
        self.speed_mps = 0.0;
        self.balked_s = Some(0.0);
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    const UP: DVec3 = DVec3::Z;
    const FRAME: f64 = 1.0 / 60.0;
    const OPEN: Ahead = Ahead {
        rise_m: 0.0,
        water_m: 0.0,
        room_m: None,
    };

    /// A horse as the ride plugin gives one: its kind's table, named.
    pub(crate) fn horse() -> Paces {
        let kind = &gaia::fauna::horse::HORSE;
        let carry = kind.carry.expect("a horse carries a rider");
        let [ahead_m, behind_m] = kind.reach_m();
        Paces {
            paces: carry
                .paces
                .iter()
                .zip(["walk", "trot", "gallop", "run"])
                .map(|(pace, name)| Pace {
                    name,
                    speed_mps: pace.speed_mps,
                    turn_per_s: pace.turn_per_s,
                    steep: pace.steep,
                    jump: pace.jump.map(Into::into),
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
            eye_per_s: 10.0,
            standing: carry.standing.map(Into::into),
            jump: carry.jump.into(),
            flight: None,
            backing: carry.backing.map(|backing| Backing {
                name: "back",
                from_mps: backing.from_mps,
                gain_mps2: backing.gain_mps2,
                top_mps: backing.top_mps,
                turn_per_s: backing.turn_per_s,
            }),
        }
    }

    /// A horse born with wings, as the ride plugin gives one: its flight
    /// the horse kind's.
    pub(crate) fn pegasus() -> Paces {
        let flight = gaia::fauna::horse::HORSE.wings.expect("wings").flight;
        Paces {
            flight: Some(Flight {
                name: "fly",
                ceiling: "ceiling",
                no_room: "no_room",
                wingless: "wingless",
                no_ground: "no_ground",
                speed_mps: flight.speed_mps,
                per_m: flight.per_m,
                sprint: flight.sprint,
                top_mps: flight.top_mps,
                ceiling_m: flight.ceiling_m,
                takeoff_m: flight.takeoff_m,
                spring: flight.spring.into(),
                land_mps: flight.land_mps,
                land_per_m: flight.land_per_m,
                bank_eye: 0.3,
            }),
            ..horse()
        }
    }

    /// Holds `throttle` and `sprint` for `seconds`.
    fn hold(pacing: &mut Pacing, throttle: f64, sprint: bool, seconds: f64) {
        hold_over(pacing, throttle, sprint, seconds, OPEN);
    }

    /// Holds `throttle` and `sprint` for `seconds` with `ahead` before it.
    fn hold_over(pacing: &mut Pacing, throttle: f64, sprint: bool, seconds: f64, ahead: Ahead) {
        for _ in 0..(seconds / FRAME).round() as usize {
            pacing.step(
                &horse(),
                UP,
                FRAME,
                Reins {
                    turn: 0.0,
                    throttle,
                    struck: [false; 2],
                    sprint,
                    jump: false,
                    reach: None,
                },
                ahead,
            );
        }
    }

    /// Taps forward or back once.
    fn tap(pacing: &mut Pacing, throttle: f64) {
        hold(pacing, throttle, false, 0.1);
        hold(pacing, 0.0, false, 0.1);
    }

    /// A stick held `reach` forward, none let go, for `seconds`, its
    /// throttle as far as it is pushed.
    fn stick(pacing: &mut Pacing, reach: Option<f64>, seconds: f64) {
        for _ in 0..(seconds / FRAME).round().max(1.0) as usize {
            pacing.step(
                &horse(),
                UP,
                FRAME,
                Reins {
                    turn: 0.0,
                    throttle: reach.unwrap_or(0.0),
                    struck: [false; 2],
                    sprint: false,
                    jump: false,
                    reach,
                },
                OPEN,
            );
        }
    }

    /// The pace a horse under a stick goes at now: a halt, walk, trot,
    /// gallop or run.
    fn gait(pacing: &Pacing) -> Option<&'static str> {
        let paces = horse();
        pacing
            .going(&paces, false)
            .map(|pace| paces.paces[pace].name)
    }

    #[test]
    fn a_stick_paces_by_its_reach_one_band_a_pace() {
        for (reach, asked) in [
            (0.1, "walk"),
            (0.45, "trot"),
            (0.75, "gallop"),
            (1.0, "run"),
        ] {
            let mut pacing = Pacing::new(DVec3::X);
            stick(&mut pacing, Some(reach), 4.0);
            assert_eq!(gait(&pacing), Some(asked), "at {reach}");
            let speed_mps = horse()
                .paces
                .iter()
                .find(|pace| pace.name == asked)
                .unwrap()
                .speed_mps;
            assert!((pacing.speed_mps - speed_mps).abs() < 1e-9, "at {reach}");
        }
        // Up and down the bands under one thumb, no press between.
        let mut pacing = Pacing::new(DVec3::X);
        for (reach, asked) in [
            (0.75, "gallop"),
            (0.1, "walk"),
            (1.0, "run"),
            (0.45, "trot"),
        ] {
            stick(&mut pacing, Some(reach), 0.1);
            assert_eq!(gait(&pacing), Some(asked), "at {reach}");
        }
    }

    #[test]
    fn a_thumb_on_an_edge_keeps_the_gait_and_past_its_slack_changes_it() {
        let slack = f64::from(crate::input::SLACK);
        let names = [
            None,
            Some("walk"),
            Some("trot"),
            Some("gallop"),
            Some("run"),
        ];
        for (edge, below) in [(0.0, 0), (0.3, 1), (0.6, 2), (0.9, 3)] {
            let (under, over) = (names[below], names[below + 1]);
            let mut pacing = Pacing::new(DVec3::X);
            stick(&mut pacing, Some(edge - 2.0 * slack), 0.1);
            assert_eq!(gait(&pacing), under, "under {edge}");
            stick(&mut pacing, Some(edge + 0.5 * slack), 0.1);
            assert_eq!(gait(&pacing), under, "resting over {edge}");
            stick(&mut pacing, Some(edge + 1.5 * slack), 0.1);
            assert_eq!(gait(&pacing), over, "past {edge} by the slack");
            stick(&mut pacing, Some(edge - 0.5 * slack), 0.1);
            assert_eq!(gait(&pacing), over, "resting under {edge}");
            stick(&mut pacing, Some(edge - 1.5 * slack), 0.1);
            assert_eq!(gait(&pacing), under, "under {edge} by the slack");
        }
    }

    #[test]
    fn the_stick_let_go_halts_it_and_the_keys_step_as_before() {
        let mut pacing = Pacing::new(DVec3::X);
        stick(&mut pacing, Some(0.75), 3.0);
        stick(&mut pacing, None, 3.0);
        assert_eq!(pacing.pace, None, "the thumb off the reins halts it");
        assert_eq!(pacing.speed_mps, 0.0);
        // The keys step it up and it cruises on, as with no stick ever.
        tap(&mut pacing, 1.0);
        tap(&mut pacing, 1.0);
        hold(&mut pacing, 0.0, false, 3.0);
        assert_eq!(pacing.pace, Some(1), "two taps, the trot, kept let go");
        assert!((pacing.speed_mps - 3.6).abs() < 1e-9);
    }

    #[test]
    fn pulled_back_it_halts_then_backs_while_held() {
        let mut pacing = Pacing::new(DVec3::X);
        stick(&mut pacing, Some(0.75), 3.0);
        stick(&mut pacing, Some(-0.8), 0.5);
        assert_eq!(pacing.pace, None);
        assert!(
            pacing.speed_mps > 0.0 && !pacing.backs(),
            "easing down, it backs nothing yet"
        );
        stick(&mut pacing, Some(-0.8), 3.0);
        assert!(
            pacing.backs() && pacing.speed_mps < 0.0,
            "stood, held back, it backs"
        );
        // Pulled back less than a press, it stands.
        stick(&mut pacing, Some(-0.3), 2.0);
        assert!(!pacing.backs());
        assert_eq!(pacing.speed_mps, 0.0);
    }

    #[test]
    fn a_balk_under_a_stick_waits_for_it_back_at_the_halt() {
        let mut pacing = Pacing::new(DVec3::X);
        stick(&mut pacing, Some(0.75), 3.0);
        pacing.stopped(0.0);
        stick(&mut pacing, Some(0.75), 1.0);
        assert_eq!(pacing.pace, None, "still pushed, it stands where it balked");
        assert!(pacing.balked_s.is_some());
        stick(&mut pacing, Some(0.45), 0.1);
        assert_eq!(gait(&pacing), Some("trot"), "eased off, it goes");
        stick(&mut pacing, Some(0.75), 0.1);
        assert_eq!(gait(&pacing), Some("gallop"));
    }

    /// Held longer, a jump goes higher: its least until it has gathered
    /// for it, the most from the hold on, rising fast at first and never
    /// back down.
    #[test]
    fn a_jump_rises_with_its_hold_from_its_least_to_its_most() {
        let jump = horse().jump;
        let least = horse().paces[2].jump.unwrap();
        assert_eq!(jump.height_m(least, 0.0), least.height_m);
        assert_eq!(jump.height_m(least, least.gather_s), least.height_m);
        assert_eq!(jump.height_m(least, jump.hold_s), jump.most_m);
        assert_eq!(jump.height_m(least, 3.0 * jump.hold_s), jump.most_m);
        let half = jump.height_m(least, (least.gather_s + jump.hold_s) / 2.0);
        assert!(
            half > (least.height_m + jump.most_m) / 2.0,
            "fast at first: {half}"
        );
        let mut was = 0.0;
        for frame in 0..60 {
            let height_m = jump.height_m(least, f64::from(frame) * FRAME);
            assert!(height_m >= was, "{frame}");
            was = height_m;
        }
        assert_eq!((jump.gathered(0.0), jump.gathered(jump.hold_s)), (0.0, 1.0));
    }

    /// A jump's arc falls back as the world pulls: it tops its height
    /// halfway, in the air as long as that height says, and goes along the
    /// ground at its speed, or springs on at the jump's forward speed.
    #[test]
    fn a_jump_arcs_under_the_world_pull() {
        let jump = horse().jump;
        for fall_mps2 in [14.0, 2.8] {
            for speed_mps in [0.0, 1.5, 9.0] {
                let arc = jump.arc(1.2, speed_mps, fall_mps2);
                let rise_mps = arc.rise_mps();
                assert!((rise_mps * rise_mps / (2.0 * fall_mps2) - 1.2).abs() < 1e-9);
                let air_s = 2.0 * libm::sqrt(2.0 * 1.2 / fall_mps2);
                assert!((arc.air_s() - air_s).abs() < 1e-9);
                let along_mps = speed_mps.max(jump.forward_mps);
                assert!((arc.length_m() - along_mps * air_s).abs() < 1e-9);
            }
        }
        let arc = jump.arc(1.2, 9.0, 14.0);
        assert!((arc.height_at(0.5) - 1.2).abs() < 1e-12);
        assert_eq!((arc.height_at(0.0), arc.height_at(1.0)), (0.0, 0.0));
    }

    #[test]
    fn forward_steps_up_a_pace_and_back_steps_down() {
        let mut pacing = Pacing::new(DVec3::X);
        hold(&mut pacing, 1.0, false, 3.0);
        assert_eq!(pacing.pace, Some(0), "held is one press");
        assert!((pacing.speed_mps - 1.5).abs() < 1e-9);
        tap(&mut pacing, 1.0);
        tap(&mut pacing, 1.0);
        tap(&mut pacing, 1.0);
        hold(&mut pacing, 0.0, false, 3.0);
        assert_eq!(pacing.pace, Some(2), "no pace past the fastest");
        assert!((pacing.speed_mps - 9.0).abs() < 1e-9);
        tap(&mut pacing, -1.0);
        hold(&mut pacing, 0.0, false, 2.0);
        assert!((pacing.speed_mps - 3.6).abs() < 1e-9);
        tap(&mut pacing, -1.0);
        tap(&mut pacing, -1.0);
        hold(&mut pacing, 0.0, false, 2.0);
        assert_eq!(pacing.pace, None);
        assert_eq!(pacing.speed_mps, 0.0);
    }

    #[test]
    fn a_tap_let_go_before_a_step_steps_the_pace() {
        let mut pacing = Pacing::new(DVec3::X);
        let flick = |struck| Reins {
            turn: 0.0,
            throttle: 0.0,
            struck,
            sprint: false,
            jump: false,
            reach: None,
        };
        pacing.step(&horse(), UP, FRAME, flick([true, false]), OPEN);
        pacing.step(&horse(), UP, FRAME, flick([true, false]), OPEN);
        assert_eq!(pacing.pace, Some(1), "two flicks, two paces up");
        pacing.step(&horse(), UP, FRAME, flick([false, true]), OPEN);
        assert_eq!(pacing.pace, Some(0));
    }

    /// Held, backing sets off a step at a time and goes faster each moment
    /// up to its top, never past it; let go and held again, it starts slow
    /// again.
    #[test]
    fn backing_starts_slow_and_gets_faster_while_held() {
        let paces = horse();
        let backing = paces.backing.unwrap();
        let mut pacing = Pacing::new(DVec3::X);
        let mut last = 0.0;
        let mut at = Vec::new();
        for frame in 1..=180 {
            hold(&mut pacing, -1.0, false, FRAME);
            let speed = -pacing.speed_mps;
            assert!(speed >= last - 1e-12, "frame {frame}: {speed} after {last}");
            assert!(speed <= backing.top_mps + 1e-12);
            last = speed;
            if frame % 15 == 0 {
                at.push(speed);
            }
        }
        // A quarter of a second in it is near its first step; three
        // quarters in, between; a second and a half in, at its top.
        assert!(at[0] < 0.75, "{at:?}");
        assert!(at[2] > 0.9 && at[2] < backing.top_mps - 0.2, "{at:?}");
        assert!((at[5] - backing.top_mps).abs() < 1e-9, "{at:?}");
        hold(&mut pacing, 0.0, false, 0.5);
        assert_eq!(pacing.speed_mps, 0.0, "let go, it halts");
        hold(&mut pacing, -1.0, false, 0.25);
        assert!(-pacing.speed_mps < 0.8, "slow again: {}", pacing.speed_mps);
    }

    /// A carrier twice as fast goes every pace and backs twice as fast, and
    /// jumps twice as far in the same arc; one half again as big reaches
    /// half again as far.
    #[test]
    fn a_way_scaled_scales_every_speed_and_jump() {
        let (paces, scaled) = (horse(), horse().scaled(2.0, 1.5));
        for (pace, twice) in paces.paces.iter().zip(&scaled.paces) {
            assert_eq!(twice.speed_mps, pace.speed_mps * 2.0);
            assert_eq!(twice.turn_per_s, pace.turn_per_s);
            assert_eq!(twice.jump, pace.jump);
            let [arc, far] = [(&paces, pace), (&scaled, twice)]
                .map(|(paces, pace)| paces.jump.arc(1.2, pace.speed_mps, 14.0));
            assert_eq!(far.rise_mps(), arc.rise_mps());
            assert!((far.length_m() - 2.0 * arc.length_m()).abs() < 1e-9);
        }
        let backing = scaled.backing.unwrap();
        assert_eq!((backing.from_mps, backing.top_mps), (1.2, 2.8));
        assert_eq!(
            (scaled.ahead_m, scaled.behind_m),
            (paces.ahead_m * 1.5, paces.behind_m * 1.5)
        );
        assert_eq!(
            (scaled.accel_mps2, scaled.sprint),
            (paces.accel_mps2, paces.sprint)
        );
    }

    #[test]
    fn back_held_at_a_halt_backs_it_and_back_going_steps_it_down() {
        let mut pacing = Pacing::new(DVec3::X);
        hold(&mut pacing, -1.0, false, 1.0);
        assert!(pacing.speed_mps < -0.6, "backs: {}", pacing.speed_mps);
        assert!(pacing.backs());
        assert_eq!(pacing.way(), -DVec3::X, "looks where it goes, behind");
        hold(&mut pacing, 0.0, false, 0.5);
        assert_eq!(pacing.speed_mps, 0.0, "let go, it halts");
        assert!(!pacing.backs());

        tap(&mut pacing, 1.0);
        hold(&mut pacing, 0.0, false, 1.0);
        assert_eq!(pacing.speed_mps, 1.5);
        hold(&mut pacing, -1.0, false, 1.5);
        assert_eq!(
            (pacing.pace, pacing.speed_mps),
            (None, 0.0),
            "back held going halts it, and backs it only pressed again"
        );

        let mut cannot = horse();
        cannot.backing = None;
        let mut pacing = Pacing::new(DVec3::X);
        for _ in 0..60 {
            pacing.step(
                &cannot,
                UP,
                FRAME,
                Reins {
                    turn: 0.0,
                    throttle: -1.0,
                    struck: [false; 2],
                    sprint: false,
                    jump: false,
                    reach: None,
                },
                OPEN,
            );
        }
        assert_eq!(pacing.speed_mps, 0.0, "a way with no backing stands");
    }

    #[test]
    fn a_wall_behind_balks_it_backing() {
        let mut pacing = Pacing::new(DVec3::X);
        hold(&mut pacing, -1.0, false, 0.5);
        pacing.stopped(0.0);
        assert_eq!(pacing.speed_mps, 0.0);
        assert!(pacing.balked_s.is_some() && !pacing.backs());
    }

    /// Forward steps it up to the gallop and no further: the run is
    /// sprint's alone, from any pace.
    #[test]
    fn forward_stops_at_the_gallop_and_sprint_runs_from_any_pace() {
        let paces = horse();
        let mut pacing = Pacing::new(DVec3::X);
        for pace in 0..3 {
            tap(&mut pacing, 1.0);
            assert_eq!(pacing.pace, Some(pace));
            hold(&mut pacing, 0.0, true, 4.0);
            assert_eq!(pacing.going(&paces, true), Some(3), "sprint at {pace}");
            assert!((pacing.speed_mps - 11.7).abs() < 1e-9);
            hold(&mut pacing, 0.0, false, 4.0);
            assert_eq!(pacing.going(&paces, false), Some(pace));
        }
        for _ in 0..3 {
            tap(&mut pacing, 1.0);
        }
        assert_eq!(pacing.pace, Some(2), "forward never runs");
    }

    /// Forward held at the gallop urges it on to the run, as sprint does,
    /// once held past the press that stepped it up there; let go, it eases
    /// back to the gallop, and back steps it down to the trot. Held at a
    /// slower pace, forward steps nothing more.
    #[test]
    fn forward_held_at_the_gallop_runs_and_let_go_it_gallops() {
        let mut pacing = Pacing::new(DVec3::X);
        // To the walk, then pressed to the trot and held there.
        hold(&mut pacing, 1.0, false, FRAME);
        hold(&mut pacing, 0.0, false, FRAME);
        hold(&mut pacing, 1.0, false, 3.0);
        assert_eq!(
            pacing.going(&horse(), false),
            Some(1),
            "held at the trot, it trots"
        );
        hold(&mut pacing, 0.0, false, FRAME);
        // The press that steps it up to the gallop, held a moment.
        hold(&mut pacing, 1.0, false, 0.4);
        assert_eq!(pacing.going(&horse(), false), Some(2), "a press is no urge");
        hold(&mut pacing, 1.0, false, 3.0);
        assert_eq!(pacing.going(&horse(), false), Some(3), "held, it runs");
        assert!(
            (pacing.speed_mps - 11.7).abs() < 1e-9,
            "{} m/s",
            pacing.speed_mps
        );
        hold(&mut pacing, 0.0, false, 2.0);
        assert_eq!(pacing.going(&horse(), false), Some(2));
        assert!((pacing.speed_mps - 9.0).abs() < 1e-9, "back to the gallop");
        hold(&mut pacing, -1.0, false, FRAME);
        hold(&mut pacing, 0.0, false, FRAME);
        assert_eq!(pacing.pace, Some(1), "back steps it down to the trot");
    }

    #[test]
    fn sprint_is_the_run_and_let_go_it_comes_back_down() {
        let mut pacing = Pacing::new(DVec3::X);
        // From a halt, through the walk, the trot and the gallop without
        // stopping.
        let mut last = 0.0;
        for _ in 0..240 {
            pacing.step(
                &horse(),
                UP,
                FRAME,
                Reins {
                    turn: 0.0,
                    throttle: 1.0,
                    struck: [false; 2],
                    sprint: true,
                    jump: false,
                    reach: None,
                },
                OPEN,
            );
            assert!(pacing.speed_mps >= last);
            last = pacing.speed_mps;
        }
        assert!((pacing.speed_mps - 11.7).abs() < 1e-9);
        assert_eq!(pacing.pace, Some(0));
        // Let go, it comes back to the walk at the way's deceleration.
        pacing.step(
            &horse(),
            UP,
            FRAME,
            Reins {
                turn: 0.0,
                throttle: 0.0,
                struck: [false; 2],
                sprint: false,
                jump: false,
                reach: None,
            },
            OPEN,
        );
        assert!((pacing.speed_mps - (11.7 - 6.0 * FRAME)).abs() < 1e-9);
        hold(&mut pacing, 0.0, false, 2.0);
        assert!((pacing.speed_mps - 1.5).abs() < 1e-9);
        // Sprint held at a halt with nothing pressed goes nowhere.
        tap(&mut pacing, -1.0);
        hold(&mut pacing, 0.0, true, 2.0);
        assert_eq!(pacing.speed_mps, 0.0);
    }

    #[test]
    fn the_faster_the_slower_it_turns() {
        let paces = horse();
        assert_eq!(paces.turn_per_s(0.0), 2.2);
        assert_eq!(paces.turn_per_s(9.0), 0.8);
        assert!((paces.turn_per_s(6.3) - 1.15).abs() < 1e-9);
        let mut pacing = Pacing::new(DVec3::X);
        pacing.step(
            &paces,
            UP,
            0.5,
            Reins {
                turn: 1.0,
                throttle: 0.0,
                struck: [false; 2],
                sprint: false,
                jump: false,
                reach: None,
            },
            OPEN,
        );
        let turned = pacing.heading.y.atan2(pacing.heading.x);
        assert!(
            (turned + 1.1).abs() < 1e-9,
            "right turns clockwise: {turned}"
        );
    }

    #[test]
    fn it_balks_at_deep_water_and_a_low_roof() {
        for ahead in [
            Ahead {
                water_m: 0.7,
                ..OPEN
            },
            Ahead {
                room_m: Some(2.5),
                ..OPEN
            },
        ] {
            let mut pacing = Pacing::new(DVec3::X);
            hold(&mut pacing, 1.0, false, 1.0);
            let velocity = pacing.step(
                &horse(),
                UP,
                FRAME,
                Reins {
                    turn: 0.0,
                    throttle: 1.0,
                    struck: [false; 2],
                    sprint: false,
                    jump: false,
                    reach: None,
                },
                ahead,
            );
            assert_eq!(velocity, DVec3::ZERO, "{ahead:?}");
            assert_eq!(pacing.pace, None);
            assert_eq!(pacing.balked_s, Some(0.0));
        }
        for ahead in [
            Ahead {
                water_m: 0.5,
                rise_m: 1.4,
                room_m: Some(2.7),
            },
            OPEN,
        ] {
            let mut pacing = Pacing::new(DVec3::X);
            hold(&mut pacing, 1.0, false, 1.0);
            let velocity = pacing.step(
                &horse(),
                UP,
                FRAME,
                Reins {
                    turn: 0.0,
                    throttle: 1.0,
                    struck: [false; 2],
                    sprint: false,
                    jump: false,
                    reach: None,
                },
                ahead,
            );
            assert!(velocity.x > 1.0, "{ahead:?}");
        }
    }

    /// A rise of `slope` per metre over the horse's reach.
    fn slope(slope: f64) -> Ahead {
        Ahead {
            rise_m: slope * horse().ahead_m,
            ..OPEN
        }
    }

    #[test]
    fn a_slope_holds_the_pace_down_and_gives_it_back() {
        let paces = horse();
        let mut pacing = Pacing::new(DVec3::X);
        for _ in 0..3 {
            tap(&mut pacing, 1.0);
        }
        hold(&mut pacing, 0.0, false, 3.0);
        assert_eq!(pacing.going(&paces, false), Some(2));
        // Up a hillside it trots, up a mountainside it walks, and down one
        // as well; the gallop asked for is kept.
        for (rise, pace) in [(0.5, 1), (1.0, 0), (-1.0, 0), (0.3, 2)] {
            hold_over(&mut pacing, 0.0, false, 3.0, slope(rise));
            assert_eq!(pacing.going(&paces, false), Some(pace), "{rise}");
            let speed_mps = paces.paces[pace].speed_mps;
            assert!((pacing.speed_mps - speed_mps).abs() < 1e-9, "{rise}");
            assert_eq!(pacing.pace, Some(2));
            assert_eq!(pacing.balked_s, None);
        }
    }

    #[test]
    fn a_wall_that_stops_it_dead_balks_it_and_one_glanced_off_slows_it() {
        let mut pacing = Pacing::new(DVec3::X);
        tap(&mut pacing, 1.0);
        tap(&mut pacing, 1.0);
        hold(&mut pacing, 0.0, false, 2.0);
        pacing.stopped(pacing.speed_mps * 0.5);
        assert_eq!((pacing.pace, pacing.speed_mps), (Some(1), 1.8));
        pacing.stopped(0.1);
        assert_eq!((pacing.pace, pacing.speed_mps), (None, 0.0));
        assert_eq!(pacing.balked_s, Some(0.0));
        // Held forward, it stays balked; pressed again, it sets off.
        hold(&mut pacing, 1.0, false, 0.5);
        assert_eq!(pacing.balked_s, None);
        let mut pacing = Pacing::new(DVec3::X);
        tap(&mut pacing, 1.0);
        hold(&mut pacing, 0.0, false, 1.0);
        pacing.stopped(0.0);
        hold(&mut pacing, 0.0, false, 0.5);
        assert!((pacing.balked_s.unwrap() - 0.5).abs() < 1e-9);
    }

    /// One step of a frame with these reins on open ground.
    fn rein(pacing: &mut Pacing, turn: f64, throttle: f64, jump: bool) {
        let reins = Reins {
            turn,
            throttle,
            struck: [false; 2],
            sprint: false,
            jump,
            reach: None,
        };
        pacing.step(&horse(), UP, FRAME, reins, OPEN);
    }

    /// Jump held for `held` frames, the press one of them, then let go:
    /// the frames from the press to the one it leaves the ground in, and
    /// the arc it leaves in, falling back as the planet pulls.
    fn jumped(pacing: &mut Pacing, held: u32) -> (u32, Arc) {
        rein(pacing, 0.0, 0.0, true);
        let mut frames = 0;
        loop {
            if let Some(arc) = pacing.gathered(&horse(), 14.0) {
                return (frames, arc);
            }
            rein(pacing, 0.0, 0.0, frames + 1 < held);
            frames += 1;
            assert!(frames < held + 60, "never gathered");
        }
    }

    /// Jump tapped: [`jumped`] with the key down one frame.
    fn gather(pacing: &mut Pacing) -> (u32, Arc) {
        jumped(pacing, 1)
    }

    /// From a halt and from a walk, tapped, it crouches longer than at a
    /// gallop and springs forward, lower and shorter than a gallop's tap:
    /// it goes on as it gathers, then at the jump's forward speed, and
    /// lands back at what it was at. Backing, it does not jump.
    #[test]
    fn from_a_halt_and_a_walk_it_crouches_and_springs_forward() {
        let paces = horse();
        let gallop = paces.paces[2].jump.unwrap();
        let galloping = paces.jump.arc(gallop.height_m, 9.0, 14.0);
        for (pace, speed_mps) in [(None, 0.0), (Some(0), 1.5)] {
            let mut pacing = Pacing::new(DVec3::X);
            (pacing.pace, pacing.speed_mps) = (pace, speed_mps);
            let (frames, arc) = gather(&mut pacing);
            let least = paces.jump_from(pace).unwrap();
            assert!(
                least.gather_s > gallop.gather_s,
                "{pace:?}: a longer crouch"
            );
            assert!((f64::from(frames) * FRAME - least.gather_s).abs() <= FRAME);
            assert!(
                (pacing.speed_mps - speed_mps).abs() < 0.1,
                "it gathers where it goes"
            );
            assert_eq!(arc.height_m, least.height_m, "{pace:?}: its least");
            let rise_mps = pacing.fly(arc);
            assert!(pacing.flies());
            assert!((rise_mps - arc.rise_mps()).abs() < 1e-12);
            assert_eq!(pacing.speed_mps, paces.jump.forward_mps, "springs forward");
            assert!(arc.height_m < gallop.height_m && arc.length_m() < galloping.length_m());
            pacing.land();
            assert_eq!(pacing.pace, pace, "lands back at what it was at");
        }

        let mut pacing = Pacing::new(DVec3::X);
        rein(&mut pacing, 0.0, -1.0, false);
        for _ in 0..30 {
            rein(&mut pacing, 0.0, -1.0, false);
        }
        assert!(pacing.speed_mps < 0.0, "backing");
        rein(&mut pacing, 0.0, -1.0, true);
        assert_eq!(pacing.leaping, None, "no jump backing");
    }

    /// Held, it gathers on at its pace and waits for the key to be let go,
    /// however long: let go, it leaves as high as the hold says, up to the
    /// most; a tap leaves at its least once it has gathered for it.
    #[test]
    fn held_it_gathers_at_its_pace_and_leaves_as_high_as_the_hold() {
        let paces = horse();
        let least = paces.paces[2].jump.unwrap();
        let mut highest = 0.0;
        for held_s in [0.0, 0.2, 0.4, 0.6, 0.8, 1.6] {
            let mut pacing = Pacing::new(DVec3::X);
            (pacing.pace, pacing.speed_mps) = (Some(2), 9.0);
            let held = (held_s / FRAME).round() as u32;
            let (frames, arc) = jumped(&mut pacing, held);
            assert!(arc.height_m >= highest, "{held_s} s: {}", arc.height_m);
            highest = arc.height_m;
            let left_s = f64::from(frames) * FRAME;
            assert!(
                left_s >= least.gather_s.max(held_s) - 1e-9,
                "{held_s} s: {left_s}"
            );
            assert!(left_s <= least.gather_s.max(held_s) + FRAME + 1e-9);
            assert_eq!(pacing.speed_mps, 9.0, "{held_s} s: at its pace");
            assert_eq!(pacing.pace, Some(2));
        }
        assert_eq!(highest, paces.jump.most_m);
        // Held on, it waits gathered.
        let mut pacing = Pacing::new(DVec3::X);
        (pacing.pace, pacing.speed_mps) = (Some(2), 9.0);
        for _ in 0..300 {
            rein(&mut pacing, 0.0, 0.0, true);
            assert_eq!(pacing.gathered(&paces, 14.0), None);
        }
        assert!(matches!(
            pacing.leaping,
            Some(Leaping::Gathering { let_go: None, .. })
        ));
        rein(&mut pacing, 0.0, 0.0, false);
        let arc = pacing.gathered(&paces, 14.0).expect("let go, it leaves");
        assert_eq!(arc.height_m, paces.jump.most_m);
    }

    /// Asked up to a faster pace and still easing to it, it jumps as the
    /// pace its speed has reached when let go does: a walk with sprint
    /// struck, a trot stepped up to the gallop, each takes its own jump; at
    /// the run's speed, the run's.
    #[test]
    fn a_jump_is_the_one_of_the_pace_its_speed_has_reached() {
        let paces = horse();
        let height_of = |pace: usize| paces.paces[pace].jump.unwrap().height_m;
        // Sprint held at the walk, easing up: the walk's jump, not the run's.
        let mut pacing = Pacing::new(DVec3::X);
        (pacing.pace, pacing.speed_mps) = (Some(0), 1.6);
        let reins = Reins {
            turn: 0.0,
            throttle: 1.0,
            struck: [false; 2],
            sprint: true,
            jump: false,
            reach: None,
        };
        pacing.step(&paces, UP, FRAME, reins, OPEN);
        let (_, arc) = gather(&mut pacing);
        assert_eq!(arc.height_m, height_of(0), "the walk's");
        // Stepped up from the trot, not yet galloping: the trot's.
        let mut pacing = Pacing::new(DVec3::X);
        (pacing.pace, pacing.speed_mps) = (Some(2), 4.0);
        let (_, arc) = gather(&mut pacing);
        assert_eq!(arc.height_m, height_of(1), "the trot's");
        // At the run's speed, the run's.
        let mut pacing = Pacing::new(DVec3::X);
        (pacing.pace, pacing.speed_mps) = (Some(3), 11.7);
        let (_, arc) = gather(&mut pacing);
        assert_eq!(arc.height_m, height_of(3), "the run's");
        // Easing down to a halt: a halt's.
        let mut pacing = Pacing::new(DVec3::X);
        (pacing.pace, pacing.speed_mps) = (None, 0.5);
        let (_, arc) = gather(&mut pacing);
        assert_eq!(arc.height_m, paces.standing.unwrap().height_m, "a halt's");
    }

    /// Stepped down to a halt as it gathers a gallop's jump, it springs as
    /// it does from a halt, never a fence from a standstill.
    #[test]
    fn halted_as_it_gathers_it_springs_as_from_a_halt() {
        let paces = horse();
        let mut pacing = Pacing::new(DVec3::X);
        (pacing.pace, pacing.speed_mps) = (Some(2), 9.0);
        rein(&mut pacing, 0.0, 0.0, true);
        pacing.pace = None;
        let (_, arc) = gather(&mut pacing);
        assert_eq!(arc.height_m, paces.standing.unwrap().height_m);
    }

    /// Jump is taken from a pace that jumps; let go and gathered, the arc
    /// rises and falls back over its length at the speed it goes; in the
    /// air it neither turns nor changes pace.
    #[test]
    fn a_jump_is_taken_from_a_pace_that_jumps_and_holds_its_line() {
        let paces = horse();
        let mut pacing = Pacing::new(DVec3::X);
        pacing.pace = Some(2);
        pacing.speed_mps = 9.0;
        let (frames, arc) = gather(&mut pacing);
        let least = paces.paces[2].jump.unwrap();
        assert!((f64::from(frames) * FRAME - least.gather_s).abs() <= FRAME);
        assert_eq!(arc, paces.jump.arc(least.height_m, 9.0, 14.0));
        pacing.fly(arc);

        let heading = pacing.heading;
        for _ in 0..10 {
            rein(&mut pacing, 1.0, 1.0, false);
            rein(&mut pacing, 1.0, 0.0, false);
        }
        assert_eq!(pacing.heading, heading);
        assert_eq!(pacing.pace, Some(2));
        assert_eq!(pacing.speed_mps, 9.0);
        pacing.land();
        assert_eq!(pacing.leaping, None);
    }
}
