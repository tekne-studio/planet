//! What one animal looks like, drawn from its kind and a seed: its coat and
//! marks, its size and build, its mane and tail.
//!
//! What the seed and its herd give an animal are its [`Traits`], values a
//! pet keeps whatever a rule becomes; its look is drawn from them on its
//! kind's body. Colours are worked out once, when the look is made: a
//! frame only reads them.

use glam::{DVec3, Vec3};
use serde::{Deserialize, Serialize};

use crate::fauna::NAME_BYTES;
use crate::fauna::kind::Kind;
use crate::fauna::random::Random;
use crate::fauna::rig::{LEGS, Part, Role};

/// A coat a kind is born in: the colour of each part of it, linear RGB.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Coat {
    pub name: &'static str,
    /// How often it is born, against the kind's other coats.
    pub weight: u32,
    pub hair: Vec3,
    pub mane: Vec3,
    /// The lower legs.
    pub points: Vec3,
    pub muzzle: Vec3,
    pub hoof: Vec3,
    /// White in large patches over the hair.
    pub patched: bool,
    /// How likely each mark is to show on it at all: a white coat shows
    /// none.
    pub marked: f64,
}

impl Coat {
    /// How light its hair is: its luminance, linear, 0 black to 1 white.
    pub fn lightness(&self) -> f32 {
        self.hair.dot(Vec3::new(0.2126, 0.7152, 0.0722))
    }
}

/// A build a kind is born in: how big, how broad, how much hair.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Build {
    pub weight: u32,
    /// Height against the rig's, and width against it.
    pub size: (f64, f64),
    pub build: (f64, f64),
    /// How tall the mane stands on the crest, metres at size 1, and the
    /// tail's hair.
    pub mane_m: (f64, f64),
    pub tail_m: (f64, f64),
    /// Long hair over the feet.
    pub feathers: bool,
    /// The coats it is born in, by name; none says every one.
    pub coats: &'static [&'static str],
}

/// White on the face: boxes over the head, in the poll's frame.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Face {
    /// What a pet keeps of it: its boxes are its kind's head's.
    pub name: &'static str,
    pub weight: u32,
    pub parts: &'static [Part],
    /// The whole head white, the muzzle bare skin.
    pub bald: bool,
}

/// Where a coat in patches is white: a few round blots over the body's side,
/// at size 1.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Patches {
    pub blots: (usize, usize),
    pub radius_m: (f64, f64),
    pub y: (f64, f64),
    pub z: (f64, f64),
}

/// The marks a kind is born with, and their colours.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Marks {
    pub faces: &'static [Face],
    /// How far up each leg is white, by weight: 0 none, 1 the pastern, 2 the
    /// lower leg too, 3 above the knee.
    pub socks: &'static [(u32, u8)],
    pub patches: Patches,
    pub white: Vec3,
    /// The foot under a white sock.
    pub pale_hoof: Vec3,
    /// Bare skin, at a bald face's muzzle.
    pub skin: Vec3,
    pub eye: Vec3,
}

/// What an animal is born with, as values: everything its seed and its
/// herd's rule give it that is seen. A pet keeps them as it was tamed, so
/// no later rule, range or table redraws it; its kind's body, the rig and
/// the colour of a white mark, stays its kind's.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Traits {
    /// The coat, by its name, and its colours as it was born in it, linear
    /// RGB: the hair, the mane and tail, the lower legs, the muzzle, the
    /// hooves.
    pub coat: String,
    pub hair: Vec3,
    pub mane: Vec3,
    pub points: Vec3,
    pub muzzle: Vec3,
    pub hoof: Vec3,
    /// White on the face, by its name in the kind; empty for none.
    pub face: String,
    /// How far up each leg is white, 0 to 3.
    pub socks: [u8; LEGS],
    /// White blots over the body's side: a coat in patches has some.
    pub blots: Vec<Blot>,
    /// Height against the rig's, and width against it.
    pub size: f64,
    pub build: f64,
    /// How tall the mane stands, metres at size 1, whether it falls, and
    /// the tail's hair.
    pub mane_m: f64,
    pub mane_falls: bool,
    pub tail_m: f64,
    /// Long hair over the feet.
    pub feathers: bool,
    /// Born with wings, as its herd says: then its coat is that one's hair
    /// all over, with no mark.
    pub wings: bool,
    /// How far its voice sits from its kind's, cents, past its size. None
    /// in traits kept from before it was drawn: the world fills it from the
    /// herd, and until then no animal is drawn in them.
    #[serde(default)]
    pub voice_cents: Option<f64>,
}

/// A round white blot of a coat in patches, on the body's side at size 1.
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
pub struct Blot {
    pub y: f64,
    pub z: f64,
    pub radius_m: f64,
}

/// One animal's look: its traits, drawn on its kind's body.
#[derive(Clone, Debug)]
pub struct Look {
    pub traits: Traits,
    pub(crate) paint: Paint,
}

/// A box of the trunk, at size 1 and build 1.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Cell {
    pub centre: DVec3,
    pub size: DVec3,
    pub color: Vec3,
}

/// The colour of every box an animal draws.
#[derive(Clone, Debug)]
pub(crate) struct Paint {
    pub trunk: Vec<Cell>,
    /// The neck, root to poll: one box, or several for a coat in patches.
    pub neck: Vec<Vec3>,
    pub head: Vec<(Part, Vec3)>,
    pub ear: Vec3,
    pub eye: Vec3,
    pub mane: Vec3,
    pub tail: Vec3,
    /// Each leg's upper bone, lower bone and pastern.
    pub legs: [[Vec3; 3]; LEGS],
    pub hooves: [Vec3; LEGS],
    pub feathers: [Vec3; LEGS],
    /// The wings' coverts, secondaries and primaries.
    pub wings: [Vec3; 3],
}

impl Traits {
    /// An animal of a kind, drawn from a seed: its build first, so a big
    /// one is broad too, then its coat among those of its build, then its
    /// marks. Born with wings in the coat `wings` names, as its herd says
    /// ([`crate::fauna::Herd::wings_of`]): its size and hair are the
    /// seed's as ever, its coat that one, all over.
    pub fn drawn(kind: &Kind, seed: u64, wings: Option<&str>) -> Traits {
        let mut random = Random::of(&[seed, 0x6c6f_6f6b]);
        let build = kind.builds[random
            .weighted(kind.builds.iter().map(|build| build.weight))
            .unwrap_or(0)];
        // A coat outside what a wingless one wears is the winged's alone.
        let plain = |coat: &Coat| {
            kind.wings
                .is_none_or(|winged| (winged.plain.0..=winged.plain.1).contains(&coat.lightness()))
        };
        let born_in = |coat: &Coat| {
            (build.coats.is_empty() || build.coats.contains(&coat.name)) && plain(coat)
        };
        // A build born in no coat a wingless one wears takes the first it
        // does wear, never a winged one's: the kind's tests keep it unused.
        let coat = random
            .weighted(
                kind.coats
                    .iter()
                    .map(|coat| if born_in(coat) { coat.weight } else { 0 }),
            )
            .or_else(|| kind.coats.iter().position(plain))
            .map_or(kind.coats[0], |coat| kind.coats[coat]);
        let marks = &kind.marks;
        let face = match random.chance(coat.marked) {
            true => random
                .weighted(marks.faces.iter().map(|face| face.weight))
                .map(|face| marks.faces[face]),
            false => None,
        };
        let socks = core::array::from_fn(|_| match random.chance(coat.marked) {
            true => random
                .weighted(marks.socks.iter().map(|(weight, _)| *weight))
                .map_or(0, |sock| marks.socks[sock].1),
            false => 0,
        });
        let size = random.range(build.size);
        let width = random.range(build.build);
        let mane_m = random.range(build.mane_m);
        let mane_falls = mane_m > 0.08 && random.chance(0.7);
        let tail_m = random.range(build.tail_m);
        let blots = match coat.patched {
            true => {
                let p = &marks.patches;
                (0..random.count(p.blots))
                    .map(|_| {
                        let (y, z) = (random.range(p.y), random.range(p.z));
                        let radius_m = random.range(p.radius_m);
                        Blot { y, z, radius_m }
                    })
                    .collect()
            }
            false => Vec::new(),
        };
        // Drawn last, so every trait drawn before it stays as it was.
        let voice_cents = Some(random.range(kind.voice_cents));
        let born = Traits {
            coat: coat.name.to_owned(),
            hair: coat.hair,
            mane: coat.mane,
            points: coat.points,
            muzzle: coat.muzzle,
            hoof: coat.hoof,
            face: face.map(|face| face.name).unwrap_or_default().to_owned(),
            socks,
            blots,
            size,
            build: width,
            mane_m,
            mane_falls,
            tail_m,
            feathers: build.feathers,
            wings: false,
            voice_cents,
        };
        // Winged, it wears its coat's hair all over, mane, muzzle, legs,
        // hooves and wings, with no mark: one colour, its eyes aside.
        let winged = kind
            .wings
            .and(wings)
            .and_then(|name| kind.coats.iter().find(|coat| coat.name == name));
        match winged {
            Some(pure) => Traits {
                coat: pure.name.to_owned(),
                hair: pure.hair,
                mane: pure.hair,
                points: pure.hair,
                muzzle: pure.hair,
                hoof: pure.hair,
                face: String::new(),
                socks: [0; LEGS],
                blots: Vec::new(),
                wings: true,
                ..born
            },
            None => born,
        }
    }

    /// Whether these are traits an animal of `kind` can be drawn in: every
    /// number finite and within what any build of the kind draws, widened
    /// by [`LEEWAY`] so a pet outlives a later table; every name short;
    /// any colour. Never today's names, which a pet outlives too.
    pub fn fits(&self, kind: &Kind) -> bool {
        let colour = |c: Vec3| c.is_finite() && c.min_element() >= 0.0 && c.max_element() <= 1.0;
        let within = |x: f64, (from, to): (f64, f64)| x.is_finite() && (from..=to).contains(&x);
        let widest = |range: fn(&Build) -> (f64, f64)| {
            let (from, to) = kind.builds.iter().map(range).fold(
                (f64::INFINITY, f64::NEG_INFINITY),
                |(from, to), (low, high)| (from.min(low), to.max(high)),
            );
            widened((from, to))
        };
        let blots = &kind.marks.patches;
        let most = blots.blots.1 as f64 + (blots.blots.1 - blots.blots.0) as f64 * LEEWAY;
        let (y, z, radius_m) = (widened(blots.y), widened(blots.z), widened(blots.radius_m));
        [self.hair, self.mane, self.points, self.muzzle, self.hoof]
            .into_iter()
            .all(colour)
            && self.coat.len() <= NAME_BYTES
            && self.face.len() <= NAME_BYTES
            && self.socks.iter().all(|&sock| sock <= 3)
            && self.blots.len() as f64 <= most
            && self.blots.iter().all(|blot| {
                within(blot.y, y) && within(blot.z, z) && within(blot.radius_m, radius_m)
            })
            && within(self.size, widest(|build| build.size))
            && within(self.build, widest(|build| build.build))
            && within(self.mane_m, widest(|build| build.mane_m))
            && within(self.tail_m, widest(|build| build.tail_m))
            && self
                .voice_cents
                .is_some_and(|cents| within(cents, widened(kind.voice_cents)))
    }
}

/// How far past what its kind draws a pet's numbers may lie, as a share of
/// the range on each side: room for a later table, never a 4 m horse.
const LEEWAY: f64 = 0.5;

/// A range of a kind, widened by [`LEEWAY`] of its span on each side;
/// never below zero for one that starts at or above it.
fn widened((from, to): (f64, f64)) -> (f64, f64) {
    let room = (to - from) * LEEWAY;
    let low = match from >= 0.0 {
        true => (from - room).max(0.0),
        false => from - room,
    };
    (low, to + room)
}

impl Look {
    /// An animal of a kind, drawn from a seed, with wings in the coat
    /// `wings` names: [`Traits::drawn`], drawn on its kind's body.
    pub fn new(kind: &Kind, seed: u64, wings: Option<&str>) -> Look {
        Look::of(kind, Traits::drawn(kind, seed, wings))
    }

    /// An animal of a kind in the traits it keeps.
    pub fn of(kind: &Kind, traits: Traits) -> Look {
        let paint = Paint::new(kind, &traits);
        Look { traits, paint }
    }
}

impl Paint {
    fn new(kind: &Kind, coat: &Traits) -> Paint {
        let (rig, marks) = (&kind.rig, &kind.marks);
        let (socks, feathers) = (coat.socks, coat.feathers);
        let face = marks.faces.iter().find(|face| face.name == coat.face);
        let white_at = |at: DVec3| {
            coat.blots
                .iter()
                .any(|blot| (at - DVec3::new(0.0, blot.y, blot.z)).length() < blot.radius_m)
        };
        // A coat with blots is cut into cells, so each takes its own white.
        let patched = !coat.blots.is_empty();
        let or_white = |color: Vec3, white: bool| if white { marks.white } else { color };

        let mut trunk = Vec::new();
        for section in rig.trunk {
            let [cuts, rows] = match patched {
                true => section.cuts,
                false => [1, 1],
            };
            for cut in 0..cuts {
                for row in 0..rows {
                    let along = |at: usize| {
                        section.from_z + (section.to_z - section.from_z) * at as f64 / cuts as f64
                    };
                    let up = |at: usize| {
                        section.bottom + (section.top - section.bottom) * at as f64 / rows as f64
                    };
                    let (z0, z1, y0, y1) = (along(cut), along(cut + 1), up(row), up(row + 1));
                    let centre = DVec3::new(0.0, (y0 + y1) / 2.0, (z0 + z1) / 2.0);
                    trunk.push(Cell {
                        centre,
                        size: DVec3::new(section.width, y1 - y0, z1 - z0),
                        color: or_white(coat.hair, white_at(centre)),
                    });
                }
            }
        }
        let neck = &rig.neck;
        let cuts = if patched { neck.cuts } else { 1 };
        let rest = DVec3::new(0.0, 0.6, -0.8);
        let neck_colors: Vec<Vec3> = (0..cuts)
            .map(|cut| {
                let along = (cut as f64 + 0.5) / cuts as f64 * neck.length_m;
                or_white(coat.hair, white_at(neck.root + rest * along))
            })
            .collect();
        let bald = face.is_some_and(|face| face.bald);
        let hair = or_white(coat.hair, bald);
        let muzzle = if bald { marks.skin } else { coat.muzzle };
        let mut head: Vec<(Part, Vec3)> = rig
            .head
            .iter()
            .map(|part| {
                let color = match part.role {
                    Role::Muzzle => muzzle,
                    Role::Eye => marks.eye,
                    Role::Mark => marks.white,
                    Role::Hair => hair,
                };
                (*part, color)
            })
            .collect();
        if let Some(face) = face.filter(|face| !face.bald) {
            head.extend(face.parts.iter().map(|part| (*part, marks.white)));
        }
        let mane_white = patched && neck_colors.iter().all(|&c| c == marks.white);
        let tail_white = patched && white_at(rig.tail.root);
        let legs = core::array::from_fn(|leg| {
            let sock = socks[leg];
            let rest = rig.leg(leg);
            let thigh = DVec3::new(0.0, rest.root.y, rest.root.z);
            let upper_white = sock >= 3 || (patched && white_at(thigh));
            [
                or_white(coat.hair, upper_white),
                or_white(coat.points, sock >= 2),
                or_white(coat.points, sock >= 1),
            ]
        });
        let mane = or_white(coat.mane, mane_white);
        Paint {
            trunk,
            neck: neck_colors,
            head,
            ear: hair,
            eye: marks.eye,
            mane,
            tail: or_white(coat.mane, tail_white),
            legs,
            hooves: core::array::from_fn(|leg| match socks[leg] >= 1 {
                true => marks.pale_hoof,
                false => coat.hoof,
            }),
            feathers: core::array::from_fn(|leg| match socks[leg] >= 1 || !feathers {
                true => marks.white,
                false => mane,
            }),
            // The coverts the coat's, the flight feathers toward its mane:
            // one colour on a winged one, whose mane is its hair.
            wings: [
                coat.hair,
                coat.hair.lerp(coat.mane, 0.4),
                coat.hair.lerp(coat.mane, 0.8),
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::fauna::horse::HORSE;

    use super::*;

    /// The same seed is the same animal; another is, nearly always, another.
    #[test]
    fn a_seed_is_one_animal() {
        let a = Look::new(&HORSE, 11, None);
        let b = Look::new(&HORSE, 11, None);
        assert_eq!(a.traits, b.traits);
        let coats: std::collections::HashSet<_> = (0..200)
            .map(|seed| Look::new(&HORSE, seed, None).traits.coat)
            .collect();
        assert!(coats.len() >= 8, "{coats:?}");
    }

    /// One born with wings is its coat's hair all over, wings, mane,
    /// muzzle and hooves too, with no mark; the rest of its look is the
    /// seed's as without wings.
    #[test]
    fn a_winged_one_is_one_colour_all_over() {
        for seed in 0..300 {
            let bare = Look::new(&HORSE, seed, None);
            assert!(!bare.traits.wings);
            for coat in ["white", "black"] {
                let look = Look::new(&HORSE, seed, Some(coat));
                assert!(look.traits.wings);
                assert_eq!(look.traits.coat, coat);
                assert_eq!(look.traits.socks, [0; LEGS]);
                assert_eq!(
                    (
                        look.traits.size,
                        look.traits.build,
                        look.traits.mane_m,
                        look.traits.tail_m
                    ),
                    (
                        bare.traits.size,
                        bare.traits.build,
                        bare.traits.mane_m,
                        bare.traits.tail_m
                    )
                );
                let hair = HORSE.coats.iter().find(|c| c.name == coat).unwrap().hair;
                let paint = &look.paint;
                let mut colors: Vec<Vec3> = paint.trunk.iter().map(|cell| cell.color).collect();
                colors.extend(&paint.neck);
                colors.extend(
                    paint
                        .head
                        .iter()
                        .filter(|(part, _)| part.role != Role::Eye)
                        .map(|(_, color)| *color),
                );
                colors.extend([paint.ear, paint.mane, paint.tail]);
                colors.extend(paint.legs.iter().flatten());
                colors.extend(paint.hooves);
                colors.extend(paint.feathers.iter().filter(|_| look.traits.feathers));
                colors.extend(paint.wings);
                assert!(
                    colors.iter().all(|&c| c.abs_diff_eq(hair, 1e-6)),
                    "{coat} look {seed}"
                );
            }
        }
        // A kind without wings is born without, whatever it is asked.
        let bare = Kind {
            wings: None,
            ..HORSE
        };
        assert!(!Look::new(&bare, 1, Some("white")).traits.wings);
    }

    /// White and black are the winged's alone: no wingless horse's hair is
    /// lighter or darker than its kind's plain range, every coat a winged
    /// one wears lies outside it, and each is lighter or darker than the
    /// plain by a clear step.
    #[test]
    fn white_and_black_are_the_winged_alone() {
        let winged = HORSE.wings.unwrap();
        let (darkest, lightest) = winged.plain;
        let lightness = |name: &str| {
            HORSE
                .coats
                .iter()
                .find(|c| c.name == name)
                .unwrap()
                .lightness()
        };
        let mut seen = (f32::MAX, f32::MIN);
        for seed in 0..5000 {
            let look = Look::new(&HORSE, seed, None);
            let light = lightness(&look.traits.coat);
            assert!(
                (darkest..=lightest).contains(&light),
                "look {seed}: {} at {light}",
                look.traits.coat
            );
            seen = (seen.0.min(light), seen.1.max(light));
        }
        for coat in winged.herds.iter().flat_map(|set| set.iter()) {
            let light = lightness(coat);
            assert!(
                light < darkest / 2.0 || light > lightest * 1.3,
                "{coat} at {light}"
            );
        }
        // The plain range is worn near its ends: it says what is born.
        assert!(
            seen.0 < darkest * 1.5 && seen.1 > lightest * 0.85,
            "{seen:?}"
        );
    }

    /// A build born only in some coats is never born in another.
    #[test]
    fn a_build_keeps_its_coats() {
        for seed in 0..2000 {
            let look = Look::new(&HORSE, seed, None);
            if look.traits.feathers {
                let draught = HORSE.builds.iter().find(|b| b.feathers).unwrap();
                assert!(
                    draught.coats.contains(&look.traits.coat.as_str()),
                    "{}",
                    look.traits.coat
                );
            }
        }
    }

    /// Kept traits outlive the rules that drew them: under a kind whose
    /// coats and builds have changed, an animal drawn from its seed is
    /// another, and one drawn in its traits is the same, box for box.
    #[test]
    fn kept_traits_outlive_the_rules() {
        const COATS: [Coat; 2] = {
            let bay = HORSE.coats[4];
            let lifted = Coat {
                name: "lifted",
                hair: Vec3::new(0.3, 0.3, 0.3),
                ..bay
            };
            [bay, lifted]
        };
        const BUILDS: [Build; 1] = [Build {
            size: (1.3, 1.4),
            build: (1.2, 1.3),
            coats: &[],
            ..HORSE.builds[0]
        }];
        let later = Kind {
            coats: &COATS,
            builds: &BUILDS,
            ..HORSE
        };
        for seed in 0..200 {
            for wings in [None, Some("white"), Some("black")] {
                let tamed = Look::new(&HORSE, seed, wings);
                assert!(tamed.traits.fits(&HORSE));
                assert_ne!(Look::new(&later, seed, wings).traits, tamed.traits);
                let kept = Look::of(&later, tamed.traits.clone());
                assert_eq!(kept.traits, tamed.traits);
                assert_eq!(format!("{:?}", kept.paint), format!("{:?}", tamed.paint));
            }
        }
    }

    /// Traits past what the kind bears are refused, whoever hands them:
    /// a number not finite, a colour missing, too many blots or too wide,
    /// a long name, a horse four times its size.
    #[test]
    fn traits_past_the_kind_do_not_fit() {
        let born = Traits::drawn(&HORSE, 3, None);
        assert!(born.fits(&HORSE));
        const BLOT: Blot = Blot {
            y: 1.2,
            z: 0.0,
            radius_m: 0.3,
        };
        type Wrong = fn(&mut Traits);
        let refused: [(&str, Wrong); 11] = [
            ("a size not a number", |t| t.size = f64::NAN),
            ("an endless tail", |t| t.tail_m = f64::INFINITY),
            ("a missing colour", |t| t.hair = Vec3::NAN),
            ("a colour past white", |t| t.mane.x = 1.5),
            ("a sock of 4", |t| t.socks[2] = 4),
            ("seven blots", |t| t.blots = vec![BLOT; 7]),
            ("a blot of 2 m", |t| {
                t.blots = vec![Blot {
                    radius_m: 2.0,
                    ..BLOT
                }]
            }),
            ("a 33-byte name", |t| t.coat = "a".repeat(33)),
            ("size 4", |t| t.size = 4.0),
            ("an 8 m mane", |t| t.mane_m = 8.0),
            ("a 16 m tail", |t| t.tail_m = 16.0),
        ];
        for (what, wrong) in refused {
            let mut traits = born.clone();
            wrong(&mut traits);
            assert!(!traits.fits(&HORSE), "{what} fits");
        }
        // Within the leeway, a later table's horse still fits.
        let mut small = born.clone();
        small.size = 0.5;
        small.blots = vec![BLOT; 6];
        assert!(small.fits(&HORSE));
    }

    /// Every build of a winged kind is born in some coat a wingless one
    /// wears, so [`Traits::drawn`] never falls back.
    #[test]
    fn every_build_wears_a_plain_coat() {
        for kind in crate::fauna::KINDS {
            let Some(winged) = kind.wings else { continue };
            for build in kind.builds {
                let worn = kind.coats.iter().any(|coat| {
                    coat.weight > 0
                        && (build.coats.is_empty() || build.coats.contains(&coat.name))
                        && (winged.plain.0..=winged.plain.1).contains(&coat.lightness())
                });
                assert!(worn, "a build of {} wears no plain coat", kind.name);
            }
        }
    }
}
