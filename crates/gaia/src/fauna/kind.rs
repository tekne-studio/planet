//! A kind of animal, as a table.

use glam::{DQuat, DVec3};

use crate::fauna::gait::Gaits;
use crate::fauna::look::{Build, Coat, Marks};
use crate::fauna::rig::Rig;
use crate::fauna::wing::Wing;

/// A kind of animal: everything that makes one, as data.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Kind {
    /// Its name, as a carrier and a prompt say it.
    pub name: &'static str,
    /// Its number, as a carrier says it: one per kind, never reused.
    pub number: u8,
    pub rig: Rig,
    pub gaits: Gaits,
    pub coats: &'static [Coat],
    pub builds: &'static [Build],
    pub marks: Marks,
    pub herding: Herding,
    /// How it carries a rider, when it does.
    pub carry: Option<Carry>,
    /// The wings some of its kind are born with, one or two a herd, when
    /// any are.
    pub wings: Option<Winged>,
    /// How far one's voice may sit from its kind's, cents either way, as
    /// its seed draws it: over what its size already moves.
    pub voice_cents: (f64, f64),
}

impl Kind {
    /// How far one reaches ahead of where it stands, to its nose, and
    /// behind it, to its rump, metres at size 1: as it carries its head at
    /// its slowest gait.
    pub fn reach_m(&self) -> [f64; 2] {
        let rig = &self.rig;
        let gait = self.gaits.0[0];
        let rump = rig
            .trunk
            .iter()
            .map(|section| section.to_z)
            .fold(0.0, f64::max);
        let poll = rig.neck.root
            + DQuat::from_rotation_x(gait.neck) * DVec3::new(0.0, 0.0, -rig.neck.length_m);
        let head = DQuat::from_rotation_x(gait.head);
        let corners = rig.head.iter().flat_map(|part| {
            (0..8).map(move |corner| {
                let sign = |bit: u32| if corner & (1 << bit) == 0 { -0.5 } else { 0.5 };
                part.centre + part.size * DVec3::new(sign(0), sign(1), sign(2))
            })
        });
        let nose = corners
            .map(|corner| -(poll + head * corner).z)
            .fold(0.0, f64::max);
        [nose, rump]
    }

    /// How long one is from the nose of its outstretched head to its rump,
    /// metres, at the largest build it is born in.
    pub fn length_m(&self) -> f64 {
        let rig = &self.rig;
        let rump = rig
            .trunk
            .iter()
            .map(|section| section.to_z)
            .fold(f64::MIN, f64::max);
        let chest = rig
            .trunk
            .iter()
            .map(|section| section.from_z)
            .fold(f64::MAX, f64::min);
        let nose = rig
            .head
            .iter()
            .map(|part| -part.centre.z + part.size.z / 2.0)
            .fold(0.0, f64::max);
        // The neck's root is behind the chest: what of it reaches past.
        let neck = (rig.neck.length_m - (rig.neck.root.z - chest)).max(0.0);
        let largest = self
            .builds
            .iter()
            .map(|build| build.size.1)
            .fold(1.0, f64::max);
        (rump - chest + neck + nose) * largest
    }
}

/// Ground an animal lives on.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Habitat {
    Grass,
    Forest,
    Sand,
    Snow,
    Rock,
}

/// How a kind's herds live: where, how many, how they keep together.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Herding {
    /// The ground it lives on, and the steepest it stands on, a rise per
    /// metre.
    pub lives_on: &'static [Habitat],
    pub steep: f64,
    /// The side of a region that may hold one herd, metres, and how likely
    /// one region is to.
    pub region_m: f64,
    pub chance: f64,
    /// How many in a herd.
    pub size: (usize, usize),
    /// How far apart the animals of a herd graze, metres, and how far from
    /// its home the herd wanders.
    pub spread_m: f64,
    pub roam_m: f64,
    /// How long a bout is, seconds: a herd moves on once a bout, grazes the
    /// rest of it.
    pub bout_s: (f64, f64),
    /// How fast it walks when it moves on, metres a second, and how fast it
    /// turns where it stands, radians a second.
    pub wander_mps: f64,
    pub turn_per_s: f64,
    /// How restless its herds are, 0 calm to 1 restless, each herd's drawn
    /// between these from its seed: a calm herd grazes long bouts and walks
    /// on at `wander_mps`; a restless one moves on four times as often, at
    /// up to `trot_mps`. Read too by what is done with one, a tamer's ring.
    pub restless: (f64, f64),
    pub trot_mps: f64,
}

/// How a kind carries a rider.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Carry {
    /// Where the rider sits, the rig's space at size 1: on the back, over
    /// the withers.
    pub saddle: DVec3,
    /// Its paces under a rider, slowest first.
    pub paces: &'static [Pace],
    /// The fastest of them a rider steps it up to; those past it only
    /// sprint held rides.
    pub stepped: usize,
    /// Which of them sprint held rides.
    pub sprint: usize,
    /// How fast one goes for its size, as a share of `paces`.
    pub by_size: BySize,
    /// How fast it gains speed and loses it, metres a second squared.
    pub accel_mps2: f64,
    pub decel_mps2: f64,
    /// The deepest water it wades, metres.
    pub wade_m: f64,
    /// How tall it stands with a rider, metres: the room it needs under a
    /// roof.
    pub height_m: f64,
    /// How long it is held to before a body mounts it, seconds: a moment
    /// for a calm one, longer for a restless one.
    pub mount_s: f64,
    /// How long it stands where it is left, grazing, seconds, while a body
    /// stays within `wait_m` metres of it; then it walks back to its herd.
    pub wait_s: f64,
    pub wait_m: f64,
    /// The least jump it takes from a halt, springing forward; none, it
    /// jumps only from a pace that does.
    pub standing: Option<Least>,
    /// How it jumps, from a halt or any pace: how high at most and how
    /// long the key is held for that.
    pub jump: Jump,
    /// How it steps backward under a rider; none, it does not.
    pub backing: Option<Backing>,
}

/// The wings some of a kind are born with, and how one with them flies
/// under a rider.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Winged {
    /// The winged a herd may have, as coats by name: each herd draws one
    /// set from its seed, equally, and has one winged animal in each coat
    /// of it. A winged one is its coat's hair all over, no mark and no
    /// blot; no wingless one is born in it, as `plain` says.
    pub herds: &'static [&'static [&'static str]],
    /// How light a wingless one's hair is, the least and the most, as
    /// [`Coat::lightness`] says: a coat outside it is the winged's alone,
    /// so a coat that light or that dark seen in a herd means wings.
    pub plain: (f32, f32),
    pub wing: &'static Wing,
    pub flight: Flight,
}

/// How a kind with wings flies under a rider.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Flight {
    /// How fast it flies, metres a second, near the ground, and how much
    /// faster for each metre over it: a body on its own flies 12 and 0.8.
    pub speed_mps: f64,
    pub per_m: f64,
    /// How many times faster sprint held flies it.
    pub sprint: f64,
    /// The fastest it flies, metres a second, however high and with sprint
    /// held: what the herds and the ground drawn about it keep up with.
    pub top_mps: f64,
    /// The highest it flies, metres over the sea, wherever the ground
    /// under it is.
    pub ceiling_m: f64,
    /// How high it climbs as it takes off, metres over the ground, and the
    /// room it needs over its head for that.
    pub takeoff_m: f64,
    /// How it leaves the ground for it.
    pub spring: Spring,
    /// How fast it comes down to land, metres a second, near the ground,
    /// and how much faster for each metre over it.
    pub land_mps: f64,
    pub land_per_m: f64,
}

/// How a kind with wings leaves the ground under a rider: from the press it
/// gathers on its legs for `gather_s`, then springs off them `height_m`
/// up, slowing to its top in `rise_s`, carried forward as fast as it went
/// and at least `forward_mps`; at the top its wings open and take it. Its
/// body is drawn by these times from when its wings were asked for, so
/// every client that sees them asked sees it alike.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Spring {
    pub gather_s: f64,
    pub rise_s: f64,
    pub height_m: f64,
    pub forward_mps: f64,
}

impl Spring {
    /// Seconds from the press to the spring's top, where the wings open.
    pub fn top_s(&self) -> f64 {
        self.gather_s + self.rise_s
    }
}

/// How a kind steps backward under a rider, from a halt: it sets off at
/// `from_mps` and gains `gain_mps2` while held, up to `top_mps`, metres a
/// second; it turns at `turn_per_s`, radians a second.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Backing {
    pub from_mps: f64,
    pub top_mps: f64,
    pub gain_mps2: f64,
    pub turn_per_s: f64,
}

/// How fast one of a kind goes for its size, as a share of its paces:
/// `shares.0` at `sizes.0` and smaller, `shares.1` at `sizes.1` and larger,
/// on a line between. Read from the size alone, so a pet keeps its speed
/// with the size it was tamed at.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct BySize {
    pub sizes: (f64, f64),
    pub shares: (f64, f64),
}

impl BySize {
    /// The share of its paces one of `size` goes at.
    pub fn share(&self, size: f64) -> f64 {
        let (small, large) = self.sizes;
        let along = ((size - small) / (large - small)).clamp(0.0, 1.0);
        self.shares.0 + (self.shares.1 - self.shares.0) * along
    }
}

/// A pace a kind goes at under a rider.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Pace {
    /// Metres a second, and how fast it turns at it, radians a second.
    pub speed_mps: f64,
    pub turn_per_s: f64,
    /// The steepest ground it keeps this pace on, a rise per metre: steeper,
    /// it goes at a slower pace that keeps it. None for one it keeps on any
    /// ground a walker climbs.
    pub steep: Option<f64>,
    /// The least jump it takes from this pace; none, it does not jump
    /// from it.
    pub jump: Option<Least>,
}

/// How a kind jumps under a rider. From the press it gathers on its
/// haunches, the deeper the longer the key is held, and leaves the ground
/// as the key is let go: from a tap, in its pace's least jump; held, higher
/// as it gathers deeper, up to `most_m` its hooves rise once held for
/// `hold_s` seconds from the press. Held past that, it waits gathered for
/// the key to be let go. In the air it is carried along the ground as fast
/// as it went and at least `forward_mps`, so a jump from a halt or a walk
/// springs forward: how long it is aloft is the world's to say, by how
/// high it goes, and how far is that time at that speed.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Jump {
    pub most_m: f64,
    pub hold_s: f64,
    pub forward_mps: f64,
}

/// The least jump a kind takes from a pace or a halt, at a tap of the key:
/// how long it gathers itself before it leaves the ground, seconds, and
/// how high its hooves rise, metres.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Least {
    pub gather_s: f64,
    pub height_m: f64,
}

#[cfg(test)]
mod tests {
    use crate::fauna::horse::HORSE;

    /// The smallest horse born goes at four fifths of its paces and the
    /// largest at six fifths, on a line between; a pet outside the sizes
    /// born keeps to the ends.
    #[test]
    fn a_horse_goes_as_fast_as_its_size_says() {
        let carry = HORSE.carry.unwrap();
        let born = HORSE
            .builds
            .iter()
            .fold((f64::MAX, f64::MIN), |(least, most), build| {
                (least.min(build.size.0), most.max(build.size.1))
            });
        assert_eq!(carry.by_size.sizes, born, "the line spans the sizes born");
        let share = |size| carry.by_size.share(size);
        assert_eq!((share(born.0), share(born.1)), (0.8, 1.2));
        assert_eq!((share(0.5), share(1.4)), (0.8, 1.2));
        assert!((share(1.0) * carry.paces[3].speed_mps - 12.5).abs() < 0.1);
        assert!(share(0.9) < share(1.0) && share(1.0) < share(1.1));
    }

    /// A horse reaches past its chest to its nose, its head carried up, and
    /// to its rump behind: less than it is long with its head outstretched.
    #[test]
    fn a_horse_reaches_to_its_nose_and_its_rump() {
        let [ahead_m, behind_m] = HORSE.reach_m();
        let chest_m = -HORSE.rig.trunk[0].from_z;
        assert!(ahead_m > chest_m + 0.5, "{ahead_m}");
        assert!((behind_m - 0.84).abs() < 1e-9, "{behind_m}");
        assert!(
            ahead_m + behind_m < HORSE.length_m(),
            "{ahead_m} {behind_m}"
        );
    }

    /// A rider steps a horse up to the gallop; the run is sprint's alone,
    /// and backing never reaches the walk.
    #[test]
    fn the_run_is_sprints_and_backing_stays_under_the_walk() {
        let carry = HORSE.carry.unwrap();
        assert_eq!(carry.paces.len(), carry.sprint + 1);
        assert_eq!(carry.stepped + 1, carry.sprint);
        let backing = carry.backing.unwrap();
        assert!(backing.from_mps < backing.top_mps);
        assert!(backing.top_mps < carry.paces[0].speed_mps);
    }
}
