//! The animals of a world with life: the kinds of [`fauna::KINDS`], their
//! herds placed on the planet and drawn near the body, and the animals
//! bodies ride. In a world without life no animal runs.
//!
//! Herds are the world's, the same in every eye: whether a region holds one
//! and where its home is follow from the world's seed and the ground, and
//! where each animal stands at an instant follows from its herd's seed and
//! the world's clock ([`fauna::Herd`]). Nothing of a free herd is sent.
//! Only the regions within reach of the view are placed and posed, on
//! either side of a sector's seam.
//!
//! An animal is known by its number, which names its kind, its herd's
//! region and its place in the herd: any client finds its herd from it. A
//! stance carries the number of the animal its body rides
//! (`Stance.carrier`). Every client draws an animal under each rider, keyed
//! by who rides: two bodies that claim one animal ride two, until one
//! leaves, and a peer's claim never moves this body's saddle. Stepped down
//! from, the animal stands where it was left and grazes while a body stays
//! near, for as long as its kind waits ([`fauna::Carry`]), then walks back
//! to where its herd has it, drawn all the way, however far that is. Who
//! saw it ridden sees it so, wherever its herd is; a client that comes
//! later knows nothing of it, and draws it with its herd.
//!
//! A pet is the world's to keep: its herd leaves it out, and it stands where
//! its keeper left it, grazing, or runs to where it was called, gathering
//! speed to its gallop and slowing to stand there ([`fauna::Run`]), in
//! closed form from what the world told, so every client draws it at one
//! place. Within the reach of the view, as a herd.
//!
//! Herds stand on the generator's ground alone, so every client agrees
//! whatever it has loaded of what is built; hooves step on what is built
//! too, as a walker's feet do. No animal moves for a body: a herd goes its
//! way whoever is near, and a pet stays where it was kept. What is not the
//! herd's is each client's own: one left walking home is stepped frame by
//! frame here, and two clients may draw it a few metres apart while it
//! does.

use std::cell::RefCell;
use std::collections::{BTreeSet, HashMap};

use gaia::fauna::{
    self, Animal, Beat, Footfall, Footing, Going, Ground, Habitat, Herd, Kind, Land, Leap, Look,
    REGION_BITS, Region, Traits,
};
use glam::{DAffine3, DQuat, DVec2, DVec3};
use seat::Seat;
use topology::{BLOCK_M, Grid, Sector, SurfacePoint};
use worldgen::{Generator, Material};

use crate::cells::Cells;
use crate::collision::STEP_M;
use crate::controller::Built;
use crate::pets::{Pet, Spot};
use crate::way::{Step, Straddle};

/// How far from the body an animal is drawn, metres.
const VIEW_M: f64 = 150.0;
/// The most animals drawn at once. One under a rider or walking home from
/// a ride is always drawn; the pets, nearest first, fill what the ridden
/// leave, and the free ones, nearest first, what all of those leave.
const DRAWN: usize = 24;
/// How many free animals that travel are heard at once, the nearest.
const HEARD: usize = 3;
/// The side of a cell of the land a herd reads, metres: whether an animal
/// may stand is said once a cell.
const LAND_M: f64 = 2.0;
/// How far apart the ground is read for its slope, metres.
const SLOPE_M: f64 = 1.0;
/// The lowest ground over the sea an animal stands on, metres.
const SHORE_M: f64 = 0.5;
/// How many cells of land are kept before they are read again.
const LAND_KEPT: usize = 1 << 16;
/// How far a herd's home moves off what is built over it, metres: from the
/// middle of a plot past its corner and the cell of land beside it.
const HOME_MOVES_M: f64 = 48.0;
/// How far a free animal its herd moves at once, as what is built turns
/// its place away, trots there, metres: further, it is set anew there.
const CATCH_M: f64 = 30.0;
/// How much faster than its herd's trot it trots there: it catches a herd
/// that moves on meanwhile.
const CATCH_PACE: f64 = 1.3;
/// How far over a hoof a built floor is still stepped onto, metres: as a
/// walker steps up a block.
const STEP_UP_M: f64 = 0.6;
/// What a footfall on something built strikes: one past every material.
pub const BUILT: u8 = u8::MAX;

/// A pet on the ground as the world's record has it at an instant.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Afoot {
    /// On the ground, world space.
    pub at: DVec3,
    /// Along the ground, unit: toward where it runs, or the way it faces.
    pub heading: DVec3,
    /// How fast it goes, metres a second.
    pub speed_mps: f64,
    /// Whether it is still running to where it was called.
    pub on_its_way: bool,
}

/// An animal drawn this frame, as a plugin or an agent reads it.
#[derive(Clone, Copy, Debug)]
pub struct Near {
    pub number: u64,
    pub kind: &'static Kind,
    /// The ground under its middle, world space.
    pub at: DVec3,
    /// Which way it faces, unit.
    pub heading: DVec3,
    /// Whether a body rides it.
    pub ridden: bool,
    /// Whose it is.
    pub keeping: Keeping,
    /// How restless its herd is, 0 calm to 1 restless.
    pub restless: f64,
    /// How fast it goes under a rider, as a share of its kind's paces: by
    /// its size ([`gaia::fauna::Animal::speed_share`]).
    pub speed_share: f64,
    /// How big it is, its kind's own at 1: how far its body reaches.
    pub size: f64,
}

/// Whose an animal is.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Keeping {
    /// Its herd's.
    Wild,
    /// This client's own pet.
    Mine,
    /// Another's pet.
    Anothers,
}

/// A body riding an animal, this frame: who rides, the animal's number and
/// how the body goes, its feet, its facing and its speed.
pub(crate) struct Rider {
    /// By session; `None` is this client's body.
    pub who: Option<u32>,
    pub number: u64,
    pub at: DVec3,
    pub facing: DVec3,
    pub speed_mps: f64,
    /// Seconds since the animal balked under it; none when it has not.
    pub balk_s: Option<f64>,
    /// Whether it is off the ground, as the body knows; none for a peer's,
    /// whose height over the ground says.
    pub aloft: Option<bool>,
    /// Whether it is in a jump, gathering on the ground or in the air: the
    /// body says so for its own, a peer's stance by the gait it jumps in.
    pub leaps: bool,
    /// Whether it flies on its wings: the body says so for its own, a
    /// peer's stance by the gait it flies in.
    pub wings: bool,
}

/// A peer's body this far over the ground under it is in the air, metres.
const ALOFT_M: f64 = 0.2;

/// The bodies about the animals this frame: those riding one, and those
/// on foot, world space.
#[derive(Clone, Copy)]
pub(crate) struct Bodies<'a> {
    pub riders: &'a [Rider],
    pub on_foot: &'a [DVec3],
}

impl Bodies<'_> {
    /// Where each body is, riding or on foot.
    fn all(self) -> impl Iterator<Item = DVec3> {
        let riders = self.riders.iter().map(|rider| rider.at);
        riders.chain(self.on_foot.iter().copied())
    }
}

/// A herd placed in its region: its home, in blocks of its sector, and its
/// path.
struct Placed {
    kind: &'static Kind,
    sector: Sector,
    home: DVec2,
    herd: Herd,
}

/// Whether each cell of land holds a kind, read once: by the kind's
/// number, the sector and the cell.
type Lands = RefCell<HashMap<(u8, Sector, i64, i64), bool>>;

/// The plots of the planet whose volume holds a cell, as the cells last
/// said: no fauna is in them.
type Occupied = BTreeSet<(Sector, [i32; 2])>;

/// One animal's body as it is drawn.
struct Body {
    number: u64,
    kind: &'static Kind,
    /// Its own seed: what its grazing is drawn from, and its look unless
    /// a pet's record keeps it.
    seed: u64,
    /// How restless its herd is, 0 to 1.
    restless: f64,
    animal: Animal,
    /// The frame it was last stepped in.
    stepped: u64,
    /// The way its rider faced the last frame it stepped, while ridden.
    facing: Option<DVec3>,
    /// Where it is against where its herd has it.
    away: Away,
    /// What a rider sits astride on it: its look is its body's for life.
    straddle: Straddle,
    /// Seconds it has gathered itself to jump under its rider, from the
    /// step its rider's jump began; none unless it gathers.
    gather_s: Option<f64>,
}

/// Where an unridden animal is against where its herd has it.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Away {
    /// Where its herd has it.
    Home,
    /// Stepped down from at `since_s` on the world's clock: it stands there
    /// and grazes, facing as it was ridden.
    Left { since_s: f64, heading: DVec3 },
    /// Walking back to where its herd has it.
    Returning,
    /// Trotting to where its herd has it now: what was built turned the
    /// place it was at away.
    Catching,
    /// A pet, where the world keeps it.
    Kept,
}

impl Body {
    /// The animal at `index` of its herd, in the traits a pet's record
    /// keeps, else as its herd draws it now.
    fn new(
        kind: &'static Kind,
        number: u64,
        herd: &Herd,
        index: usize,
        kept: Option<Traits>,
    ) -> Body {
        let seed = herd.seed_of(index);
        let look = match kept {
            Some(traits) => Look::of(kind, traits),
            None => Look::new(kind, seed, herd.wings_of(index)),
        };
        let animal = Animal::new(kind, look);
        let steps = (animal.flanks().into_iter())
            .map(|flank| Step {
                top_m: flank.top_m as f32,
                bottom_m: flank.bottom_m as f32,
                out_m: flank.out_m as f32,
            })
            .collect();
        Body {
            number,
            kind,
            seed,
            restless: herd.restless(),
            animal,
            stepped: 0,
            facing: None,
            away: Away::Home,
            straddle: Straddle { steps },
            gather_s: None,
        }
    }

    fn near(&self, ridden: bool, pets: &HashMap<u64, Pet>) -> Near {
        Near {
            number: self.number,
            kind: self.kind,
            at: self.animal.position(),
            heading: self.animal.heading(),
            ridden,
            keeping: match pets.get(&self.number) {
                None => Keeping::Wild,
                Some(pet) if pet.mine => Keeping::Mine,
                Some(_) => Keeping::Anothers,
            },
            restless: self.restless,
            speed_share: self.animal.speed_share(),
            size: self.animal.traits().size,
        }
    }
}

/// Every animal of the world this client draws.
#[derive(Default)]
pub(crate) struct Animals {
    /// Each region within reach of the view or holding an animal drawn,
    /// with its herd, or none where it holds none.
    herds: HashMap<Region, Option<Placed>>,
    /// Each animal under a rider, by who rides it: `None` is this client's
    /// body, a peer is its session.
    ridden: HashMap<Option<u32>, Body>,
    /// Each animal out of its herd and unridden, by its number: stepped
    /// down from, waiting or walking home.
    left: HashMap<u64, Body>,
    /// Each free animal drawn where its herd has it, by its number.
    free: HashMap<u64, Body>,
    /// Every pet of the world, by its number, as the world told it.
    pets: HashMap<u64, Pet>,
    /// The animal this client's body stepped down from this frame: its
    /// number, where it stands and which way it faces.
    stepped_off: Option<(u64, DVec3, DVec3)>,
    seen: Vec<Near>,
    /// The beats heard this frame, each with whether a body rode it.
    struck: Vec<(Footfall, bool)>,
    /// What each free animal stepped this frame did: of those that travel,
    /// only the nearest [`HEARD`] are heard.
    ambled: Vec<Ambled>,
    /// Where a ridden animal's wings struck a downstroke this frame.
    beats: Vec<DVec3>,
    /// The wings of a ridden animal held out to glide this frame, by who
    /// rides it.
    glides: Vec<(Option<u32>, fauna::Glide)>,
    /// Where an animal ridden sprang into a jump this frame, by who rides
    /// it, and whether to take off.
    sprang: Vec<(Option<u32>, DVec3, bool)>,
    /// How an animal ridden came down from a jump, a fall or a flight this
    /// frame, by who rides it.
    landed: Vec<(Option<u32>, fauna::Landing)>,
    land: Lands,
    /// What is built that no fauna is in, and the revision of the cells
    /// it was read at.
    occupied: Occupied,
    occupied_at: Option<u64>,
    /// Whether the herds were placed again this frame, for what is built.
    replaced: bool,
    frame: u64,
    /// A pet out of its herd as [`Animals::find`] last found it, by its
    /// record: where it stands holds until the world tells it otherwise.
    found: RefCell<Option<(Pet, Near)>>,
}

impl Animals {
    /// A new world, or one without life: no herd of the last one is kept.
    pub(crate) fn clear(&mut self) {
        self.herds.clear();
        self.ridden.clear();
        self.left.clear();
        self.free.clear();
        self.stepped_off = None;
        self.seen.clear();
        self.struck.clear();
        self.ambled.clear();
        self.beats.clear();
        self.glides.clear();
        self.sprang.clear();
        self.landed.clear();
        self.land.get_mut().clear();
        self.occupied.clear();
        self.occupied_at = None;
        self.found.get_mut().take();
    }

    /// Every pet of the world, as the world told it: those no longer told
    /// walk back to their herds, and each wears the traits its record
    /// keeps. A record this client cannot place, a spot that is no place,
    /// is no pet: an old row the world kept before it refused such spots
    /// draws its animal with its herd.
    pub(crate) fn set_pets<'a>(
        &mut self,
        pets: impl Iterator<Item = &'a Pet>,
        generator: &Generator,
    ) {
        let placed = |pet: &&Pet| {
            spot_at(&pet.spot, generator).is_some()
                && pet
                    .from
                    .as_ref()
                    .is_none_or(|from| spot_at(from, generator).is_some())
        };
        self.pets = pets
            .filter(placed)
            .map(|pet| (pet.number, pet.clone()))
            .collect();
        // A pet wears the traits its record keeps, when an animal can be
        // drawn in them: a body drawn otherwise is drawn again, where it
        // stands.
        let pets = &self.pets;
        let worn = |body: &Body| {
            let kept = pets.get(&body.number).and_then(|pet| worn(pet, body.kind));
            kept.is_none_or(|traits| body.animal.look.traits == *traits)
        };
        self.free.retain(|_, body| worn(body));
        self.left.retain(|_, body| worn(body));
        self.ridden.retain(|_, body| worn(body));
    }

    /// The animal this client's body stepped down from this frame: its
    /// number, where it stands and which way it faces.
    pub(crate) fn stepped_off(&self) -> Option<(u64, DVec3, DVec3)> {
        self.stepped_off
    }

    /// The kind of the animal a number names.
    pub(crate) fn kind_of(number: u64) -> Option<&'static Kind> {
        Region::of(number).and_then(|(region, _)| region.kind())
    }

    /// An animal at `now_s`: where it is drawn near, else, a pet, where the
    /// world keeps it, standing or on its way to where it was called,
    /// unridden. None for one that is neither.
    pub(crate) fn find(&self, number: u64, now_s: f64, generator: &Generator) -> Option<Near> {
        if let Some(near) = self.seen.iter().find(|near| near.number == number) {
            return Some(*near);
        }
        let pet = self.pets.get(&number)?;
        if let Some((_, near)) = self.found.borrow().as_ref().filter(|(was, _)| was == pet) {
            return Some(*near);
        }
        let region = Region::of(number)?.0;
        let kind = region.kind()?;
        // Its herd's temperament where the herd is placed here; else its
        // kind's middle, as no herd is placed to find one pet.
        let (calm, restless) = kind.herding.restless;
        let restless = match self.herds.get(&region) {
            Some(Some(placed)) => placed.herd.restless(),
            _ => (calm + restless) / 2.0,
        };
        // The seed draws only how it grazes, not where it stands. A pet that
        // is nowhere, on its way from afar, is found where it will arrive.
        let (at, heading) = match kept(pet, kind, 0, now_s, generator) {
            Some(going) => (going.position, going.heading),
            None => {
                let (up, heading) = spot_at(&pet.spot, generator)?;
                let ground_m = generator.sample(up.to_array()).height_m.max(0.0);
                (up * (generator.sphere().radius_m() + ground_m), heading)
            }
        };
        let near = Near {
            number,
            kind,
            at,
            heading,
            ridden: false,
            keeping: if pet.mine {
                Keeping::Mine
            } else {
                Keeping::Anothers
            },
            restless,
            speed_share: match (kind.carry, &pet.traits) {
                (Some(carry), Some(traits)) => carry.by_size.share(traits.size),
                _ => 1.0,
            },
            size: pet.traits.as_ref().map_or(1.0, |traits| traits.size),
        };
        *self.found.borrow_mut() = Some((pet.clone(), near));
        Some(near)
    }

    /// Where a pet is at `now_s` by what the world said, as every client
    /// draws it: none while it is out of sight, called from afar, or for
    /// no pet of this number.
    pub(crate) fn afoot(&self, number: u64, now_s: f64, generator: &Generator) -> Option<Afoot> {
        let pet = self.pets.get(&number)?;
        let kind = Self::kind_of(number)?;
        // The seed draws only how it grazes.
        let (going, on_its_way) = kept_run(pet, kind, 0, now_s, generator)?;
        Some(Afoot {
            at: going.position,
            heading: going.heading,
            speed_mps: going.speed_mps,
            on_its_way,
        })
    }

    /// The fastest a pet of this client's is called at: its gallop, at its
    /// size's share, as a rider has it go.
    pub(crate) fn called_mps(&self, number: u64) -> Option<f64> {
        let pet = self.pets.get(&number)?;
        Some(called(pet, Self::kind_of(number)?).top_mps)
    }

    /// Every animal drawn this frame.
    pub(crate) fn near(&self) -> &[Near] {
        &self.seen
    }

    /// The beats heard this frame: where, on what, a material's number or
    /// [`BUILT`], how hard, and whether a body rode the animal.
    pub(crate) fn struck(&self) -> &[(Footfall, bool)] {
        &self.struck
    }

    /// Where the wings of an animal ridden struck a downstroke this frame.
    pub(crate) fn wingbeats(&self) -> &[DVec3] {
        &self.beats
    }

    /// Where each animal ridden sprang into a jump this frame, by who rides
    /// it: `None` for this client's body, a peer by its session; and
    /// whether to take off, which every client draws alike from its rider's
    /// gait.
    pub(crate) fn sprang(&self) -> &[(Option<u32>, DVec3, bool)] {
        &self.sprang
    }

    /// How each animal ridden came down from a jump, a fall or a flight
    /// this frame, by who rides it, as [`Animals::sprang`].
    pub(crate) fn landed(&self) -> &[(Option<u32>, fauna::Landing)] {
        &self.landed
    }

    /// The animal under a rider: what a sound of its body is made from. By
    /// who rides, as [`Animals::winged`].
    pub(crate) fn under(&self, who: Option<u32>) -> Option<&fauna::Animal> {
        self.ridden.get(&who).map(|body| &body.animal)
    }

    /// The wings of each animal ridden held out to glide this frame, by
    /// who rides it: `None` for this client's body, a peer by its session.
    pub(crate) fn glides(&self) -> &[(Option<u32>, fauna::Glide)] {
        &self.glides
    }

    /// Whether the animal under a rider was born with wings. By who rides:
    /// `None` for this client's body, a peer by its session.
    pub(crate) fn winged(&self, who: Option<u32>) -> bool {
        self.ridden
            .get(&who)
            .is_some_and(|body| body.animal.wings().is_some())
    }

    /// What a rider sits astride on the animal under it: its side under
    /// the saddle. By who rides, as [`Animals::winged`].
    pub(crate) fn straddle(&self, who: Option<u32>) -> Option<&Straddle> {
        self.ridden.get(&who).map(|body| &body.straddle)
    }

    /// How far the animal under a rider banks on its wings, radians: see
    /// [`gaia::fauna::Animal::bank`]. By who rides, as [`Animals::winged`].
    pub(crate) fn bank(&self, who: Option<u32>) -> f64 {
        self.ridden.get(&who).map_or(0.0, |body| body.animal.bank())
    }

    /// The boxes of every animal drawn this frame, appended to `out`.
    pub(crate) fn boxes(&self, out: &mut Vec<scene::BoxPart>) {
        let drawn = self.ridden.values().chain(self.left.values());
        for body in drawn.chain(self.free.values()) {
            body.animal.boxes(out);
        }
    }

    /// Where a body sits on the animal it rides this frame: the saddle,
    /// world space, its columns right, up and back. By who rides: `None`
    /// for this client's body, a peer by its session.
    pub(crate) fn saddle(&self, who: Option<u32>) -> Option<DAffine3> {
        self.ridden.get(&who)?.animal.saddle()
    }

    /// A frame: the animals ridden step under their riders, those left wait
    /// or walk home, and the nearest of the free herds about `near`, on the
    /// planet, step where their herds have them at `now_s` on the world's
    /// clock, whoever is near. Every other animal is let go, and every herd
    /// out of reach.
    pub(crate) fn update(
        &mut self,
        dt: f64,
        now_s: f64,
        near: Option<SurfacePoint>,
        bodies: Bodies<'_>,
        generator: &Generator,
        built: Built<'_>,
    ) {
        self.frame += 1;
        self.stepped_off = None;
        self.seen.clear();
        self.struck.clear();
        self.ambled.clear();
        self.beats.clear();
        self.glides.clear();
        self.sprang.clear();
        self.landed.clear();
        if self.land.get_mut().len() > LAND_KEPT {
            self.land.get_mut().clear();
        }
        self.occupy(built.cells);
        let ground = Hooves { generator, built };
        self.ride(dt, now_s, bodies.riders, generator, &ground);
        self.keep_pets(dt, now_s, near, generator, &ground);
        self.walk_home(dt, now_s, bodies, generator, &ground);
        // Free herds wait on the volumes near: first placed, they already
        // know what is built.
        let near = near.filter(|_| built.cells.come());
        let in_view = near.map_or_else(Vec::new, |near| Self::in_view(near, generator));
        self.roam(dt, now_s, &in_view, near, generator, &ground);
        self.hear(near, generator, &ground);
        let held: Vec<Region> = (self.ridden.values().chain(self.left.values()))
            .filter_map(|body| Some(Region::of(body.number)?.0))
            .collect();
        self.herds
            .retain(|region, _| in_view.contains(region) || held.contains(region));
    }

    /// Reads what is built that no fauna is in, when the cells changed.
    /// Every herd is placed again from it when it differs: a herd is a
    /// function of its seed, the clock and the land, so every client that
    /// holds the same volumes places it alike, from the same frame on.
    fn occupy(&mut self, cells: &Cells) {
        self.replaced = false;
        if self.occupied_at == Some(cells.revision()) {
            return;
        }
        self.occupied_at = Some(cells.revision());
        let occupied: Occupied = cells.occupied().collect();
        self.replaced = occupied != self.occupied;
        if self.replaced {
            self.occupied = occupied;
            self.herds.clear();
        }
    }

    /// Of the free animals that travel, the nearest [`HEARD`] to `near`
    /// are heard; a herd that grazes is not.
    fn hear(&mut self, near: Option<SurfacePoint>, generator: &Generator, ground: &Hooves<'_>) {
        let Some(near) = near else {
            return;
        };
        let here = DVec3::from(generator.sphere().blocks().direction(near));
        let mut ambled = std::mem::take(&mut self.ambled);
        ambled.retain(|ambled| ambled.travels);
        ambled.sort_by(|a, b| {
            let off = |at: DVec3| at.normalize_or_zero().angle_between(here);
            off(a.at).total_cmp(&off(b.at))
        });
        let beats = ambled.iter().take(HEARD).filter_map(|ambled| ambled.beat);
        self.struck
            .extend(beats.map(|beat| (beat.fall(ground), false)));
        ambled.clear();
        self.ambled = ambled;
    }

    /// The animals ridden step under their riders: each rider keeps the
    /// animal it rode, else takes the one left or free of that number, else
    /// a new one of it. One no longer ridden is left where it stands.
    fn ride(
        &mut self,
        dt: f64,
        now_s: f64,
        riders: &[Rider],
        generator: &Generator,
        ground: &Hooves<'_>,
    ) {
        let mut was = std::mem::take(&mut self.ridden);
        for rider in riders {
            let kept = was
                .get(&rider.who)
                .is_some_and(|body| body.number == rider.number)
                .then(|| was.remove(&rider.who))
                .flatten();
            let body = kept
                .or_else(|| self.left.remove(&rider.number))
                .or_else(|| self.free.remove(&rider.number))
                .or_else(|| self.body(rider.number, generator));
            let Some(mut body) = body else {
                continue;
            };
            carry(&mut body, dt, rider, ground, &mut self.struck);
            self.beats.extend(body.animal.wingbeat());
            let glide = body.animal.glide();
            self.glides.extend(glide.map(|glide| (rider.who, glide)));
            let sprang = body.animal.sprang();
            self.sprang
                .extend(sprang.map(|at| (rider.who, at, rider.wings)));
            let landed = body.animal.landed();
            self.landed
                .extend(landed.map(|landing| (rider.who, landing)));
            body.stepped = self.frame;
            body.away = Away::Home;
            self.seen.push(body.near(true, &self.pets));
            self.ridden.insert(rider.who, body);
        }
        for (who, body) in was {
            if riders.iter().any(|rider| rider.number == body.number) {
                continue;
            }
            let heading = body.facing.unwrap_or_else(|| body.animal.heading());
            if who.is_none() {
                self.stepped_off = Some((body.number, body.animal.position(), heading));
            }
            let left = Body {
                facing: None,
                away: Away::Left {
                    since_s: now_s,
                    heading,
                },
                ..body
            };
            self.left.insert(left.number, left);
        }
    }

    /// Each pet unridden within [`VIEW_M`] of `near` stands where the world
    /// keeps it, or runs there when called; one farther is let go. Stepped
    /// down from a moment before the world says where it was left, it
    /// stands where it is until then.
    fn keep_pets(
        &mut self,
        dt: f64,
        now_s: f64,
        near: Option<SurfacePoint>,
        generator: &Generator,
        ground: &Hooves<'_>,
    ) {
        let grid = generator.sphere().blocks();
        let radius_m = generator.sphere().radius_m();
        let here = near.map(|near| DVec3::from(grid.direction(near)));
        let mut pets: Vec<(f64, u64)> = Vec::new();
        for pet in self.pets.values() {
            let ridden = self.ridden.values().any(|body| body.number == pet.number);
            let at = spot_at(&pet.spot, generator).map(|(up, _)| up);
            let far_m = here.zip(at).map_or(f64::INFINITY, |(here, at)| {
                here.angle_between(at) * radius_m
            });
            if !ridden {
                pets.push((far_m, pet.number));
            }
        }
        pets.sort_by(|a, b| a.0.total_cmp(&b.0));
        for (far_m, number) in pets {
            // A pet's run in sight is at most earshot: one that far past
            // the view stays out of it.
            if far_m > VIEW_M + fauna::HEARS_M || self.seen.len() >= DRAWN {
                self.left.remove(&number);
                continue;
            }
            let had = self
                .left
                .remove(&number)
                .or_else(|| self.free.remove(&number));
            // Where it goes is known before its body: one past the view
            // between frames is never drawn, only to be dropped.
            let (kind, seed) = match &had {
                Some(body) => (body.kind, body.seed),
                None => match self.seeded(number, generator) {
                    Some(seeded) => seeded,
                    None => continue,
                },
            };
            let Some(pet) = self.pets.get(&number) else {
                continue;
            };
            let told_s = pet.spot.at_ms as f64 / 1000.0;
            let (going, away) = match had.as_ref().map(|body| body.away) {
                Some(Away::Left { since_s, heading }) if told_s < since_s => {
                    let at = had.as_ref().map(|body| body.animal.position());
                    let going = Going {
                        graze: fauna::grazing((now_s - since_s).max(0.0), seed),
                        ..Going::standing(at.unwrap_or_default(), heading)
                    };
                    (going, Away::Left { since_s, heading })
                }
                _ => match kept(pet, kind, seed, now_s, generator) {
                    Some(going) => (going, Away::Kept),
                    None => continue,
                },
            };
            let drawn = here.is_some_and(|here| {
                here.angle_between(going.position.normalize_or(DVec3::Z)) * radius_m <= VIEW_M
            });
            if !drawn {
                continue;
            }
            let Some(mut body) = had.or_else(|| self.body(number, generator)) else {
                continue;
            };
            body.away = away;
            amble(&mut body, going, dt, ground, &mut self.ambled);
            body.stepped = self.frame;
            self.seen.push(body.near(false, &self.pets));
            self.left.insert(number, body);
        }
    }

    /// Each animal left waits where it was left while a body is near, as
    /// long as its kind waits, then walks back to where its herd has it; at
    /// home, it is its herd's again. A pet freed whose herd no longer holds
    /// its place has no home there: it grazes where it was left, and is
    /// gone once no body is within view of it.
    fn walk_home(
        &mut self,
        dt: f64,
        now_s: f64,
        bodies: Bodies<'_>,
        generator: &Generator,
        ground: &Hooves<'_>,
    ) {
        let numbers: Vec<u64> = self.left.keys().copied().collect();
        for number in numbers {
            if self.pets.contains_key(&number) {
                continue;
            }
            let home = self.herded(number, now_s, generator);
            let Some(mut body) = self.left.remove(&number) else {
                continue;
            };
            let from = body.animal.position();
            let Some(home) = home else {
                if !bodies.all().any(|at| (at - from).length() <= VIEW_M) {
                    continue;
                }
                let (since_s, heading) = match body.away {
                    Away::Left { since_s, heading } => (since_s, heading),
                    _ => (now_s, body.animal.heading()),
                };
                body.away = Away::Left { since_s, heading };
                let going = Going {
                    graze: fauna::grazing((now_s - since_s).max(0.0), body.seed),
                    ..Going::standing(from, heading)
                };
                amble(&mut body, going, dt, ground, &mut self.ambled);
                body.stepped = self.frame;
                self.seen.push(body.near(false, &self.pets));
                self.left.insert(number, body);
                continue;
            };
            let mut going = home;
            // A pet freed goes home at once.
            if body.away == Away::Kept {
                body.away = Away::Returning;
            }
            if let Away::Left { since_s, heading } = body.away {
                let waited_s = now_s - since_s;
                let carry = body.kind.carry;
                let (wait_s, wait_m) = carry.map_or((0.0, 0.0), |c| (c.wait_s, c.wait_m));
                let watched = bodies.all().any(|at| (at - from).length() <= wait_m);
                if waited_s < wait_s && watched {
                    going = Going {
                        graze: fauna::grazing(waited_s.max(0.0), body.seed),
                        ..Going::standing(from, heading)
                    };
                } else {
                    body.away = Away::Returning;
                }
            }
            if body.away == Away::Returning {
                let walk_mps = body.kind.herding.wander_mps;
                // Home within a step, its herd draws it from here, so it
                // never leaps there.
                let Some(step) = toward(from, home.position, walk_mps, dt, ground) else {
                    body.away = Away::Home;
                    self.free.insert(number, body);
                    continue;
                };
                going = step;
            }
            amble(&mut body, going, dt, ground, &mut self.ambled);
            body.stepped = self.frame;
            self.seen.push(body.near(false, &self.pets));
            self.left.insert(number, body);
        }
    }

    /// The free animals step where their herds have them: the nearest
    /// within [`VIEW_M`] of `near`, of the herds of the regions `in_view`,
    /// none ridden or left, as many as the animals drawn already leave of
    /// [`DRAWN`].
    fn roam(
        &mut self,
        dt: f64,
        now_s: f64,
        in_view: &[Region],
        near: Option<SurfacePoint>,
        generator: &Generator,
        ground: &Hooves<'_>,
    ) {
        let grid = generator.sphere().blocks();
        let radius_m = generator.sphere().radius_m();
        for &region in in_view {
            if !self.herds.contains_key(&region) {
                let placed = self.place(region, generator);
                self.herds.insert(region, placed);
            }
        }
        let mut found = Vec::new();
        if let Some(near) = near {
            // How far along the ground, whatever the heights: an animal on
            // a hill is as near as one in a valley.
            let here = DVec3::from(grid.direction(near));
            let Self {
                herds,
                land,
                occupied,
                ridden,
                left,
                pets,
                ..
            } = self;
            for region in in_view {
                let Some(Some(placed)) = herds.get_mut(region) else {
                    continue;
                };
                let plane = placed.plane(generator, land, occupied);
                for index in 0..placed.herd.len() {
                    let number = region.number(index);
                    let taken = left.contains_key(&number)
                        || pets.contains_key(&number)
                        || ridden.values().any(|body| body.number == number);
                    if taken {
                        continue;
                    }
                    let pose = placed.herd.pose(index, now_s, &plane);
                    let direction = DVec3::from(grid.direction(plane.point(pose.at)));
                    let far_m = direction.angle_between(here) * radius_m;
                    if far_m <= VIEW_M {
                        found.push((far_m, *region, index, pose));
                    }
                }
            }
        }
        found.sort_by(|a, b| a.0.total_cmp(&b.0));
        found.truncate(DRAWN.saturating_sub(self.seen.len()));
        for (_, region, index, pose) in found {
            let Some(Some(placed)) = self.herds.get(&region) else {
                continue;
            };
            let going = placed
                .plane(generator, &self.land, &self.occupied)
                .going(pose, radius_m);
            let number = region.number(index);
            let mut body = self
                .free
                .remove(&number)
                .unwrap_or_else(|| Body::new(placed.kind, number, &placed.herd, index, None));
            let going = catch(&mut body, going, dt, ground, self.replaced);
            amble(&mut body, going, dt, ground, &mut self.ambled);
            body.stepped = self.frame;
            self.seen.push(body.near(false, &self.pets));
            self.free.insert(number, body);
        }
        let frame = self.frame;
        self.free.retain(|_, body| body.stepped == frame);
    }

    /// The regions whose herds may come within view of `near`, of every
    /// kind: those of its sector, and over a seam within reach, those of
    /// the sector across it.
    fn in_view(near: SurfacePoint, generator: &Generator) -> Vec<Region> {
        let grid = generator.sphere().blocks();
        let side = f64::from(grid.side());
        let mut regions = Vec::new();
        for kind in fauna::KINDS {
            let region_b = kind.herding.region_m / BLOCK_M;
            let reach_b = (VIEW_M + Herd::reach_m(kind)) / BLOCK_M;
            let last = libm::ceil(side / region_b) as i64 - 1;
            let span = |at: f64| {
                let from = libm::floor((at - reach_b) / region_b) as i64;
                from..=libm::floor((at + reach_b) / region_b) as i64
            };
            for i in span(near.u) {
                for j in span(near.v) {
                    let middle = |n: i64| (n as f64 + 0.5) * region_b;
                    let point = grid.wrapped(SurfacePoint::new(near.sector, middle(i), middle(j)));
                    let cell = |at: f64| (libm::floor(at / region_b) as i64).clamp(0, last);
                    let region = Region {
                        kind: kind.number,
                        sector: point.sector,
                        i: cell(point.u),
                        j: cell(point.v),
                    };
                    if !regions.contains(&region) {
                        regions.push(region);
                    }
                }
            }
        }
        regions
    }

    /// A body for the animal a number names: a pet's in the traits its
    /// record keeps, at any place of its herd, even one its herd no longer
    /// holds; another as its herd draws it. None for a number whose region
    /// holds no such animal.
    fn body(&mut self, number: u64, generator: &Generator) -> Option<Body> {
        let (region, index) = Region::of(number)?;
        let pet = self.pets.get(&number);
        let (is_pet, kept) = (pet.is_some(), pet.and_then(|pet| worn(pet, region.kind()?)));
        let kept = kept.cloned();
        let placed = self.placed(region, generator)?;
        (is_pet || index < placed.herd.len())
            .then(|| Body::new(placed.kind, number, &placed.herd, index, kept))
    }

    /// The kind of the animal a number names and its seed, as its body
    /// would have them, without drawing one.
    fn seeded(&mut self, number: u64, generator: &Generator) -> Option<(&'static Kind, u64)> {
        let (region, index) = Region::of(number)?;
        let placed = self.placed(region, generator)?;
        Some((placed.kind, placed.herd.seed_of(index)))
    }

    /// Where the herd of the animal a number names has it at `now_s`.
    fn herded(&mut self, number: u64, now_s: f64, generator: &Generator) -> Option<Going> {
        let (region, index) = Region::of(number)?;
        self.placed(region, generator)?;
        let radius_m = generator.sphere().radius_m();
        let Some(Some(placed)) = self.herds.get_mut(&region) else {
            return None;
        };
        if index >= placed.herd.len() {
            return None;
        }
        let plane = placed.plane(generator, &self.land, &self.occupied);
        let pose = placed.herd.pose(index, now_s, &plane);
        Some(plane.going(pose, radius_m))
    }

    /// The herd of a region, placed once while it is kept.
    fn placed(&mut self, region: Region, generator: &Generator) -> Option<&Placed> {
        if !self.herds.contains_key(&region) {
            let placed = self.place(region, generator);
            self.herds.insert(region, placed);
        }
        self.herds.get(&region)?.as_ref()
    }

    /// Whether a region holds a herd of a kind, by the world's seed, and
    /// where its home is: the first of a few spots in it an animal may stand
    /// on. None for a region past its sector's side.
    fn place(&self, region: Region, generator: &Generator) -> Option<Placed> {
        let kind = region.kind()?;
        let herding = kind.herding;
        let grid = generator.sphere().blocks();
        let region_b = herding.region_m / BLOCK_M;
        // Past what a number holds, no region has a herd.
        let regions = (libm::ceil(f64::from(grid.side()) / region_b) as i64).min(1 << REGION_BITS);
        if !(0..regions).contains(&region.i) || !(0..regions).contains(&region.j) {
            return None;
        }
        let (herd, homes) = region.herd(generator.recipe().seed)?;
        let home = homes.into_iter().find_map(|share| {
            let spot = (DVec2::new(region.i as f64, region.j as f64) + share) * region_b;
            let plane = Plane {
                generator,
                grid,
                kind,
                sector: region.sector,
                home: spot,
                land: &self.land,
                occupied: &self.occupied,
            };
            plane.home().map(|at| spot + at / BLOCK_M)
        })?;
        Some(Placed {
            kind,
            sector: region.sector,
            home,
            herd,
        })
    }
}

impl Placed {
    fn plane<'a>(
        &self,
        generator: &'a Generator,
        land: &'a Lands,
        occupied: &'a Occupied,
    ) -> Plane<'a> {
        Plane {
            generator,
            grid: generator.sphere().blocks(),
            kind: self.kind,
            sector: self.sector,
            home: self.home,
            land,
            occupied,
        }
    }
}

/// An animal steps under its rider: where the rider is, facing as it
/// faces, at its speed, backward when it went against its facing, in the
/// air when it is off the ground.
fn carry(
    body: &mut Body,
    dt: f64,
    rider: &Rider,
    ground: &Hooves<'_>,
    struck: &mut Vec<(Footfall, bool)>,
) {
    let up = rider.at.normalize_or(DVec3::Z);
    let turn_rps = match body.facing {
        Some(was) if dt > 0.0 => {
            libm::atan2(was.cross(rider.facing).dot(up), was.dot(rider.facing)) / dt
        }
        _ => 0.0,
    };
    let aloft = rider.aloft.unwrap_or_else(|| {
        let under = ground.under(rider.at);
        (rider.at - under.point).dot(under.up) > ALOFT_M
    });
    // It gathers from the step its rider's jump began, as deep as the time
    // says: every eye that saw the jump begin draws the same gathering.
    body.gather_s = match rider.leaps && !aloft && !rider.wings {
        true => Some(body.gather_s.map_or(0.0, |s| s + dt)),
        false => None,
    };
    let gathered = (body.gather_s.zip(body.kind.carry))
        .map(|(s, carry)| crate::paces::Jump::from(carry.jump).gathered(s));
    let leap = match (aloft, gathered) {
        _ if rider.wings => None,
        (true, _) => Some(Leap::Flying),
        (false, gathered) => gathered.map(Leap::Gathering),
    };
    // Gone against its facing, it steps backward: a peer's says so only by
    // where it went.
    let back = !rider.wings
        && body.animal.is_set()
        && (rider.at - body.animal.position()).dot(rider.facing) < 0.0;
    let going = Going {
        position: rider.at,
        heading: rider.facing,
        speed_mps: match back {
            true => -rider.speed_mps,
            false => rider.speed_mps,
        },
        turn_rps,
        graze: 0.0,
        balk_s: rider.balk_s,
        leap,
        wings: rider.wings,
    };
    let beat = body.animal.step(dt, going, ground);
    struck.extend(beat.map(|beat| (beat.fall(ground), true)));
    body.facing = Some(rider.facing);
}

/// A free animal drawn last frame that its herd moved further than a step
/// at once, `replaced` for what is built, trots there within [`CATCH_M`].
/// Within a step of it, or past `CATCH_M`, it is where its herd has it, as
/// it is after any other leap: a clock jumped draws it where it is now.
fn catch(body: &mut Body, going: Going, dt: f64, ground: &Hooves<'_>, replaced: bool) -> Going {
    let from = body.animal.position();
    let far_m = (going.position - from).length();
    let caught = !body.animal.is_set() || far_m > CATCH_M;
    let sets_off = replaced && body.animal.leaps(dt, &going);
    if caught || (body.away != Away::Catching && !sets_off) {
        body.away = Away::Home;
        return going;
    }
    let trot_mps = body.kind.herding.trot_mps * CATCH_PACE;
    match toward(from, going.position, trot_mps, dt, ground) {
        Some(step) => {
            body.away = Away::Catching;
            step
        }
        None => {
            body.away = Away::Home;
            going
        }
    }
}

/// A step at `mps` from `from` toward `to`, along the ground and facing
/// that way; `None` within a step of it.
fn toward(from: DVec3, to: DVec3, mps: f64, dt: f64, ground: &Hooves<'_>) -> Option<Going> {
    let up = from.normalize_or(DVec3::Z);
    let toward = to - from;
    let along = toward - up * toward.dot(up);
    if along.length() <= mps * dt {
        return None;
    }
    let heading = along.normalize();
    Some(Going {
        position: ground.under(from + heading * mps * dt).point,
        heading,
        speed_mps: mps,
        ..Going::standing(from, heading)
    })
}

/// What a free animal did this frame, for whether it is heard.
struct Ambled {
    at: DVec3,
    travels: bool,
    beat: Option<Beat>,
}

/// An unridden animal steps where it goes.
fn amble(body: &mut Body, going: Going, dt: f64, ground: &Hooves<'_>, ambled: &mut Vec<Ambled>) {
    let beat = body.animal.step(dt, going, ground);
    ambled.push(Ambled {
        at: body.animal.position(),
        travels: body.animal.travels(),
        beat,
    });
}

/// A herd's plane on the planet: metres from its home along its sector's
/// address axes, a block being [`BLOCK_M`] as at the sector's middle.
struct Plane<'a> {
    generator: &'a Generator,
    grid: Grid,
    kind: &'static Kind,
    sector: Sector,
    /// In blocks of the sector.
    home: DVec2,
    land: &'a Lands,
    occupied: &'a Occupied,
}

impl Plane<'_> {
    fn point(&self, at: DVec2) -> SurfacePoint {
        let blocks = self.home + at / BLOCK_M;
        SurfacePoint::new(self.sector, blocks.x, blocks.y)
    }

    /// Where an animal of the herd goes in the world, standing on the
    /// generator's ground.
    fn going(&self, pose: fauna::Pose, radius_m: f64) -> Going {
        let point = self.point(pose.at);
        let tangents = self.grid.tangents(point);
        let up = DVec3::from(tangents.up);
        let (du, dv) = (DVec3::from(tangents.du), DVec3::from(tangents.dv));
        let heading = (du * pose.heading.x + dv * pose.heading.y).normalize_or(du.normalize());
        // The plane turns counterclockwise from `u` to `v`: about up, or
        // about down, as the sector's axes go.
        let turns = du.cross(dv).dot(up).signum();
        let height_m = self.generator.sample(tangents.up).height_m;
        Going {
            position: up * (radius_m + height_m.max(0.0)),
            heading,
            speed_mps: pose.speed_mps,
            turn_rps: pose.turn_rps * turns,
            graze: pose.graze,
            balk_s: None,
            leap: None,
            wings: false,
        }
    }
}

impl Land for Plane<'_> {
    /// Whether a cell of land holds the kind: inside the sector, over the
    /// sea, on ground it lives on and no steeper than it stands on, and no
    /// volume holding a cell over it or over a cell beside it: a body is
    /// longer than the point it stands on, and none puts its head in. The
    /// ground of each cell is read once, at its middle, so a herd reads the
    /// same land wherever it asks from.
    fn walkable(&self, at: DVec2) -> bool {
        let (ci, cj) = self.cell(at);
        !self.built_over(ci, cj) && self.ground_takes(ci, cj)
    }
}

impl Plane<'_> {
    /// The cell of land a point is in, in cells of [`LAND_M`] of the
    /// sector.
    fn cell(&self, at: DVec2) -> (i64, i64) {
        let blocks = self.home + at / BLOCK_M;
        let cell_b = LAND_M / BLOCK_M;
        (
            (blocks.x / cell_b).floor() as i64,
            (blocks.y / cell_b).floor() as i64,
        )
    }

    /// Whether the ground of a cell of land holds the kind, read once.
    fn ground_takes(&self, ci: i64, cj: i64) -> bool {
        let key = (self.kind.number, self.sector, ci, cj);
        if let Some(&takes) = self.land.borrow().get(&key) {
            return takes;
        }
        let cell_b = LAND_M / BLOCK_M;
        let middle = DVec2::new(ci as f64 + 0.5, cj as f64 + 0.5) * cell_b;
        let takes = self.reads(middle);
        self.land.borrow_mut().insert(key, takes);
        takes
    }

    /// Where the herd's home is, metres from the spot of its region the
    /// plane is laid from: the spot, where the land takes it; where its
    /// ground takes it and only what is built refuses it, the nearest point
    /// of land within [`HOME_MOVES_M`] that takes it, so a herd built over
    /// moves the least it can; where the ground refuses it, none.
    fn home(&self) -> Option<DVec2> {
        if self.walkable(DVec2::ZERO) {
            return Some(DVec2::ZERO);
        }
        let (ci, cj) = self.cell(DVec2::ZERO);
        if !self.ground_takes(ci, cj) {
            return None;
        }
        let reach = (HOME_MOVES_M / LAND_M) as i64;
        let mut cells: Vec<(i64, i64)> = (-reach..=reach)
            .flat_map(|i| (-reach..=reach).map(move |j| (i, j)))
            .filter(|&(i, j)| i * i + j * j <= reach * reach)
            .collect();
        cells.sort_by_key(|&(i, j)| i * i + j * j);
        cells
            .into_iter()
            .map(|(i, j)| DVec2::new(i as f64, j as f64) * LAND_M)
            .find(|&at| self.walkable(at))
    }

    /// Whether a volume holding a cell stands over a cell of land or a cell
    /// beside it: the plots of the middles of the cells either side, each
    /// way, across a seam as anywhere. Cells nest in plots, so that is the
    /// cell's own plot and those it borders.
    fn built_over(&self, ci: i64, cj: i64) -> bool {
        if self.occupied.is_empty() {
            return false;
        }
        let cell_b = LAND_M / BLOCK_M;
        let side = |c: i64| [c as f64 - 0.5, c as f64 + 1.5].map(|at| at * cell_b);
        side(ci).into_iter().any(|u| {
            side(cj).into_iter().any(|v| {
                let point = self.grid.wrapped(SurfacePoint::new(self.sector, u, v));
                self.occupied
                    .contains(&(point.sector, seat::plot_of(point)))
            })
        })
    }

    fn reads(&self, blocks: DVec2) -> bool {
        stands(self.kind, self.generator, self.sector, blocks)
    }
}

/// Whether a kind stands at a place of a sector, in its blocks: inside
/// the sector, over the sea, on ground it lives on and no steeper than it
/// stands on.
fn stands(kind: &Kind, generator: &Generator, sector: Sector, blocks: DVec2) -> bool {
    let grid = generator.sphere().blocks();
    let side = f64::from(grid.side());
    let slope_b = SLOPE_M / BLOCK_M;
    if blocks.min_element() < slope_b || blocks.max_element() > side - slope_b {
        return false;
    }
    let sample = |du: f64, dv: f64| {
        let point = SurfacePoint::new(sector, blocks.x + du, blocks.y + dv);
        generator.sample(grid.direction(point))
    };
    let middle = sample(0.0, 0.0);
    let lives =
        habitat(middle.material).is_some_and(|habitat| kind.herding.lives_on.contains(&habitat));
    if middle.height_m < SHORE_M || !lives {
        return false;
    }
    let height = |du: f64, dv: f64| sample(du, dv).height_m;
    let across = height(slope_b, 0.0) - height(-slope_b, 0.0);
    let along = height(0.0, slope_b) - height(0.0, -slope_b);
    libm::hypot(across, along) / (2.0 * SLOPE_M) <= kind.herding.steep
}

/// How far either way from straight behind the eye a pet called from
/// afar may come into sight, radians, tried nearest first; and how far
/// from where it is called to, as shares of [`fauna::COMES_M`].
const ENTERS_AT: [f64; 7] = [0.0, 0.4, -0.4, 0.8, -0.8, 1.2, -1.2];
const ENTERS_M: [f64; 3] = [1.0, 0.7, 1.3];

/// Where an animal of `kind` called from afar to `to` comes into sight:
/// [`fauna::COMES_M`] or so from it, behind an eye looking along `ahead`,
/// on ground its kind stands on, as near straight behind as there is.
/// None when there is no such ground about.
pub(crate) fn entrance(
    kind: &Kind,
    to: DVec3,
    ahead: DVec3,
    generator: &Generator,
) -> Option<DVec3> {
    let radius_m = generator.sphere().radius_m();
    let grid = generator.sphere().blocks();
    let up = to.normalize();
    let behind = -(ahead - up * ahead.dot(up)).normalize_or(up.any_orthonormal_vector());
    ENTERS_M.iter().find_map(|share| {
        ENTERS_AT.iter().find_map(|&turn| {
            let away = DQuat::from_axis_angle(up, turn) * behind;
            let angle = share * fauna::COMES_M / radius_m;
            let at = DQuat::from_axis_angle(up.cross(away), angle) * up * radius_m;
            let point = grid.surface_point(at.to_array());
            let blocks = DVec2::new(point.u, point.v);
            stands(kind, generator, point.sector, blocks).then_some(at)
        })
    })
}

/// Where a spot is on the planet, the unit up through it, and which way
/// it faces there along the ground; none for a spot that is no place: a
/// sector that is none, a place off its side, a number not finite.
pub(crate) fn spot_at(spot: &Spot, generator: &Generator) -> Option<(DVec3, DVec3)> {
    let sector = Sector::new(u8::try_from(spot.sector).ok()?)?;
    let grid = generator.sphere().blocks();
    let side = f64::from(grid.side());
    let within = |at: f64| at.is_finite() && (0.0..=side).contains(&at);
    if !(within(spot.u) && within(spot.v) && spot.heading.is_finite()) {
        return None;
    }
    let tangents = grid.tangents(SurfacePoint::new(sector, spot.u, spot.v));
    let (du, dv) = (DVec3::from(tangents.du), DVec3::from(tangents.dv));
    let (sin, cos) = libm::sincos(spot.heading);
    let heading = (du * cos + dv * sin).normalize_or(du.normalize());
    Some((DVec3::from(tangents.up), heading))
}

/// The spot of a world point on the planet, facing `heading` along the
/// ground there, at `at_ms` on the world's clock: the heading said as an
/// angle from the sector's `+u` toward its `+v`, the axes as the address
/// draws them, neither unit nor quite square.
pub(crate) fn spot_of(at: DVec3, heading: DVec3, at_ms: u64, generator: &Generator) -> Spot {
    let grid = generator.sphere().blocks();
    let point = grid.surface_point(at.to_array());
    let tangents = grid.tangents(point);
    let (du, dv) = (DVec3::from(tangents.du), DVec3::from(tangents.dv));
    // The heading as a sum of the two axes: a and b solve it in their own
    // metric.
    let (uu, uv, vv) = (du.dot(du), du.dot(dv), dv.dot(dv));
    let (hu, hv) = (heading.dot(du), heading.dot(dv));
    let det = uu * vv - uv * uv;
    let (a, b) = ((hu * vv - hv * uv) / det, (hv * uu - hu * uv) / det);
    Spot {
        sector: point.sector.index() as u32,
        u: point.u,
        v: point.v,
        heading: libm::atan2(b, a),
        at_ms,
    }
}

/// The traits a pet wears: those its record keeps, when an animal of its
/// kind can be drawn in them; else none, and its herd draws it.
fn worn<'a>(pet: &'a Pet, kind: &Kind) -> Option<&'a Traits> {
    pet.traits.as_deref().filter(|traits| traits.fits(kind))
}

/// Where a pet goes at `now_s` as the world keeps it: called, on its way
/// from where it set off to where it was called, along the great circle
/// as [`called`] runs it; else standing there, grazing since it came. On
/// the generator's ground, as a herd stands. None while it is out of
/// sight, called from afar and not yet come.
fn kept(pet: &Pet, kind: &Kind, seed: u64, now_s: f64, generator: &Generator) -> Option<Going> {
    kept_run(pet, kind, seed, now_s, generator).map(|(going, _)| going)
}

/// How a pet called runs: up to its gallop at its size's share, gaining
/// and losing speed as it does under a rider; a kind that carries none, at
/// its herd's wander.
#[derive(Clone, Copy, Debug)]
struct Called {
    top_mps: f64,
    accel_mps2: f64,
    decel_mps2: f64,
}

/// How a kind that carries none gains speed and loses it, called, metres
/// a second squared.
const WANDER_MPS2: f64 = 2.0;

fn called(pet: &Pet, kind: &Kind) -> Called {
    let gallop = kind
        .carry
        .and_then(|carry| Some((carry, carry.paces.get(carry.stepped)?)));
    match gallop {
        Some((carry, gallop)) => {
            let size = pet.traits.as_ref().map(|traits| traits.size);
            Called {
                top_mps: gallop.speed_mps * size.map_or(1.0, |size| carry.by_size.share(size)),
                accel_mps2: carry.accel_mps2,
                decel_mps2: carry.decel_mps2,
            }
        }
        None => Called {
            top_mps: kind.herding.wander_mps,
            accel_mps2: WANDER_MPS2,
            decel_mps2: WANDER_MPS2,
        },
    }
}

/// [`kept`], and whether it is still on its way to where it was called.
fn kept_run(
    pet: &Pet,
    kind: &Kind,
    seed: u64,
    now_s: f64,
    generator: &Generator,
) -> Option<(Going, bool)> {
    if coming(pet, now_s) {
        return None;
    }
    let radius_m = generator.sphere().radius_m();
    let on_ground = |up: DVec3| up * (radius_m + generator.sample(up.to_array()).height_m.max(0.0));
    let (to, facing) = spot_at(&pet.spot, generator)?;
    let since_s = pet.spot.at_ms as f64 / 1000.0;
    let pace = called(pet, kind);
    let mut stood_s = now_s - since_s;
    if let Some(from) = pet.from.as_ref().and_then(|from| spot_at(from, generator)) {
        let angle = from.0.angle_between(to);
        let far_m = angle * radius_m;
        let run = fauna::Run::new(
            far_m,
            pet.from_mps,
            pace.top_mps,
            pace.accel_mps2,
            pace.decel_mps2,
        );
        let (along_m, speed_mps) = run.at(now_s - since_s);
        if along_m < far_m && far_m > 0.0 {
            let axis = from.0.cross(to).normalize_or(facing);
            let up = DQuat::from_axis_angle(axis, along_m / radius_m)
                .mul_vec3(from.0)
                .normalize();
            let toward = to - up * to.dot(up);
            let heading = toward.normalize_or(facing);
            let going = Going {
                speed_mps,
                ..Going::standing(on_ground(up), heading)
            };
            return Some((going, true));
        }
        stood_s -= run.whole_s();
    }
    let going = Going {
        graze: fauna::grazing(stood_s.max(0.0), seed),
        ..Going::standing(on_ground(to), facing)
    };
    Some((going, false))
}

/// Whether a pet called from afar is out of sight at `now_s`: it left
/// where it was at `from.at_ms` and comes into sight at `spot.at_ms`. A
/// call within earshot sets off as it is made, both the same instant.
pub(crate) fn coming(pet: &Pet, now_s: f64) -> bool {
    let comes_ms = pet.spot.at_ms;
    let afar = pet.from.is_some_and(|from| from.at_ms < comes_ms);
    afar && now_s * 1000.0 < comes_ms as f64
}

/// The ground an animal lives on, as a material of the generator's.
fn habitat(material: Material) -> Option<Habitat> {
    match material {
        Material::Grass => Some(Habitat::Grass),
        Material::Forest => Some(Habitat::Forest),
        Material::Sand => Some(Habitat::Sand),
        Material::Snow => Some(Habitat::Snow),
        Material::Rock => Some(Habitat::Rock),
        _ => None,
    }
}

/// The ground as a hoof steps on it: the generator's, and what is built
/// over it within a step, as a walker's feet meet it.
struct Hooves<'a> {
    generator: &'a Generator,
    built: Built<'a>,
}

impl Ground for Hooves<'_> {
    fn under(&self, at: DVec3) -> Footing {
        let sphere = self.generator.sphere();
        let grid = sphere.blocks();
        let point = grid.surface_point(at.to_array());
        let direction = grid.direction(point);
        let up = DVec3::from(direction);
        let sample = self.generator.sample(direction);
        let feet_m = at.length() - sphere.radius_m();
        // What the probe reaches is under the hoof; a floor over that is
        // the top of a wall it is in, a trunk's, a volume's or a part's, and
        // the hoof stands on the highest of the rest, by the wall's foot.
        let probe_m = feet_m + STEP_UP_M;
        let floor_m = self
            .built
            .footings(self.generator, Seat::Sector(point.sector), point, probe_m)
            .filter_map(|footing| footing.floor_m)
            .filter(|&floor_m| floor_m <= probe_m + STEP_M)
            .reduce(f64::max)
            .filter(|&floor_m| floor_m > sample.height_m);
        let (height_m, surface) = match floor_m {
            Some(floor_m) => (floor_m, BUILT),
            None => (sample.height_m, sample.material as u8),
        };
        Footing {
            point: up * (sphere.radius_m() + height_m),
            up,
            surface,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Seed 1 opens 45 m from a herd's home, on ground some 150 m over the
    /// datum: every animal of it is drawn, wherever the clock has it.
    #[test]
    fn a_herd_on_high_ground_is_drawn_whole_at_every_hour() {
        let generator = Generator::new(worldgen::Recipe::new(1)).unwrap();
        let spawn = crate::spawn_point(&generator);
        let (cells, parts) = (crate::Cells::default(), crate::parts::Parts::default());
        let seen: Vec<usize> = [0.0, 1e3, 1e5, 1e7]
            .into_iter()
            .map(|now_s| {
                let mut animals = Animals::default();
                let built = Built {
                    cells: &cells,
                    parts: &parts,
                    trunks: &crate::Trunks::default(),
                };
                let bodies = Bodies {
                    riders: &[],
                    on_foot: &[],
                };
                animals.update(1.0 / 60.0, now_s, Some(spawn), bodies, &generator, built);
                animals.near().len()
            })
            .collect();
        assert!(seen[0] >= HORSE_HERD.0, "{seen:?}");
        assert!(seen.iter().all(|&n| n == seen[0]), "{seen:?}");
    }

    const HORSE_HERD: (usize, usize) = fauna::horse::HORSE.herding.size;

    /// A pet the world kept at no place, an old row, is no pet: its animal
    /// is drawn with its herd, as wild.
    #[test]
    fn a_pet_at_no_place_is_drawn_with_its_herd() {
        let generator = Generator::new(worldgen::Recipe::new(1)).unwrap();
        let spawn = crate::spawn_point(&generator);
        let (cells, parts) = (crate::Cells::default(), crate::parts::Parts::default());
        let trunks = crate::Trunks::default();
        let built = Built {
            cells: &cells,
            parts: &parts,
            trunks: &trunks,
        };
        let bodies = Bodies {
            riders: &[],
            on_foot: &[],
        };
        let mut animals = Animals::default();
        animals.update(1.0 / 60.0, 1.0, Some(spawn), bodies, &generator, built);
        let number = animals.near()[0].number;
        let at = DVec3::from(generator.sphere().blocks().direction(spawn));
        let good = spot_of(at * generator.sphere().radius_m(), DVec3::X, 0, &generator);
        for bad in [
            Spot {
                u: f64::NAN,
                ..good
            },
            Spot {
                heading: f64::INFINITY,
                ..good
            },
            Spot { sector: 9, ..good },
            Spot { v: -5.0, ..good },
        ] {
            for (spot, from) in [(bad, None), (good, Some(bad))] {
                let pet = Pet {
                    number,
                    name: "Nowhere".into(),
                    spot,
                    from,
                    from_mps: 0.0,
                    owner: "another".into(),
                    mine: false,
                    session: 0,
                    traits: None,
                };
                animals.set_pets([pet].iter(), &generator);
                animals.update(1.0 / 60.0, 1.0, Some(spawn), bodies, &generator, built);
                let drawn = animals.near().iter().find(|near| near.number == number);
                assert_eq!(
                    drawn.map(|near| near.keeping),
                    Some(Keeping::Wild),
                    "{spot:?} {from:?}"
                );
            }
        }
    }

    /// A pet called from afar is nowhere, neither pet nor herd, while it is
    /// on its way out of sight; then it comes into sight behind the eye,
    /// on ground it stands on, gallops in and stands beside its keeper.
    #[test]
    fn a_pet_called_from_afar_comes_into_sight_behind_the_eye_and_gallops_in() {
        let generator = Generator::new(worldgen::Recipe::new(1)).unwrap();
        let spawn = crate::spawn_point(&generator);
        let (cells, parts) = (crate::Cells::default(), crate::parts::Parts::default());
        let trunks = crate::Trunks::default();
        let built = Built {
            cells: &cells,
            parts: &parts,
            trunks: &trunks,
        };
        let bodies = Bodies {
            riders: &[],
            on_foot: &[],
        };
        let mut animals = Animals::default();
        let now_s = 1_000.0;
        animals.update(1.0 / 60.0, now_s, Some(spawn), bodies, &generator, built);
        let number = animals.near()[0].number;
        let kind = Animals::kind_of(number).unwrap();
        let radius_m = generator.sphere().radius_m();
        let to = DVec3::from(generator.sphere().blocks().direction(spawn)) * radius_m;
        let ahead = to.normalize().any_orthonormal_vector();

        let enter = entrance(kind, to, ahead, &generator).expect("ground about the spawn");
        let apart_m = enter.angle_between(to) * radius_m;
        assert!(
            (0.6..1.4).contains(&(apart_m / fauna::COMES_M)),
            "{apart_m} m off"
        );
        assert!((enter - to).normalize().dot(ahead) < -0.3, "behind the eye");
        let point = generator.sphere().blocks().surface_point(enter.to_array());
        let blocks = DVec2::new(point.u, point.v);
        assert!(stands(kind, &generator, point.sector, blocks));

        // A pony, called at its own gallop, comes into sight at it, as the
        // caller says: never the run, never another size's.
        let carry = kind.carry.unwrap();
        let mut pony = Look::new(kind, 7, None).traits;
        pony.size = carry.by_size.sizes.0;
        let gallop_mps = carry.paces[carry.stepped].speed_mps * carry.by_size.shares.0;
        let (left_ms, comes_ms) = (1_000_000, 1_003_000);
        let pet = Pet {
            number,
            name: "Canela".into(),
            spot: spot_of(to, ahead, comes_ms, &generator),
            from: Some(spot_of(enter, to - enter, left_ms, &generator)),
            from_mps: gallop_mps,
            owner: "ana".into(),
            mine: false,
            session: 0,
            traits: Some(Box::new(pony)),
        };
        let drawn = |animals: &mut Animals, now_s: f64| {
            animals.update(1.0 / 60.0, now_s, Some(spawn), bodies, &generator, built);
            animals
                .near()
                .iter()
                .find(|near| near.number == number)
                .copied()
        };
        animals.set_pets([pet.clone()].iter(), &generator);
        for t_s in [0.0, 1.0, 2.9] {
            assert!(kept(&pet, kind, 7, now_s + t_s, &generator).is_none());
            let seen = drawn(&mut animals, now_s + t_s).map(|near| near.keeping);
            assert_eq!(seen, None, "out of sight at {t_s} s");
        }
        let run_mps = carry.paces.last().unwrap().speed_mps;
        assert!(gallop_mps < 0.9 * run_mps, "a pony's gallop is no run");
        let came = kept(&pet, kind, 7, now_s + 3.2, &generator).unwrap();
        let came_m = came.position.angle_between(enter) * radius_m;
        assert!(
            came_m < 0.3 * gallop_mps && (came.speed_mps - gallop_mps).abs() < 1e-9,
            "galloping in at its gallop: {} m/s",
            came.speed_mps
        );
        assert_eq!(animals.called_mps(number), Some(gallop_mps));
        let near = drawn(&mut animals, now_s + 3.2).expect("in sight");
        assert_eq!(near.keeping, Keeping::Anothers);
        let pace = called(&pet, kind);
        let run = fauna::Run::new(
            apart_m,
            gallop_mps,
            gallop_mps,
            pace.accel_mps2,
            pace.decel_mps2,
        );
        let run_s = run.whole_s();
        let stood = kept(&pet, kind, 7, now_s + 3.0 + run_s + 1.0, &generator).unwrap();
        assert!(stood.position.angle_between(to) * radius_m < 0.01 && stood.speed_mps == 0.0);

        // A call within earshot sets off at once, both instants the same.
        let at_once = Pet {
            from: Some(Spot {
                at_ms: comes_ms,
                ..pet.from.unwrap()
            }),
            ..pet.clone()
        };
        assert!(
            kept(
                &at_once,
                kind,
                7,
                comes_ms as f64 / 1000.0 - 0.05,
                &generator
            )
            .is_some()
        );
    }

    /// A pet wears the traits its record keeps, whatever its herd would
    /// give it now, and a body drawn otherwise is drawn again.
    #[test]
    fn a_pet_wears_the_traits_its_record_keeps() {
        let generator = Generator::new(worldgen::Recipe::new(1)).unwrap();
        let spawn = crate::spawn_point(&generator);
        let (cells, parts) = (crate::Cells::default(), crate::parts::Parts::default());
        let trunks = crate::Trunks::default();
        let built = Built {
            cells: &cells,
            parts: &parts,
            trunks: &trunks,
        };
        let bodies = Bodies {
            riders: &[],
            on_foot: &[],
        };
        let mut animals = Animals::default();
        animals.update(1.0 / 60.0, 1.0, Some(spawn), bodies, &generator, built);
        let number = animals.near()[0].number;
        let kind = Animals::kind_of(number).unwrap();
        let herds = animals.body(number, &generator).unwrap().animal.look.traits;
        let radius_m = generator.sphere().radius_m();
        let at = DVec3::from(generator.sphere().blocks().direction(spawn)) * radius_m;
        let pet = |traits: Option<Traits>| Pet {
            number,
            name: "Canela".into(),
            spot: spot_of(at, DVec3::X, 0, &generator),
            from: None,
            from_mps: 0.0,
            owner: "ana".into(),
            mine: false,
            session: 0,
            traits: traits.map(Box::new),
        };
        // Another animal's traits, winged and not, and a record from before.
        let others = [None, Some("white"), Some("black")]
            .map(|wings| Some(Traits::drawn(kind, 9_001, wings)));
        for kept in others.into_iter().chain([None]) {
            animals.set_pets([pet(kept.clone())].iter(), &generator);
            let worn = animals.body(number, &generator).unwrap().animal.look.traits;
            assert_eq!(worn, kept.clone().unwrap_or(herds.clone()));
            let drawn = animals.free.get(&number);
            assert!(
                drawn.is_none_or(|drawn| drawn.animal.look.traits == worn),
                "drawn again"
            );
        }
        // Traits no animal can be drawn in are drawn as its herd's.
        let broken = Traits {
            size: f64::NAN,
            ..herds.clone()
        };
        animals.set_pets([pet(Some(broken.clone()))].iter(), &generator);
        assert_eq!(
            animals.body(number, &generator).unwrap().animal.look.traits,
            herds
        );
        // Drawn so, it is kept so as the pets are told again.
        animals.update(1.0 / 60.0, 1.1, Some(spawn), bodies, &generator, built);
        assert!(animals.left.contains_key(&number));
        animals.set_pets([pet(Some(broken))].iter(), &generator);
        assert!(animals.left.contains_key(&number), "drawn again");
    }

    /// A pet whose herd no longer holds its place, as a rule drew the herd
    /// smaller since it was tamed, is drawn in what it keeps; freed, it
    /// grazes where it was left while a body is near, and is gone once
    /// none is.
    #[test]
    fn a_pet_past_its_herds_places_is_drawn() {
        let generator = Generator::new(worldgen::Recipe::new(1)).unwrap();
        let spawn = crate::spawn_point(&generator);
        let (cells, parts) = (crate::Cells::default(), crate::parts::Parts::default());
        let trunks = crate::Trunks::default();
        let built = Built {
            cells: &cells,
            parts: &parts,
            trunks: &trunks,
        };
        let radius_m = generator.sphere().radius_m();
        let at = DVec3::from(generator.sphere().blocks().direction(spawn)) * radius_m;
        let near = [at];
        let bodies = Bodies {
            riders: &[],
            on_foot: &near,
        };
        let mut animals = Animals::default();
        animals.update(1.0 / 60.0, 1.0, Some(spawn), bodies, &generator, built);
        let (region, _) = Region::of(animals.near()[0].number).unwrap();
        let len = animals.placed(region, &generator).unwrap().herd.len();
        let number = region.number(len + 2);
        assert!(animals.body(number, &generator).is_none(), "no animal");
        let kept = Traits::drawn(region.kind().unwrap(), 9_001, Some("black"));
        let pet = Pet {
            number,
            spot: spot_of(at, DVec3::X, 0, &generator),
            traits: Some(Box::new(kept.clone())),
            ..Pet::default()
        };
        animals.set_pets([pet].iter(), &generator);
        animals.update(1.0 / 60.0, 1.1, Some(spawn), bodies, &generator, built);
        assert_eq!(animals.left[&number].animal.look.traits, kept);
        let stood = animals.left[&number].animal.position();
        // Freed, it stays while watched.
        animals.set_pets([].iter(), &generator);
        for step in 0..120 {
            let now_s = 1.2 + f64::from(step) / 60.0;
            animals.update(1.0 / 60.0, now_s, Some(spawn), bodies, &generator, built);
        }
        let left = &animals.left[&number];
        assert!((left.animal.position() - stood).length() < 0.5);
        // No body near, it is gone.
        let away = Bodies {
            riders: &[],
            on_foot: &[],
        };
        animals.update(1.0 / 60.0, 4.0, Some(spawn), away, &generator, built);
        assert!(!animals.left.contains_key(&number));
    }

    /// Others' pets standing near take the places of free animals: the
    /// animals drawn stay within [`DRAWN`], however many are kept here.
    #[test]
    fn pets_near_share_the_herds_budget() {
        let generator = Generator::new(worldgen::Recipe::new(1)).unwrap();
        let spawn = crate::spawn_point(&generator);
        let (cells, parts) = (crate::Cells::default(), crate::parts::Parts::default());
        let trunks = crate::Trunks::default();
        let built = Built {
            cells: &cells,
            parts: &parts,
            trunks: &trunks,
        };
        let bodies = Bodies {
            riders: &[],
            on_foot: &[],
        };
        let mut animals = Animals::default();
        // Horses of herds far along the sector, a pet being where its
        // keeper left it; the herd at the spawn stays free.
        let mut numbers: Vec<u64> = Vec::new();
        let home = Animals::in_view(spawn, &generator);
        for step in 1..64 {
            let along = SurfacePoint::new(spawn.sector, spawn.u + 400.0 * f64::from(step), spawn.v);
            for region in Animals::in_view(along, &generator) {
                if home.contains(&region) {
                    continue;
                }
                for index in 0..HORSE_HERD.1 {
                    let number = region.number(index);
                    if !numbers.contains(&number) && animals.body(number, &generator).is_some() {
                        numbers.push(number);
                    }
                }
            }
        }
        numbers.truncate(DRAWN + 4);
        assert_eq!(numbers.len(), DRAWN + 4, "enough horses near to keep");
        // Each stands a step apart around the spawn, kept by another.
        let grid = generator.sphere().blocks();
        let pets: Vec<Pet> = numbers
            .iter()
            .enumerate()
            .map(|(i, &number)| {
                let point = SurfacePoint::new(spawn.sector, spawn.u + 3.0 * i as f64, spawn.v);
                let at = DVec3::from(grid.direction(point)) * generator.sphere().radius_m();
                Pet {
                    number,
                    name: format!("pet {i}"),
                    spot: spot_of(at, DVec3::X, 0, &generator),
                    from: None,
                    from_mps: 0.0,
                    owner: "another".into(),
                    mine: false,
                    session: 0,
                    traits: None,
                }
            })
            .collect();
        animals.set_pets(pets.iter(), &generator);
        animals.update(1.0 / 60.0, 1.0, Some(spawn), bodies, &generator, built);
        let near = animals.near();
        assert_eq!(near.len(), DRAWN);
        assert!(
            near.iter().all(|near| near.keeping == Keeping::Anothers),
            "no free animal past the budget"
        );
    }

    /// A reconnect's Welcome jumps the clock: the herd drawn takes the jump
    /// and stands where a client that just came draws it, each animal set
    /// anew where its path has it.
    #[test]
    fn a_clock_jumped_by_a_reconnect_draws_the_herd_where_it_is_now() {
        let generator = Generator::new(worldgen::Recipe::new(1)).unwrap();
        let spawn = crate::spawn_point(&generator);
        let (cells, parts) = (crate::Cells::default(), crate::parts::Parts::default());
        let trunks = crate::Trunks::default();
        let built = Built {
            cells: &cells,
            parts: &parts,
            trunks: &trunks,
        };
        let bodies = Bodies {
            riders: &[],
            on_foot: &[],
        };
        let frame = 1.0 / 60.0;
        let live = |animals: &mut Animals, from_s: f64, frames: usize| {
            for n in 0..frames {
                let now_s = from_s + frame * n as f64;
                animals.update(frame, now_s, Some(spawn), bodies, &generator, built);
            }
        };
        let drawn = |animals: &Animals| {
            let mut near: Vec<(u64, DVec3)> =
                animals.near().iter().map(|n| (n.number, n.at)).collect();
            near.sort_by_key(|&(number, _)| number);
            near
        };
        let mut stayed = Animals::default();
        live(&mut stayed, 1.0e3, 120);
        let before = drawn(&stayed);
        let jumped_s = 1.0e3 + 600.0;
        live(&mut stayed, jumped_s, 3);
        let moved = drawn(&stayed)
            .iter()
            .zip(&before)
            .any(|((_, at), (_, was))| (*at - *was).length() > 2.0);
        assert!(moved, "ten minutes on, the herd has gone somewhere");
        let mut came = Animals::default();
        live(&mut came, jumped_s, 3);
        let (stayed, came) = (drawn(&stayed), drawn(&came));
        assert!(came.len() >= HORSE_HERD.0, "{came:?}");
        assert_eq!(stayed.len(), came.len());
        for ((number, at), (same, there)) in stayed.into_iter().zip(came) {
            assert_eq!(number, same);
            let off_m = (at - there).length();
            assert!(off_m < 0.01, "animal {number:x} is {off_m} m off");
        }
    }

    /// A body standing where a horse grazes, and a rider beside it: the
    /// horse goes on where its herd has it, frame for frame as if nobody
    /// were there. No animal moves for a body.
    #[test]
    fn a_horse_goes_its_way_whoever_stands_in_it() {
        let generator = Generator::new(worldgen::Recipe::new(1)).unwrap();
        let spawn = crate::spawn_point(&generator);
        let (cells, parts) = (crate::Cells::default(), crate::parts::Parts::default());
        let built = Built {
            cells: &cells,
            parts: &parts,
            trunks: &crate::Trunks::default(),
        };
        let now_s = 1.0e3;
        let none = Bodies {
            riders: &[],
            on_foot: &[],
        };
        let (mut alone, mut crowded) = (Animals::default(), Animals::default());
        alone.update(1.0 / 60.0, now_s, Some(spawn), none, &generator, built);
        let horse = alone.near()[0];
        let rider = Rider {
            who: Some(9),
            number: 1,
            at: horse.at + horse.heading * 1.5,
            facing: horse.heading,
            speed_mps: 0.0,
            balk_s: None,
            aloft: Some(false),
            leaps: false,
            wings: false,
        };
        let (riders, on_foot) = ([rider], [horse.at]);
        let bodies = Bodies {
            riders: &riders,
            on_foot: &on_foot,
        };
        crowded.update(1.0 / 60.0, now_s, Some(spawn), bodies, &generator, built);
        let at = |animals: &Animals| {
            let near = animals
                .near()
                .iter()
                .find(|near| near.number == horse.number);
            near.map(|near| near.at)
        };
        for frame in 1..4 * 60 {
            let t_s = now_s + f64::from(frame) / 60.0;
            alone.update(1.0 / 60.0, t_s, Some(spawn), none, &generator, built);
            crowded.update(1.0 / 60.0, t_s, Some(spawn), bodies, &generator, built);
            assert_eq!(at(&crowded), at(&alone), "frame {frame}");
        }
    }

    /// A horse ridden 300 m from its herd and left there: it stands beside
    /// the body, drawn every frame, and is still there 20 s on; once the
    /// body has gone it walks back, never faster than a walk, until it is
    /// with its herd.
    #[test]
    fn a_horse_left_far_from_its_herd_waits_then_walks_back() {
        let generator = Generator::new(worldgen::Recipe::new(1)).unwrap();
        let spawn = crate::spawn_point(&generator);
        let (cells, parts) = (crate::Cells::default(), crate::parts::Parts::default());
        let built = Built {
            cells: &cells,
            parts: &parts,
            trunks: &crate::Trunks::default(),
        };
        let ground = Hooves {
            generator: &generator,
            built,
        };
        let grid = generator.sphere().blocks();
        let radius_m = generator.sphere().radius_m();
        let mut now_s = 1.0e3;
        let mut animals = Animals::default();
        let none = Bodies {
            riders: &[],
            on_foot: &[],
        };
        animals.update(1.0 / 60.0, now_s, Some(spawn), none, &generator, built);
        let horse = animals.near()[0];
        let up = horse.at.normalize();
        let east = up.any_orthonormal_vector();
        // Somewhere 300 m off over the sea, in the same sector.
        let left = (0..16)
            .map(|i| {
                let turn =
                    glam::DQuat::from_axis_angle(up, f64::from(i) * core::f64::consts::TAU / 16.0);
                ground.under(horse.at + turn * east * 300.0).point
            })
            .find(|at| at.length() > radius_m + SHORE_M)
            .expect("land 300 m off");
        let near = |at: DVec3| grid.surface_point(at.to_array());
        let rider = [Rider {
            who: None,
            number: horse.number,
            at: left,
            facing: up.cross(east),
            speed_mps: 0.0,
            balk_s: None,
            aloft: Some(false),
            leaps: false,
            wings: false,
        }];
        let riding = Bodies {
            riders: &rider,
            on_foot: &[],
        };
        animals.update(
            1.0 / 60.0,
            now_s,
            Some(near(left)),
            riding,
            &generator,
            built,
        );
        let of = |animals: &Animals| {
            animals
                .near()
                .iter()
                .find(|near| near.number == horse.number)
                .map(|near| near.at)
        };
        let ground_m = |a: DVec3, b: DVec3| a.normalize().angle_between(b.normalize()) * radius_m;
        // Stepped down from, the body beside it.
        let beside = [left + east * 1.5];
        let watching = Bodies {
            riders: &[],
            on_foot: &beside,
        };
        let dt = 1.0 / 20.0;
        for frame in 0..20 * 20 {
            now_s += dt;
            animals.update(
                dt,
                now_s,
                Some(near(beside[0])),
                watching,
                &generator,
                built,
            );
            let at = of(&animals).unwrap_or_else(|| panic!("gone at frame {frame}"));
            assert!(
                ground_m(at, left) < 2.0,
                "frame {frame}: {} m from where it was left",
                ground_m(at, left)
            );
        }
        // The body goes back to the herd; the horse walks there after it,
        // drawn all the way.
        let walk_mps = horse.kind.herding.wander_mps;
        let gone = [horse.at + east * 10.0];
        let mut was = of(&animals).unwrap();
        let start_m = ground_m(was, horse.at);
        assert!(start_m > 250.0, "left {start_m} m from its herd");
        let home = |animals: &Animals| !animals.left.contains_key(&horse.number);
        let mut frames = 0;
        while !home(&animals) {
            frames += 1;
            assert!(
                f64::from(frames) * dt * walk_mps < 2.0 * start_m,
                "not home yet"
            );
            now_s += dt;
            let away = Bodies {
                riders: &[],
                on_foot: &gone,
            };
            animals.update(dt, now_s, Some(near(gone[0])), away, &generator, built);
            let at = of(&animals).unwrap_or_else(|| panic!("gone at frame {frames}"));
            let step_m = ground_m(at, was);
            assert!(
                step_m <= walk_mps * dt + 1e-3,
                "frame {frames}: a leap of {step_m} m"
            );
            was = at;
        }
    }

    /// A body riding horse `number` in paces, and the horse drawn under it
    /// every frame as a client draws it.
    struct Riding {
        controller: crate::controller::Controller,
        animals: Animals,
        number: u64,
        now_s: f64,
    }

    impl Riding {
        /// Mounted at `from`, facing along `u`, on what is built there
        /// `on_m` over the datum, if anything.
        fn new(world: &World, number: u64, from: SurfacePoint, on_m: Option<f64>) -> Riding {
            use crate::controller::Controller;
            use crate::way::{Profile, Steering};
            let generator = &world.generator;
            let grid = generator.sphere().blocks();
            let mut controller = Controller::spawn(from, generator);
            controller.stand_at(None, generator);
            if let Some(on_m) = on_m {
                controller.lift_onto(on_m);
            }
            let along = grid.tangents(from).du;
            controller.face(topology::bearing_deg(grid.direction(from), along).unwrap());
            let profile = Profile::Paces(Box::new(crate::paces::tests::horse()));
            assert!(controller.take_way(
                "test.horse",
                &profile,
                Steering::Steered,
                number,
                generator
            ));
            Riding {
                controller,
                animals: Animals::default(),
                number,
                now_s: 1.0e3,
            }
        }

        /// A frame of `wish`: where each hoof set down stands, world space.
        fn ride(&mut self, world: &World, wish: crate::controller::Wish) -> Vec<DVec3> {
            let controller = &mut self.controller;
            controller.update(1.0 / 60.0, wish, &world.generator, world.built());
            let rider = [Rider {
                who: None,
                number: self.number,
                at: controller.drawn_position(),
                facing: controller.facing(),
                speed_mps: controller.ground_mps(),
                balk_s: controller.balked_s(),
                aloft: Some(!controller.grounded()),
                leaps: controller.leaping(),
                wings: false,
            }];
            let bodies = Bodies {
                riders: &rider,
                on_foot: &[],
            };
            self.now_s += 1.0 / 60.0;
            world.step(&mut self.animals, self.now_s, controller.point(), bodies);
            let animal = &self.animals.ridden[&None].animal;
            let feet = animal.feet().into_iter().zip(animal.planted());
            feet.filter_map(|(foot, planted)| planted.then_some(foot))
                .collect()
        }
    }

    /// A horse of a herd near the spawn: its number.
    fn a_horse(world: &World) -> u64 {
        let mut herds = Animals::default();
        let spawn = crate::spawn_point(&world.generator);
        world.step(&mut herds, 1.0e3, spawn, NONE);
        herds.near()[0].number
    }

    /// A horse ridden at a trunk, head on and at a graze, at each pace and
    /// jumping into it: every hoof it sets down stands on the ground, never
    /// up the trunk, and back and turning take it away again.
    #[test]
    fn a_horse_ridden_at_a_trunk_stands_on_the_ground_and_leaves_it() {
        use crate::controller::Wish;
        let mut world = World::new();
        let (plant, trunks, _) = crate::trunks::tests::a_tree(&world.generator);
        world.trunks = trunks;
        let number = a_horse(&world);
        let generator = &world.generator;
        let (grid, sphere) = (generator.sphere().blocks(), generator.sphere());
        let root = plant.root.column;
        // How far a hoof is over the ground under it, metres.
        let over = |foot: DVec3| {
            let direction = grid.direction(grid.surface_point(foot.to_array()));
            foot.length() - sphere.radius_m() - generator.sample(direction).height_m
        };
        for side_m in [0.0, -1.0, 1.0] {
            for (strikes, jump) in [(1, false), (2, false), (3, false), (2, true), (3, true)] {
                let case = format!("{side_m} m aside, pace {strikes}, jumping {jump}");
                let from = SurfacePoint::new(
                    root.sector,
                    f64::from(root.u) - 7.5,
                    f64::from(root.v) + 0.5 + side_m,
                );
                let mut riding = Riding::new(&world, number, from, None);
                let ride = |riding: &mut Riding, wish: Wish, frame: usize| {
                    for foot in riding.ride(&world, wish) {
                        let over_m = over(foot);
                        assert!(
                            over_m.abs() < 0.4,
                            "{case}, frame {frame}: a hoof set down {over_m} m over the ground"
                        );
                    }
                };
                // At it for four seconds, then halted and backed away for
                // three, then walked off for three, turning a quarter first.
                for frame in 0..240 {
                    let wish = Wish {
                        struck: [frame < 10 * strikes && frame % 10 == 0, false],
                        up: jump && frame > 60,
                        ..Wish::default()
                    };
                    ride(&mut riding, wish, frame);
                }
                let stopped = riding.controller.position();
                for frame in 240..420 {
                    let wish = Wish {
                        movement: [0.0, -1.0],
                        struck: [false, frame < 280 && frame % 10 == 0],
                        ..Wish::default()
                    };
                    ride(&mut riding, wish, frame);
                }
                let backed = riding.controller.position();
                let backed_m = (backed - stopped).length();
                assert!(backed_m > 1.0, "{case}: backed {backed_m} m");
                for frame in 420..600 {
                    let wish = Wish {
                        movement: [f32::from(frame < 460), 0.0],
                        struck: [frame == 420, false],
                        ..Wish::default()
                    };
                    ride(&mut riding, wish, frame);
                }
                let left_m = (riding.controller.position() - backed).length();
                assert!(left_m > 2.0, "{case}: walked {left_m} m turning");
            }
        }
    }

    /// A world with a floor laid over the plot [`crate::Cells::floor`]
    /// lays, and on it, cells counted from its corner: a wall two metres
    /// high along `v` at `u` 10, over `v` 0 to 20, and over `v` 30 to 63 a
    /// stair up four blocks from `u` 30, a landing to `u` 43 and a stair
    /// down to the floor by `u` 48. Its sector, the floor's corner cell,
    /// and how high a cell's top stands there, in cells, by its column.
    fn built() -> (World, Sector, [i32; 3], impl Fn(i32, i32) -> i32) {
        use voxel::{Gesture, Span};
        let mut world = World::new();
        let side = f64::from(world.generator.sphere().blocks().side());
        let sector = Sector::new(4).unwrap();
        let point = SurfacePoint::new(sector, side * 0.41, side * 0.37);
        let [x, y, top] = world.cells.floor(&world.generator, point);
        let rise = |du: i32| match du {
            30..=33 => du - 29,
            34..=43 => 4,
            44..=47 => 48 - du,
            _ => 0,
        };
        let mut gestures = vec![Gesture::Create {
            span: Span::between([x + 10, y, top], [x + 10, y + 20, top + 3]),
            paint: 1,
        }];
        gestures.extend((30..48).map(|du| Gesture::Create {
            span: Span::between([x + du, y + 30, top], [x + du, y + 63, top + rise(du) - 1]),
            paint: 1,
        }));
        assert!(world.cells.apply(&world.generator, sector, &gestures));
        let height = move |du: i32, dv: i32| match (du, dv) {
            (10, 0..=20) => top + 4,
            (du, 30..=63) => top + rise(du),
            _ => top,
        };
        (world, sector, [x, y, top], height)
    }

    /// A hoof by a wall stands on the floor it is on, never on the wall's
    /// top nor the ground under the floor; asked from the floor over a step
    /// two blocks up it stands on that step, and asked from up the stair
    /// it stands on the stair, never on the ground under it.
    #[test]
    fn a_hoof_stands_on_what_is_built_within_a_step_and_never_on_a_wall() {
        let (world, sector, [x, y, top], _) = built();
        let sphere = world.generator.sphere();
        let at = |u: f64, v: f64, cells: i32| {
            let point = SurfacePoint::new(sector, f64::from(x) + u, f64::from(y) + v);
            DVec3::from(sphere.position(point, f64::from(cells) * BLOCK_M))
        };
        let stands = |foot: DVec3| {
            let under = world.ground().under(foot).point;
            (under.length() - sphere.radius_m()) / BLOCK_M - f64::from(top)
        };
        let near = |a: f64, b: f64| (a - b).abs() < 1e-6;
        let by_wall = stands(at(9.7, 10.5, top));
        assert!(near(by_wall, 0.0), "by the wall, {by_wall} cells up");
        let up = stands(at(31.5, 40.5, top));
        assert!(near(up, 2.0), "a step two up, {up} cells up");
        let down = stands(at(31.5, 40.5, top + 4));
        assert!(
            (2.0..=3.0).contains(&down),
            "down the stair, {down} cells up"
        );
    }

    /// A horse walked up a stair of cells, over its landing and down: every
    /// hoof it sets down stands on the stair, as high as the cells about
    /// it, and it goes the whole way.
    #[test]
    fn a_horse_ridden_up_a_stair_and_down_stands_on_it() {
        use crate::controller::Wish;
        let (world, sector, [x, y, top], height) = built();
        let number = a_horse(&world);
        let grid = world.generator.sphere().blocks();
        let radius_m = world.generator.sphere().radius_m();
        let from = SurfacePoint::new(sector, f64::from(x) + 20.5, f64::from(y) + 46.5);
        let on_m = f64::from(top) * BLOCK_M;
        let mut riding = Riding::new(&world, number, from, Some(on_m));
        for frame in 0..60 * 12 {
            let wish = Wish {
                struck: [frame == 0, false],
                ..Wish::default()
            };
            for foot in riding.ride(&world, wish) {
                let point = grid.surface_point(foot.to_array());
                let (du, dv) = (point.u - f64::from(x), point.v - f64::from(y));
                // The width a footing spans: the cells about the hoof.
                let span = |at: f64| {
                    (at - crate::collision::BODY_HALF).floor() as i32
                        ..=(at + crate::collision::BODY_HALF).floor() as i32
                };
                let tops: Vec<i32> = span(du)
                    .flat_map(|u| span(dv).map(move |v| (u, v)))
                    .map(|(u, v)| height(u, v))
                    .collect();
                let (low, high) = (tops.iter().min().unwrap(), tops.iter().max().unwrap());
                let cells = (foot.length() - radius_m) / BLOCK_M;
                assert!(
                    cells >= f64::from(*low) - 0.3 && cells <= f64::from(*high) + 0.3,
                    "frame {frame}: a hoof at {du:.2} set down {:.2} cells up, the cells about it {low} to {high}",
                    cells - f64::from(top)
                );
            }
        }
        let gone = grid
            .surface_point(riding.controller.position().to_array())
            .u
            - f64::from(x);
        assert!(gone > 49.0, "it ended at {gone}, short of the stair's foot");
    }

    /// A floor laid on a grazing horse: its herd places it again off what
    /// is built, and it trots there, never set anew; then over a minute of
    /// its herd's bouts no animal of it comes into that plot, nor within a
    /// metre of it.
    #[test]
    fn a_herd_leaves_what_is_built_on_it_and_never_goes_in() {
        let mut world = World::new();
        let spawn = crate::spawn_point(&world.generator);
        let mut animals = Animals::default();
        let mut now_s = 1e3;
        world.step(&mut animals, now_s, spawn, NONE);
        let horse = animals.near()[0];
        let point = world.near(horse.at);
        world.cells.floor(&world.generator, point);
        let plot = seat::plot_of(point);
        // How far a place is outside the plot built over, metres.
        let outside_m = |at: DVec3| {
            let at = world.near(at);
            if at.sector != point.sector {
                return f64::INFINITY;
            }
            let [low_u, low_v] = plot.map(|n| f64::from(n << seat::PLOT_BITS));
            let side = f64::from(1 << seat::PLOT_BITS);
            let off = |at: f64, low: f64| (low - at).max(at - low - side).max(0.0);
            libm::hypot(off(at.u, low_u), off(at.v, low_v)) * BLOCK_M
        };
        assert_eq!(outside_m(horse.at), 0.0, "the floor is on it");
        let trot_mps = gaia::fauna::horse::HORSE.herding.trot_mps * CATCH_PACE;
        let mut was = horse.at;
        let mut out = false;
        for frame in 0..60 * 70 {
            now_s += 1.0 / 60.0;
            world.step(&mut animals, now_s, spawn, NONE);
            let near = animals.near().to_vec();
            if !out {
                let it = near.iter().find(|near| near.number == horse.number);
                let at = it.expect("still drawn").at;
                let moved_m = (at - was).length();
                assert!(
                    moved_m < trot_mps / 60.0 + 0.05,
                    "frame {frame}: it went {moved_m} m at once"
                );
                was = at;
                out = outside_m(at) > LAND_M;
            }
            // Ten seconds to trot off, then a minute of bouts.
            if frame >= 60 * 10 {
                assert!(out, "it never left the floor");
                for near in near.iter().filter(|near| !near.ridden) {
                    let off_m = outside_m(near.at);
                    assert!(off_m > 1.0, "frame {frame}: {} {off_m} m out", near.number);
                }
            }
        }
    }

    /// Free herds wait on the volumes near: none is drawn while the cells
    /// wait on the world, and they come once the world says nothing near
    /// is lacking.
    #[test]
    fn free_herds_wait_on_the_volumes_near() {
        use protocol::Message;
        use protocol::cells as wire;
        let mut world = World::new();
        world.cells.link(1);
        let spawn = crate::spawn_point(&world.generator);
        let mut animals = Animals::default();
        world.step(&mut animals, 1e3, spawn, NONE);
        assert!(animals.near().is_empty(), "drawn before the cells came");
        let complete = wire::Seen {
            complete: true,
            ..wire::Seen::default()
        };
        let payload = complete.encode_to_vec();
        world.cells.receive(&world.generator, wire::SEEN, &payload);
        world.step(&mut animals, 1e3, spawn, NONE);
        assert!(!animals.near().is_empty(), "the herd near");
    }

    /// A world of seed 1 with nothing built.
    struct World {
        generator: Generator,
        cells: crate::Cells,
        parts: crate::parts::Parts,
        trunks: crate::Trunks,
    }

    impl World {
        fn new() -> World {
            World {
                generator: Generator::new(worldgen::Recipe::new(1)).unwrap(),
                cells: crate::Cells::default(),
                parts: crate::parts::Parts::default(),
                trunks: crate::Trunks::default(),
            }
        }

        fn built(&self) -> Built<'_> {
            Built {
                cells: &self.cells,
                parts: &self.parts,
                trunks: &self.trunks,
            }
        }

        fn ground(&self) -> Hooves<'_> {
            Hooves {
                generator: &self.generator,
                built: self.built(),
            }
        }

        fn step(&self, animals: &mut Animals, now_s: f64, near: SurfacePoint, bodies: Bodies<'_>) {
            animals.update(
                1.0 / 60.0,
                now_s,
                Some(near),
                bodies,
                &self.generator,
                self.built(),
            );
        }

        fn near(&self, at: DVec3) -> SurfacePoint {
            self.generator
                .sphere()
                .blocks()
                .surface_point(at.to_array())
        }

        /// Land `far_m` from `from`, along the first of sixteen ways that
        /// is over the sea.
        fn land_off(&self, from: DVec3, far_m: f64) -> DVec3 {
            let up = from.normalize();
            let east = up.any_orthonormal_vector();
            let sea_m = self.generator.sphere().radius_m() + SHORE_M;
            (0..16)
                .map(|i| {
                    let turn = f64::from(i) * core::f64::consts::TAU / 16.0;
                    let way = glam::DQuat::from_axis_angle(up, turn) * east;
                    self.ground().under(from + way * far_m).point
                })
                .find(|at| at.length() > sea_m)
                .expect("land off")
        }
    }

    const NONE: Bodies<'static> = Bodies {
        riders: &[],
        on_foot: &[],
    };

    fn rider(who: Option<u32>, number: u64, at: DVec3) -> Rider {
        let up = at.normalize();
        Rider {
            who,
            number,
            at,
            facing: up.cross(up.any_orthonormal_vector()),
            speed_mps: 0.0,
            balk_s: None,
            aloft: who.is_none().then_some(false),
            leaps: false,
            wings: false,
        }
    }

    fn drawn(animals: &Animals, number: u64) -> Vec<Near> {
        animals
            .near()
            .iter()
            .filter(|near| near.number == number)
            .copied()
            .collect()
    }

    /// A peer claiming the animal this body rides, 50 m off: each rides its
    /// own, and this body's saddle stays over its own feet.
    #[test]
    fn two_riders_of_one_animal_ride_two_and_mine_stays_under_me() {
        let world = World::new();
        let spawn = crate::spawn_point(&world.generator);
        let mut animals = Animals::default();
        world.step(&mut animals, 1e3, spawn, NONE);
        let horse = animals.near()[0];
        let theirs = world.land_off(horse.at, 50.0);
        for _ in 0..30 {
            let riders = [
                rider(None, horse.number, horse.at),
                rider(Some(7), horse.number, theirs),
            ];
            let bodies = Bodies {
                riders: &riders,
                on_foot: &[],
            };
            world.step(&mut animals, 1e3, spawn, bodies);
        }
        let up = horse.at.normalize();
        let over = |saddle: DAffine3, feet: DVec3| {
            let off = saddle.translation - feet;
            ((off - up * off.dot(up)).length(), off.dot(up))
        };
        let (along_m, up_m) = over(animals.saddle(None).unwrap(), horse.at);
        assert!(
            along_m < 1.0 && (0.5..3.0).contains(&up_m),
            "{along_m} m along, {up_m} m up"
        );
        let (along_m, up_m) = over(animals.saddle(Some(7)).unwrap(), theirs);
        assert!(
            along_m < 1.0 && (0.5..3.0).contains(&up_m),
            "{along_m} m along, {up_m} m up"
        );
        let ridden = drawn(&animals, horse.number);
        assert_eq!(ridden.len(), 2, "{ridden:?}");
        assert!(ridden.iter().all(|near| near.ridden));
        // The peer leaves: one horse, under this body.
        let riders = [rider(None, horse.number, horse.at)];
        let bodies = Bodies {
            riders: &riders,
            on_foot: &[],
        };
        world.step(&mut animals, 1e3, spawn, bodies);
        assert_eq!(drawn(&animals, horse.number).len(), 1);
        assert!(animals.left.is_empty());
    }

    /// A peer's mount is drawn with its wings out exactly when its stance
    /// says it flies, as [`crate::Client`] reads `wings` from the gait: a
    /// pegasus 20 m up under a flying peer spreads them, and under one
    /// whose gait is anything else keeps them folded.
    #[test]
    fn a_peers_pegasus_spreads_its_wings_only_when_its_stance_flies() {
        let world = World::new();
        let spawn = crate::spawn_point(&world.generator);
        let mut animals = Animals::default();
        world.step(&mut animals, 1e3, spawn, NONE);
        let horses: Vec<Near> = animals.near().to_vec();
        let mut pegasi = 0;
        for horse in horses {
            let mut grown = [0.0; 2];
            for (flies, grown) in [true, false].into_iter().zip(&mut grown) {
                let mut animals = Animals::default();
                world.step(&mut animals, 1e3, spawn, NONE);
                for _ in 0..120 {
                    let riders = [Rider {
                        wings: flies,
                        ..rider(
                            Some(7),
                            horse.number,
                            horse.at * (1.0 + 20.0 / horse.at.length()),
                        )
                    }];
                    let bodies = Bodies {
                        riders: &riders,
                        on_foot: &[],
                    };
                    world.step(&mut animals, 1e3, spawn, bodies);
                }
                let wings = animals
                    .ridden
                    .get(&Some(7))
                    .and_then(|body| body.animal.wings().copied());
                *grown = wings.map_or(-1.0, |wings| wings.grown);
            }
            // A horse born without wings has none to grow either way.
            if grown[0] >= 0.0 {
                pegasi += 1;
                assert!(grown[0] > 0.9 && grown[1] < 0.1, "{grown:?}");
            }
        }
        assert!(pegasi > 0, "a pegasus near the spawn");
    }

    /// A peer's pegasus takes off as its rider's does, from the stance's
    /// gait alone: it gathers on the ground from the stance that turned to
    /// flight, springs off its legs as the body rises, once, and opens its
    /// wings only at the spring's top.
    #[test]
    fn a_peers_pegasus_takes_off_by_its_gait_and_opens_its_wings_at_the_top() {
        let world = World::new();
        let spawn = crate::spawn_point(&world.generator);
        let winged = gaia::fauna::horse::HORSE.wings.expect("a horse has wings");
        let spring = crate::paces::Spring::from(winged.flight.spring);
        let open_s = winged.wing.open_s;
        let beat_s = 1.0 / winged.wing.beat_hz.0;
        let frames = ((spring.top_s() + open_s + beat_s) * 60.0).round() as usize;
        let mut pegasi = 0;
        let mut herd = Animals::default();
        world.step(&mut herd, 1e3, spawn, NONE);
        for horse in herd.near().to_vec() {
            let ground = world.land_off(horse.at, 50.0);
            let up = ground.normalize();
            let mut animals = Animals::default();
            world.step(&mut animals, 1e3, spawn, NONE);
            let standing = [rider(Some(7), horse.number, ground)];
            for _ in 0..60 {
                let bodies = Bodies {
                    riders: &standing,
                    on_foot: &[],
                };
                world.step(&mut animals, 1e3, spawn, bodies);
            }
            let grown = |animals: &Animals| {
                let body = animals.ridden.get(&Some(7))?;
                body.animal.wings().map(|wings| wings.grown)
            };
            if grown(&animals).is_none() {
                continue;
            }
            pegasi += 1;
            let (mut sprang, mut beats) = (Vec::new(), Vec::new());
            for frame in 1..=frames {
                let s = frame as f64 / 60.0;
                // Its rider's spring, as the stances bring it.
                let over_m = spring.height_at(s, spring.height_m);
                let riders = [Rider {
                    wings: true,
                    ..rider(Some(7), horse.number, ground + up * over_m)
                }];
                let bodies = Bodies {
                    riders: &riders,
                    on_foot: &[],
                };
                world.step(&mut animals, 1e3, spawn, bodies);
                let grown = grown(&animals).expect("its wings");
                if s < spring.top_s() - 0.5 / 60.0 {
                    assert!(grown < 0.01, "{s} s: {grown} grown before the top");
                }
                if s > spring.top_s() + open_s + 0.05 {
                    assert!(grown > 0.95, "{s} s: {grown} grown past the top");
                }
                for &(who, _, take_off) in animals.sprang() {
                    assert_eq!((who, take_off), (Some(7), true));
                    sprang.push(s);
                }
                beats.extend(animals.wingbeats().iter().map(|_| s));
            }
            assert_eq!(sprang.len(), 1, "it springs once: {sprang:?}");
            // Opened raised, its first downstroke takes the air within a
            // beat of the wings being out, never before the top.
            let first_s = beats.first().copied().unwrap_or(f64::INFINITY);
            assert!(
                (spring.top_s()..spring.top_s() + open_s + beat_s).contains(&first_s),
                "first beat at {first_s} s"
            );
            let late_s = sprang[0] - spring.gather_s;
            assert!((0.0..0.1).contains(&late_s), "springs at {} s", sprang[0]);
        }
        assert!(pegasi > 0, "a pegasus near the spawn");
    }

    /// A peer that only ever stood 600 m from the herd sees a horse ridden
    /// up and stepped down from: it stays drawn where it was left, and so
    /// it does wherever the body looking is, its sector or another.
    #[test]
    fn a_horse_left_far_from_its_herd_is_drawn_by_who_saw_it_ridden() {
        let world = World::new();
        let spawn = crate::spawn_point(&world.generator);
        let mut herd = Animals::default();
        world.step(&mut herd, 1e3, spawn, NONE);
        let horse = herd.near()[0];
        let left = world.land_off(horse.at, 600.0);
        let mut peer = Animals::default();
        let riders = [rider(Some(3), horse.number, left)];
        let riding = Bodies {
            riders: &riders,
            on_foot: &[],
        };
        for _ in 0..10 {
            world.step(&mut peer, 1e3, world.near(left), riding);
        }
        assert_eq!(drawn(&peer, horse.number).len(), 1);
        let beside = [world.land_off(left, 1.5)];
        let watching = Bodies {
            riders: &[],
            on_foot: &beside,
        };
        // Over the world from it, on another sector.
        let antipode = world.near(-left);
        assert_ne!(antipode.sector, world.near(left).sector);
        for (frame, near) in [world.near(left), antipode]
            .into_iter()
            .cycle()
            .take(60)
            .enumerate()
        {
            world.step(&mut peer, 1e3 + frame as f64 / 60.0, near, watching);
            let at = drawn(&peer, horse.number);
            assert_eq!(at.len(), 1, "frame {frame}");
            assert!((at[0].at - left).length() < 2.0, "frame {frame}");
        }
    }

    /// By a seam, the regions in reach are on both sides of it; out on the
    /// planet, those of the regions about the body alone are kept, however
    /// far it went.
    #[test]
    fn herds_are_placed_within_reach_and_on_both_sides_of_a_seam() {
        let world = World::new();
        let spawn = crate::spawn_point(&world.generator);
        let by_seam = SurfacePoint::new(spawn.sector, 2.0, spawn.v);
        let sectors: Vec<Sector> = Animals::in_view(by_seam, &world.generator)
            .iter()
            .map(|region| region.sector)
            .collect();
        assert!(sectors.contains(&spawn.sector));
        assert!(sectors.iter().any(|&sector| sector != spawn.sector));

        let mut animals = Animals::default();
        let side = f64::from(world.generator.sphere().blocks().side());
        let step_b = 300.0 / BLOCK_M;
        for i in 0..20 {
            for j in 0..20 {
                let u = (spawn.u + (f64::from(i) - 10.0) * step_b).clamp(1.0, side - 1.0);
                let v = (spawn.v + (f64::from(j) - 10.0) * step_b).clamp(1.0, side - 1.0);
                let at = SurfacePoint::new(spawn.sector, u, v);
                world.step(&mut animals, 1e3, at, NONE);
                let reach = Animals::in_view(at, &world.generator).len();
                assert!(
                    animals.herds.len() <= reach,
                    "{} kept, {reach} in reach",
                    animals.herds.len()
                );
            }
        }
    }

    /// Every sector's regions fit the number.
    #[test]
    fn every_region_has_a_number() {
        let generator = Generator::new(worldgen::Recipe::new(1)).unwrap();
        let side = f64::from(generator.sphere().blocks().side());
        for kind in fauna::KINDS {
            let regions = side / (kind.herding.region_m / BLOCK_M);
            assert!(regions <= f64::from(1 << REGION_BITS), "{}", kind.name);
        }
    }
}
