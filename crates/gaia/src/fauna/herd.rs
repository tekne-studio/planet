//! Where a herd's animals are at an instant, in closed form.
//!
//! A herd is a function of its seed and the clock, with no state that
//! changes it: every client that asks at the same instant answers the same
//! pose, bit for bit, and a client that missed a minute answers what it
//! would have seen. Nothing is integrated, so nothing drifts.
//!
//! Time is cut in bouts of a length the seed says. In each bout the herd has
//! a centre, a smooth wander around its home, and its animals stand in its
//! formation round that centre: each its own place, the places a body
//! length and a half apart. The formation turns a little from bout to bout
//! and each place is moved a little off it, so a herd never stands in rank.
//! An animal stands where the last bout left it, turns where it stands
//! toward its next place, walks there with ease, and grazes until the bout
//! ends. It goes only where it faces: never sideways, never backward.
//!
//! Each herd has a temperament from its seed, between its kind's calmest
//! and most restless: a calm herd grazes long bouts and walks on; a
//! restless one moves on up to four times as often, up to a trot. Nothing
//! near it changes either: a herd goes its way whoever stands in it.
//!
//! No two animals stand in one another, nor walk into one another: from
//! bout to bout every place moves by the same step and turn, so two
//! animals walking for as long keep apart as their places do, and the one
//! ahead along the step sets off first, so the herd strings out on the way
//! and never closes up.
//!
//! A herd lives in a plane of its own, metres from its home, with nothing
//! of the world in it: [`Land`] says where it may stand. Every place it
//! takes can be walked from home in a line, and a move whose line is
//! refused goes through home, stopping there to turn, so no animal ever
//! crosses what the land refuses. A place the land refuses is taken at the centre, or at home:
//! only there may two animals come near.

use core::f64::consts::{PI, TAU};
use std::collections::HashMap;

use glam::DVec2;

use crate::fauna::ease::smoothstep;
use crate::fauna::kind::{Herding, Kind};
use crate::fauna::random::{Random, mix};

/// Where a herd may stand: whoever places the herd answers.
pub trait Land {
    /// Whether an animal may stand at a point of the herd's plane, metres
    /// from its home.
    fn walkable(&self, at: DVec2) -> bool;
}

/// One animal of a herd at an instant.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Pose {
    /// In the herd's plane, metres from its home.
    pub at: DVec2,
    /// Which way it faces, unit.
    pub heading: DVec2,
    /// Metres a second along its heading, and radians a second it turns,
    /// counterclockwise positive.
    pub speed_mps: f64,
    pub turn_rps: f64,
    /// How far its head is down in the grass, 0 to 1.
    pub graze: f64,
}

/// How far apart a line is read, metres.
const READ_M: f64 = 2.0;
/// The formation's places are at least this many body lengths apart.
const APART: f64 = 1.5;
/// How far a place is moved off the formation in a bout at most, metres.
const JITTER_M: f64 = 0.4;
/// How far the formation turns from its own way at most, radians: two
/// bouts in a row differ by a third of twice that, eased.
const SWING: f64 = 0.7;
/// The most the last of a herd sets off after the first, seconds, for each
/// metre it stands behind it along the herd's step.
const STRING_S_PER_M: f64 = 0.8;
/// Bouts between the herd's wander points: the centre eases from one to
/// the next over this many.
const WANDER_BOUTS: i64 = 3;
/// A move ends at least this long before its bout does, seconds.
const SETTLE_S: f64 = 5.0;
/// Shorter than this, metres, a move is none: the animal stays, facing
/// where it rests. Any longer one it turns to and walks, however short, so
/// it never goes but forward.
const STILL_M: f64 = 1e-6;
/// Grazing, the head lifts once every so many seconds, for this long.
const LOOK_EVERY_S: f64 = 17.0;
const LOOK_S: f64 = 3.0;
/// The head goes down this long after the animal stops.
const HEAD_DOWN_S: f64 = 2.0;
/// How much shorter a restless herd's bouts are than a calm one's: it moves
/// on this many times as often.
const RESTLESS_BOUTS: f64 = 4.0;

/// How far apart a kind's herd grazes, metres: as its herding says, and at
/// least a body length and a half.
fn spacing_m(kind: &Kind) -> f64 {
    kind.herding
        .spread_m
        .max(APART * kind.length_m() + 2.0 * JITTER_M)
}

/// The places of a formation of `size`, round its centre: the points of a
/// square lattice `spacing_m` apart nearest it, the nearest first.
fn formation(spacing_m: f64, size: usize) -> Vec<DVec2> {
    let reach = libm::ceil(libm::sqrt(size as f64)) as i64;
    let mut points: Vec<DVec2> = (-reach..=reach)
        .flat_map(|i| (-reach..=reach).map(move |j| DVec2::new(i as f64, j as f64)))
        .collect();
    points.sort_by(|a, b| {
        let angle = |p: &DVec2| libm::atan2(p.y, p.x);
        a.length_squared()
            .total_cmp(&b.length_squared())
            .then(angle(a).total_cmp(&angle(b)))
    });
    points.truncate(size);
    points.into_iter().map(|p| p * spacing_m).collect()
}

/// A herd: its animals' paths, from its seed.
#[derive(Clone, Debug)]
pub struct Herd {
    seed: u64,
    herding: Herding,
    size: usize,
    /// How restless it is, 0 to 1, and so how fast it walks on, metres a
    /// second.
    restless: f64,
    wander_mps: f64,
    bout_s: f64,
    phase_s: f64,
    /// Each animal's place round the centre, before the bout turns it.
    places: Vec<DVec2>,
    /// The way the formation faces, radians, that each bout swings about.
    facing: f64,
    /// The coat of each animal born with wings, by its place in the herd.
    winged: Vec<(usize, &'static str)>,
    /// Each bout's moves, as asked: a memo, which changes no answer.
    bouts: HashMap<i64, Vec<Route>>,
}

/// One animal's move in one bout.
#[derive(Clone, Copy, Debug)]
struct Route {
    from: DVec2,
    to: DVec2,
    /// Through home, when the straight line is refused.
    through_home: bool,
    length_m: f64,
    /// When it starts, seconds from the bout's start; how long the turn
    /// before it and the walk take.
    start_s: f64,
    turn_s: f64,
    walk_s: f64,
    /// The heading it walks off along and the one it ends along: for a
    /// move too short to walk, the one it rests facing.
    off: f64,
    end: f64,
}

impl Herd {
    /// The herd of a kind a seed makes.
    pub fn new(kind: &Kind, seed: u64) -> Herd {
        let herding = kind.herding;
        let mut random = Random::of(&[seed, 0x6865_7264]);
        let size = random.count(herding.size);
        let calm_s = random.range(herding.bout_s);
        let phase = random.unit();
        let facing = random.unit() * TAU;
        let restless = random.range(herding.restless);
        let bout_s = calm_s / (1.0 + (RESTLESS_BOUTS - 1.0) * restless);
        let wander_mps = herding.wander_mps + (herding.trot_mps - herding.wander_mps) * restless;
        // The winged come from a stream of their own, so the rest of a herd
        // is the one its seed drew before wings were given by herd.
        let mut random = Random::of(&[seed, 0x7069_6773]);
        let winged = kind.wings.map_or(Vec::new(), |winged| {
            let set = random
                .weighted(winged.herds.iter().map(|_| 1))
                .map_or(&[][..], |set| winged.herds[set]);
            let mut places: Vec<usize> = (0..size).collect();
            set.iter()
                .take(size)
                .map(|&coat| {
                    (
                        places.swap_remove(random.count((0, places.len() - 1))),
                        coat,
                    )
                })
                .collect()
        });
        Herd {
            seed,
            herding,
            size,
            restless,
            wander_mps,
            bout_s,
            phase_s: phase * bout_s,
            places: formation(spacing_m(kind), size),
            facing,
            winged,
            bouts: HashMap::new(),
        }
    }

    /// The furthest from its home a herd of a kind stands, metres: what a
    /// client reads its region's herd within.
    pub fn reach_m(kind: &Kind) -> f64 {
        let places = formation(spacing_m(kind), kind.herding.size.1);
        let widest = places.iter().map(|at| at.length()).fold(0.0, f64::max);
        kind.herding.roam_m + widest + JITTER_M
    }

    /// How many animals it has.
    pub fn len(&self) -> usize {
        self.size
    }

    pub fn is_empty(&self) -> bool {
        self.size == 0
    }

    /// How restless it is, 0 calm to 1 restless, from its seed.
    pub fn restless(&self) -> f64 {
        self.restless
    }

    /// The coat an animal is born in with wings, by its place in the herd;
    /// none for one born without.
    pub fn wings_of(&self, animal: usize) -> Option<&'static str> {
        self.winged
            .iter()
            .find_map(|&(place, coat)| (place == animal).then_some(coat))
    }

    /// An animal's own seed: what its look and its identity are drawn
    /// from.
    pub fn seed_of(&self, animal: usize) -> u64 {
        mix(self.seed ^ mix(animal as u64 + 1))
    }

    /// Where an animal of the herd is at `t` seconds of the clock, and what
    /// it does.
    pub fn pose(&mut self, animal: usize, t: f64, land: &impl Land) -> Pose {
        let since = t - self.phase_s;
        let bout = libm::floor(since / self.bout_s) as i64;
        let in_s = since - bout as f64 * self.bout_s;
        // Forget what is past: only this bout and the one before are read.
        if self.bouts.len() > 4 {
            self.bouts.retain(|b, _| *b >= bout - 2);
        }
        let route = self.route(animal, bout, land);
        let before = self.route(animal, bout - 1, land);
        // Still grazing where the last bout left it, since it stopped there,
        // facing as it arrived.
        let stopped_s = before.start_s + before.turn_s + before.walk_s - self.bout_s;
        let was = before.end;
        let standing = |at: DVec2, angle: f64, since_s: f64| Pose {
            at,
            heading: unit(angle),
            speed_mps: 0.0,
            turn_rps: 0.0,
            graze: grazing(since_s, mix(self.seed ^ animal as u64)),
        };
        if in_s < route.start_s {
            return standing(route.from, was, in_s - stopped_s.min(in_s));
        }
        let turned_s = in_s - route.start_s;
        if turned_s < route.turn_s {
            let (along, rate) = eased(turned_s / route.turn_s);
            let turn = wrap(route.off - was);
            return Pose {
                at: route.from,
                heading: unit(was + turn * along),
                speed_mps: 0.0,
                turn_rps: turn * rate / route.turn_s,
                graze: 0.0,
            };
        }
        let walked_s = turned_s - route.turn_s;
        if walked_s < route.walk_s {
            return route.walking(walked_s, self.herding.turn_per_s);
        }
        standing(route.to, route.end, walked_s - route.walk_s)
    }

    /// An animal's move in a bout, from the herd's moves then.
    fn route(&mut self, animal: usize, bout: i64, land: &impl Land) -> Route {
        if !self.bouts.contains_key(&bout) {
            let routes = self.moves(bout, land);
            self.bouts.insert(bout, routes);
        }
        self.bouts[&bout][animal]
    }

    /// The herd's moves in a bout, each animal from the place the bout
    /// before gave it to its own. They walk for as long, the longest move's
    /// time, and set off in order along the herd's step, the first ahead.
    fn moves(&self, bout: i64, land: &impl Land) -> Vec<Route> {
        let (was, from) = self.places_in(bout - 1, land);
        let (centre, to) = self.places_in(bout, land);
        let step = centre - was;
        let mut random = Random::of(&[self.seed, bout as u64, 0x6d6f_7665]);
        // How strung out the herd goes this bout, and how long before the
        // first sets off.
        let string = random.unit();
        let first_s = random.unit() * 2.0;
        let along = step.try_normalize().unwrap_or(DVec2::ZERO);
        let ahead: Vec<f64> = from.iter().map(|at| at.dot(along)).collect();
        let front = ahead.iter().copied().fold(f64::MIN, f64::max);
        let paths: Vec<(bool, f64)> = from
            .iter()
            .zip(&to)
            .map(|(&from, &to)| {
                let through_home = !clear(land, from, to);
                let length_m = match through_home {
                    true => from.length() + to.length(),
                    false => from.distance(to),
                };
                (through_home, length_m)
            })
            .collect();
        let longest_m = paths.iter().map(|path| path.1).fold(0.0, f64::max);
        let turn_s = PI / self.herding.turn_per_s;
        let last_s = first_s
            + STRING_S_PER_M * string * (front - ahead.iter().copied().fold(front, f64::min));
        // Eased in and out, at its peak it walks at its pace, unless the
        // bout is too short for that: then it hurries.
        let room_s = (self.bout_s - last_s - turn_s - SETTLE_S).max(1.0);
        let walk_s = (1.5 * longest_m / self.wander_mps).min(room_s);
        (0..self.size)
            .map(|animal| {
                let (through_home, length_m) = paths[animal];
                let (from, to) = (from[animal], to[animal]);
                let (off, end) = match length_m < STILL_M {
                    true => {
                        let resting = resting_angle(self.seed, animal, bout);
                        (resting, resting)
                    }
                    false => {
                        let (first, last) = match through_home {
                            true => (-from, to),
                            false => (to - from, to - from),
                        };
                        (libm::atan2(first.y, first.x), libm::atan2(last.y, last.x))
                    }
                };
                Route {
                    from,
                    to,
                    through_home,
                    length_m,
                    start_s: first_s + STRING_S_PER_M * string * (front - ahead[animal]),
                    turn_s,
                    walk_s: if length_m < STILL_M { 0.0 } else { walk_s },
                    off,
                    end,
                }
            })
            .collect()
    }

    /// Where the herd stands in a bout: its centre, and each animal at its
    /// place round it, the formation swung and each place moved a little.
    /// Where the land does not take the whole formation, the centre comes
    /// toward home until it does; where even home's does not, a place the
    /// land refuses, or cannot be walked to from home in a line, comes in
    /// along that line to the furthest point that is taken, home at last:
    /// no two stand in one another while the land leaves them room.
    fn places_in(&self, bout: i64, land: &impl Land) -> (DVec2, Vec<DVec2>) {
        let wander = self.centre(bout);
        let turned = self.facing + self.swing(bout);
        let (cos, sin) = (libm::cos(turned), libm::sin(turned));
        let takes = |at: DVec2| land.walkable(at) && clear(land, DVec2::ZERO, at);
        let around = |centre: DVec2| -> Vec<DVec2> {
            self.places
                .iter()
                .enumerate()
                .map(|(animal, place)| {
                    let mut random =
                        Random::of(&[self.seed, animal as u64, bout as u64, 0x7370_6f74]);
                    let swung =
                        DVec2::new(cos * place.x - sin * place.y, sin * place.x + cos * place.y);
                    centre + swung + disc(&mut random) * JITTER_M
                })
                .collect()
        };
        for toward in [1.0, 0.75, 0.5, 0.25, 0.0] {
            let centre = wander * toward;
            let places = around(centre);
            if places.iter().all(|&at| takes(at)) {
                return (centre, places);
            }
        }
        let inward = |at: DVec2| {
            let steps = libm::ceil(at.length() / READ_M) as usize;
            let along = (1..=steps)
                .rev()
                .map(|step| at * step as f64 / steps as f64);
            along
                .into_iter()
                .find(|&at| takes(at))
                .unwrap_or(DVec2::ZERO)
        };
        let places = around(DVec2::ZERO).into_iter().map(inward).collect();
        (DVec2::ZERO, places)
    }

    /// How far the formation is swung off its own way in a bout, radians:
    /// eased between wander points as the centre is.
    fn swing(&self, bout: i64) -> f64 {
        self.wandered(bout, 0x7377_696e, |random| {
            (2.0 * random.unit() - 1.0) * SWING
        })
    }

    /// The herd's centre in a bout: wander points within its roam of home,
    /// eased from one to the next.
    fn centre(&self, bout: i64) -> DVec2 {
        let roam_m = self.herding.roam_m;
        let x = self.wandered(bout, 0x6365_6e74, |random| disc(random).x * roam_m);
        let y = self.wandered(bout, 0x6365_6e74, |random| disc(random).y * roam_m);
        DVec2::new(x, y)
    }

    /// A value drawn at each wander point, by `salt`, and eased from one to
    /// the next over the bouts between.
    fn wandered(&self, bout: i64, salt: u64, draw: impl Fn(&mut Random) -> f64) -> f64 {
        let step = bout.div_euclid(WANDER_BOUTS);
        let s = bout.rem_euclid(WANDER_BOUTS) as f64 / WANDER_BOUTS as f64;
        let point = |step: i64| draw(&mut Random::of(&[self.seed, step as u64, salt]));
        let (a, b) = (point(step), point(step + 1));
        a + (b - a) * smoothstep(0.0, 1.0, s)
    }
}

impl Route {
    /// Where it is `walked_s` into its walk. In a line, eased from end to
    /// end. Through home, it walks there, stops, turns where it stands to
    /// the way on, and walks on, each leg eased: it goes only where it
    /// faces.
    fn walking(&self, walked_s: f64, turn_per_s: f64) -> Pose {
        let leg = |from: DVec2, to: DVec2, angle: f64, s: f64, leg_s: f64| {
            let (along, rate) = eased(s);
            Pose {
                at: from.lerp(to, along),
                heading: unit(angle),
                speed_mps: from.distance(to) * rate / leg_s,
                turn_rps: 0.0,
                graze: 0.0,
            }
        };
        if !self.through_home {
            return leg(
                self.from,
                self.to,
                self.off,
                walked_s / self.walk_s,
                self.walk_s,
            );
        }
        let turn = wrap(self.end - self.off);
        let turn_s = (turn.abs() / turn_per_s).min(0.4 * self.walk_s);
        let legs_s = self.walk_s - turn_s;
        let first_s = legs_s * self.from.length() / self.length_m.max(1e-9);
        let second_s = legs_s - first_s;
        if walked_s < first_s {
            return leg(
                self.from,
                DVec2::ZERO,
                self.off,
                walked_s / first_s,
                first_s,
            );
        }
        let turning_s = walked_s - first_s;
        if turning_s < turn_s {
            let (along, rate) = eased(turning_s / turn_s);
            return Pose {
                at: DVec2::ZERO,
                heading: unit(self.off + turn * along),
                speed_mps: 0.0,
                turn_rps: turn * rate / turn_s,
                graze: 0.0,
            };
        }
        let on_s = turning_s - turn_s;
        leg(
            DVec2::ZERO,
            self.to,
            self.end,
            on_s / second_s.max(1e-9),
            second_s.max(1e-9),
        )
    }
}

/// How far along an eased move is at `s` of its time, 0 to 1, and how fast
/// it goes there, in its length per its time.
fn eased(s: f64) -> (f64, f64) {
    let s = s.clamp(0.0, 1.0);
    (s * s * (3.0 - 2.0 * s), 6.0 * s * (1.0 - s))
}

/// Whether the land takes a straight line, read every [`READ_M`].
fn clear(land: &impl Land, from: DVec2, to: DVec2) -> bool {
    let steps = libm::ceil(from.distance(to) / READ_M).max(1.0) as usize;
    (0..=steps).all(|step| land.walkable(from.lerp(to, step as f64 / steps as f64)))
}

/// A point in the unit disc, as many in each part of it.
fn disc(random: &mut Random) -> DVec2 {
    let angle = TAU * random.unit();
    let radius = libm::sqrt(random.unit());
    DVec2::new(libm::cos(angle), libm::sin(angle)) * radius
}

/// Where an animal faces resting in a bout it did not move in.
fn resting_angle(seed: u64, animal: usize, bout: i64) -> f64 {
    TAU * Random::of(&[seed, animal as u64, bout as u64, 0x7265_7374]).unit()
}

fn unit(angle: f64) -> DVec2 {
    DVec2::new(libm::cos(angle), libm::sin(angle))
}

/// An angle brought within a half turn either way.
fn wrap(angle: f64) -> f64 {
    let wrapped = (angle + PI).rem_euclid(TAU) - PI;
    if wrapped <= -PI {
        wrapped + TAU
    } else {
        wrapped
    }
}

/// How far down the head is, `since_s` seconds after it stopped: down after
/// a moment, lifted now and then to look round.
pub fn grazing(since_s: f64, seed: u64) -> f64 {
    let down = smoothstep(0.0, HEAD_DOWN_S, since_s);
    let offset = (seed >> 11) as f64 / (1u64 << 53) as f64 * LOOK_EVERY_S;
    let cycle = (since_s + offset).rem_euclid(LOOK_EVERY_S);
    let look = smoothstep(0.0, 0.6, cycle) * (1.0 - smoothstep(LOOK_S - 0.6, LOOK_S, cycle));
    down * (1.0 - look)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fauna::horse::HORSE;

    struct Meadow;

    impl Land for Meadow {
        fn walkable(&self, _: DVec2) -> bool {
            true
        }
    }

    /// A lake east of home: everything over `x = 20` is water.
    struct Lake;

    impl Land for Lake {
        fn walkable(&self, at: DVec2) -> bool {
            at.x < 20.0
        }
    }

    /// A fence east of home along `y = 0`: a move across it goes through
    /// home.
    struct Fence;

    impl Land for Fence {
        fn walkable(&self, at: DVec2) -> bool {
            !(at.x > 1.0 && at.y.abs() < 0.6)
        }
    }

    /// A herd strings out as it moves: along the way it goes, the one
    /// furthest ahead sets off first and each behind it later, by its
    /// distance behind; over enough bouts the last leaves seconds after the
    /// first.
    #[test]
    fn the_first_ahead_sets_off_first_and_the_last_after() {
        let herd = Herd::new(&HORSE, 42);
        let mut widest_s = 0.0f64;
        for bout in 1..60 {
            let (was, _) = herd.places_in(bout - 1, &Meadow);
            let (centre, _) = herd.places_in(bout, &Meadow);
            let Some(along) = (centre - was).try_normalize() else {
                continue;
            };
            let routes = herd.moves(bout, &Meadow);
            let ahead = |route: &Route| route.from.dot(along);
            for a in &routes {
                for b in &routes {
                    if ahead(a) > ahead(b) {
                        assert!(a.start_s <= b.start_s, "bout {bout}: behind set off first");
                    }
                }
            }
            let starts = routes.iter().map(|route| route.start_s);
            let (first, last) =
                starts.fold((f64::MAX, f64::MIN), |(lo, hi), s| (lo.min(s), hi.max(s)));
            widest_s = widest_s.max(last - first);
        }
        assert!(widest_s > 2.0, "never strung out: {widest_s} s");
    }

    /// Every herd has a pegasus: a white one, a black one, or one of each,
    /// each about as often, at distinct places in the herd.
    #[test]
    fn every_herd_has_its_winged() {
        let mut sets = [0usize; 3];
        for seed in 0..3000 {
            let herd = Herd::new(&HORSE, seed);
            let mut coats: Vec<&str> = (0..herd.len()).filter_map(|a| herd.wings_of(a)).collect();
            coats.sort_unstable();
            let set = match coats[..] {
                ["white"] => 0,
                ["black"] => 1,
                ["black", "white"] => 2,
                _ => panic!("herd {seed}: {coats:?}"),
            };
            sets[set] += 1;
        }
        for count in sets {
            assert!((900..1100).contains(&count), "{sets:?}");
        }
    }

    /// Asked at the same instant, two herds of one seed answer the same
    /// pose, whatever each was asked before.
    #[test]
    fn the_same_instant_is_the_same_pose() {
        let mut a = Herd::new(&HORSE, 42);
        let mut b = Herd::new(&HORSE, 42);
        // One is stepped by frames for an hour, the other asked cold.
        let start = 1.7e9;
        let mut t = start;
        while t < start + 3600.0 {
            t += 1.0 / 60.0 + 0.004 * libm::sin(t);
            for animal in 0..a.len() {
                let _ = a.pose(animal, t, &Meadow);
            }
        }
        for animal in 0..b.len() {
            assert_eq!(a.pose(animal, t, &Meadow), b.pose(animal, t, &Meadow));
        }
    }

    /// The path is continuous: nowhere does an animal jump or spin.
    #[test]
    fn nobody_jumps() {
        for seed in 0..6 {
            let mut herd = Herd::new(&HORSE, seed);
            let dt = 0.05;
            for animal in 0..herd.len() {
                let mut last = herd.pose(animal, 0.0, &Lake);
                let mut t = 0.0;
                while t < 1500.0 {
                    t += dt;
                    let pose = herd.pose(animal, t, &Lake);
                    let moved = pose.at.distance(last.at);
                    assert!(
                        moved <= dt * 4.0 + 1e-9,
                        "seed {seed}, animal {animal} jumped {moved} m at {t}"
                    );
                    assert!(pose.at.x < 20.0, "in the lake at {t}: {:?}", pose.at);
                    last = pose;
                }
            }
        }
    }

    /// An animal goes only where it faces, never sideways nor backward: it
    /// turns where it stands, then walks; it never swings round faster than
    /// its kind turns, and never goes faster than its herd's pace, at a
    /// walk or a trot, over an hour of the clock, every frame.
    #[test]
    fn it_turns_then_walks_forward() {
        let most_rps = 1.5 * HORSE.herding.turn_per_s;
        let mut through_home = 0;
        for seed in 0..8 {
            let mut herd = Herd::new(&HORSE, seed);
            let most_mps = 1.5 * herd.wander_mps;
            let dt = 1.0 / 30.0;
            for bout in 0..200 {
                let moves = herd.moves(bout, &Fence);
                through_home += moves.iter().filter(|route| route.through_home).count();
            }
            for animal in 0..herd.len() {
                let mut last = herd.pose(animal, 0.0, &Fence);
                let mut t = 0.0;
                while t < 3600.0 {
                    t += dt;
                    let pose = herd.pose(animal, t, &Fence);
                    let step = pose.at - last.at;
                    let swung = libm::acos(pose.heading.dot(last.heading).clamp(-1.0, 1.0));
                    assert!(
                        swung <= most_rps * dt + 1e-6,
                        "seed {seed}, animal {animal} swung {swung} rad in a frame at {t}"
                    );
                    if step.length() > 1e-6 {
                        let along = step.normalize().dot(pose.heading);
                        assert!(
                            along > 0.99,
                            "seed {seed}, animal {animal} went {along} along its heading at {t}"
                        );
                    }
                    assert!(
                        step.length() <= most_mps * dt + 1e-9,
                        "seed {seed}, animal {animal} went {} m/s at {t}",
                        step.length() / dt
                    );
                    last = pose;
                }
            }
        }
        assert!(through_home > 0, "no move went through home");
    }

    /// The herd keeps together: its animals graze within a few spreads of
    /// one another.
    #[test]
    fn the_herd_keeps_together() {
        let mut herd = Herd::new(&HORSE, 9);
        for minute in 0..120 {
            let t = f64::from(minute) * 60.0;
            let at: Vec<DVec2> = (0..herd.len())
                .map(|animal| herd.pose(animal, t, &Meadow).at)
                .collect();
            for a in &at {
                for b in &at {
                    let apart = a.distance(*b);
                    assert!(apart < 6.0 * HORSE.herding.spread_m, "{apart} m apart");
                }
            }
        }
    }

    /// No two animals of a herd come nearer than a body length, standing or
    /// on the way, over a day of the clock read every quarter second, on
    /// open land and by a lake, for every seed tried.
    #[test]
    fn no_two_stand_in_one_another() {
        fn check(land: &impl Land, seed: u64) -> f64 {
            let mut herd = Herd::new(&HORSE, seed);
            let mut nearest_m = f64::INFINITY;
            let mut t = 1.7e9;
            while t < 1.7e9 + 86_400.0 {
                let at: Vec<DVec2> = (0..herd.len())
                    .map(|animal| herd.pose(animal, t, land).at)
                    .collect();
                for (i, a) in at.iter().enumerate() {
                    for b in &at[i + 1..] {
                        nearest_m = nearest_m.min(a.distance(*b));
                    }
                }
                t += 0.25;
            }
            nearest_m
        }
        let length_m = HORSE.length_m();
        for seed in 0..8 {
            for nearest_m in [check(&Meadow, seed), check(&Lake, seed)] {
                assert!(
                    nearest_m >= length_m,
                    "seed {seed}: {nearest_m} m, a body is {length_m} m"
                );
            }
        }
    }
}
