//! The horse, as a table: a riding horse's rig, its walk, trot and gallop,
//! ten coats and their marks, pony to draught, a herd on grass and in
//! the wood, a rider on its back, and a pegasus in every herd: wings that
//! fly it, white or black all over.

use glam::{DVec3, Vec3};

use crate::fauna::gait::{Gait, Gaits};
use crate::fauna::kind::{
    Backing, BySize, Carry, Flight, Habitat, Herding, Jump, Kind, Least, Pace, Spring, Winged,
};
use crate::fauna::look::{Build, Coat, Face, Marks, Patches};
use crate::fauna::rig::{Ears, Leg, Neck, Part, Rig, Role, Section, Tail};
use crate::fauna::wing::{Row, Wing};

// Linear RGB.
const WHITE_HAIR: Vec3 = Vec3::new(0.74, 0.72, 0.67);
const WHITE_MANE: Vec3 = Vec3::new(0.82, 0.81, 0.77);
const WHITE_MARK: Vec3 = Vec3::new(0.8, 0.8, 0.78);
const GREY_SKIN: Vec3 = Vec3::new(0.12, 0.11, 0.11);
const IRON: Vec3 = Vec3::new(0.2, 0.2, 0.205);
const IRON_DARK: Vec3 = Vec3::new(0.07, 0.07, 0.072);
const BLACK_HAIR: Vec3 = Vec3::new(0.014, 0.013, 0.014);
const DARK_BROWN: Vec3 = Vec3::new(0.13, 0.06, 0.025);
const MEALY: Vec3 = Vec3::new(0.2, 0.1, 0.045);
const BAY: Vec3 = Vec3::new(0.34, 0.1, 0.03);
const BAY_MUZZLE: Vec3 = Vec3::new(0.1, 0.035, 0.012);
const CHESTNUT: Vec3 = Vec3::new(0.48, 0.16, 0.04);
const CHESTNUT_MANE: Vec3 = Vec3::new(0.384, 0.128, 0.032);
const CHESTNUT_MUZZLE: Vec3 = Vec3::new(0.24, 0.09, 0.035);
const FLAXEN: Vec3 = Vec3::new(0.72, 0.55, 0.3);
const PALOMINO: Vec3 = Vec3::new(0.64, 0.43, 0.16);
const PALOMINO_MANE: Vec3 = Vec3::new(0.86, 0.8, 0.66);
const HOOF_DARK: Vec3 = Vec3::new(0.035, 0.03, 0.028);
const HOOF_GREY: Vec3 = Vec3::new(0.1, 0.09, 0.085);
const HOOF_PALE: Vec3 = Vec3::new(0.3, 0.25, 0.18);
const HOOF_AMBER: Vec3 = Vec3::new(0.15, 0.125, 0.09);
const EYE: Vec3 = Vec3::new(0.004, 0.003, 0.003);

/// A coat with every mark it may take.
const fn coat(name: &'static str, hair: Vec3, mane: Vec3, points: Vec3, muzzle: Vec3) -> Coat {
    Coat {
        name,
        weight: 1,
        hair,
        mane,
        points,
        muzzle,
        hoof: HOOF_DARK,
        patched: false,
        marked: 1.0,
    }
}

const COATS: [Coat; 10] = [
    // A grey gone white: the white pegasus's alone, as black is the black
    // one's (`Winged::plain`).
    Coat {
        hoof: HOOF_GREY,
        marked: 0.0,
        ..coat("white", WHITE_HAIR, WHITE_MANE, WHITE_HAIR, GREY_SKIN)
    },
    // An iron grey: dark grey, darker legs and mane.
    Coat {
        marked: 0.2,
        ..coat("grey", IRON, IRON_DARK, IRON_DARK, GREY_SKIN)
    },
    Coat {
        marked: 0.2,
        ..coat("black", BLACK_HAIR, BLACK_HAIR, BLACK_HAIR, BLACK_HAIR)
    },
    // Dark brown, black points, a paler muzzle.
    coat("brown", DARK_BROWN, BLACK_HAIR, BLACK_HAIR, MEALY),
    // Red brown, black mane, tail and lower legs.
    coat("bay", BAY, BLACK_HAIR, BLACK_HAIR, BAY_MUZZLE),
    Coat {
        hoof: HOOF_AMBER,
        ..coat(
            "chestnut",
            CHESTNUT,
            CHESTNUT_MANE,
            CHESTNUT,
            CHESTNUT_MUZZLE,
        )
    },
    // Copper with a pale mane and tail.
    Coat {
        hoof: HOOF_AMBER,
        ..coat("flaxen", CHESTNUT, FLAXEN, CHESTNUT, CHESTNUT_MUZZLE)
    },
    // Gold, a pale mane and tail.
    Coat {
        hoof: HOOF_AMBER,
        ..coat("palomino", PALOMINO, PALOMINO_MANE, PALOMINO, GREY_SKIN)
    },
    // Bay or chestnut and white in large patches.
    Coat {
        patched: true,
        ..coat("skewbald", BAY, BLACK_HAIR, BLACK_HAIR, BAY_MUZZLE)
    },
    Coat {
        patched: true,
        hoof: HOOF_AMBER,
        ..coat("sorrel", CHESTNUT, CHESTNUT_MANE, CHESTNUT, CHESTNUT_MUZZLE)
    },
];

/// White on the forehead, `width` by `length`, and down the nose,
/// `nose` wide.
const fn forehead(width: f64, length: f64) -> Part {
    Part {
        centre: DVec3::new(0.0, 0.142, -0.06 - length / 2.0),
        size: DVec3::new(width, 0.012, length),
        role: Role::Mark,
    }
}

const fn nose(width: f64) -> Part {
    Part {
        centre: DVec3::new(0.0, 0.092, -0.45),
        size: DVec3::new(width, 0.012, 0.26),
        role: Role::Mark,
    }
}

const FACES: [Face; 5] = [
    Face {
        name: "plain",
        weight: 3,
        parts: &[],
        bald: false,
    },
    // A star.
    Face {
        name: "star",
        weight: 2,
        parts: &[forehead(0.07, 0.07)],
        bald: false,
    },
    // A stripe.
    Face {
        name: "stripe",
        weight: 2,
        parts: &[forehead(0.035, 0.22), nose(0.03)],
        bald: false,
    },
    // A blaze.
    Face {
        name: "blaze",
        weight: 2,
        parts: &[forehead(0.09, 0.22), nose(0.075)],
        bald: false,
    },
    Face {
        name: "bald",
        weight: 1,
        parts: &[],
        bald: true,
    },
];

/// Four beats, each hoof alone: left hind, left fore, right hind, right
/// fore. Three hooves are down most of the time.
const WALK: Gait = Gait {
    from_mps: 0.0,
    blend_mps: 0.0,
    hz: 0.72,
    hz_per_mps: 0.14,
    stance_m: 1.0,
    duty: 0.7,
    lands: [0.25, 0.75, 0.0, 0.5],
    lift_m: 0.11,
    bounce_m: 0.012,
    bounces: 2.0,
    sink: 0.31,
    rock: 0.0,
    nod: 0.07,
    nods: 2.0,
    neck: 0.85,
    head: -1.05,
    tail: 0.0,
    beats: &[0.0, 0.25, 0.5, 0.75],
    force: 0.4,
};

/// Two beats, the diagonal pairs together: left fore with right hind, right
/// fore with left hind.
const TROT: Gait = Gait {
    from_mps: 2.7,
    blend_mps: 0.7,
    hz: 1.05,
    hz_per_mps: 0.09,
    stance_m: 1.0,
    duty: 0.42,
    lands: [0.5, 1.0, 0.0, 0.5],
    lift_m: 0.2,
    bounce_m: 0.035,
    bounces: 2.0,
    sink: 0.21,
    rock: 0.0,
    nod: 0.025,
    nods: 2.0,
    neck: 0.8,
    head: -0.95,
    tail: 0.25,
    beats: &[0.0, 0.5],
    force: 0.65,
};

/// Four beats and a flight, right lead, "po-co-tó": the left hind (po),
/// the right hind and the left fore almost together (co), the right fore
/// (tó), then no hoof down. Short, short, long, rest: the body comes down
/// on the leading fore, nose low, and gathers in the flight. Its middle
/// pair is the trot's diagonal and its first and last the trot's other,
/// drawn apart, so the blend moves each leg the least.
const GALLOP: Gait = Gait {
    from_mps: 6.3,
    blend_mps: 1.0,
    hz: 1.5,
    hz_per_mps: 0.08,
    stance_m: 1.3,
    duty: 0.32,
    lands: [0.53, 0.71, 0.29, 0.47],
    lift_m: 0.26,
    bounce_m: 0.06,
    bounces: 1.0,
    sink: 0.76,
    rock: 0.09,
    nod: 0.12,
    nods: 1.0,
    neck: 0.5,
    head: -0.7,
    tail: 0.95,
    // Heard in three, the middle pair as one, and the flight silent.
    beats: &[0.29, 0.5, 0.71],
    force: 1.0,
};

/// The gallop stretched out, as a horse races: the same four beats and
/// flight, a longer stride and a longer flight, neck and head reached
/// forward, at the stride's rate of the gallop's top.
const RUN: Gait = Gait {
    from_mps: 11.0,
    blend_mps: 2.0,
    hz: 1.5,
    hz_per_mps: 0.06,
    // Longer than the legs span, as the gallop's, by choice: a stance fitted
    // to the legs was tried in the world beside this one, and this one kept.
    stance_m: 1.7,
    duty: 0.28,
    lift_m: 0.3,
    bounce_m: 0.05,
    rock: 0.07,
    nod: 0.1,
    neck: 0.3,
    head: -0.55,
    tail: 1.0,
    ..GALLOP
};

const GAITS: [Gait; 4] = [WALK, TROT, GALLOP, RUN];

/// A pegasus's wing: from the withers, in front of the rider's knees, about
/// half again the horse's length from tip to tip, deep and full, beating
/// slow and deep. Flight feathers lie under three rows of coverts.
pub const WING: Wing = Wing {
    shoulder: DVec3::new(0.2, 1.46, -0.62),
    bones_m: [0.44, 0.55, 0.37],
    edge_m: [[0.28, 0.1], [0.17, 0.07], [0.1, 0.045]],
    rows: &[
        // Secondaries, from the arm to the wrist, straight back.
        Row {
            along: [0.35, 2.0],
            count: 9,
            long_m: [0.7, 0.78],
            wide_m: 0.26,
            thick_m: 0.03,
            fan: [-0.05, 0.08],
            fingers: 0.0,
            over_m: 0.0,
            paint: 1,
        },
        // Primaries, fanned from the hand toward the tip.
        Row {
            along: [2.0, 3.0],
            count: 8,
            long_m: [0.72, 0.84],
            wide_m: 0.2,
            thick_m: 0.03,
            fan: [0.12, 0.8],
            fingers: 0.05,
            over_m: 0.0,
            paint: 2,
        },
        // Greater coverts over the secondaries' roots.
        Row {
            along: [0.2, 2.0],
            count: 9,
            long_m: [0.42, 0.4],
            wide_m: 0.26,
            thick_m: 0.035,
            fan: [-0.04, 0.06],
            fingers: 0.0,
            over_m: 0.035,
            paint: 0,
        },
        // Primary coverts over the primaries' roots.
        Row {
            along: [2.0, 2.85],
            count: 6,
            long_m: [0.36, 0.28],
            wide_m: 0.2,
            thick_m: 0.035,
            fan: [0.1, 0.65],
            fingers: 0.0,
            over_m: 0.035,
            paint: 0,
        },
        // Lesser coverts along the leading edge.
        Row {
            along: [0.0, 2.3],
            count: 8,
            long_m: [0.24, 0.16],
            wide_m: 0.27,
            thick_m: 0.04,
            fan: [-0.02, 0.15],
            fingers: 0.0,
            over_m: 0.065,
            paint: 0,
        },
    ],
    beat_hz: (0.9, 1.8),
    stroke: (0.1, 1.0),
    // Slow enough to be watched, a sweep and never a flick; the same to
    // fold away.
    open_s: 0.5,
    fold_s: 0.5,
    heave_m: 0.08,
};

/// A riding horse, about 1.5 m at the withers.
pub const HORSE: Kind = Kind {
    name: "horse",
    number: 1,
    rig: Rig {
        fore: Leg {
            root: DVec3::new(-0.16, 0.985, -0.6),
            foot: DVec3::new(-0.15, 0.0, -0.62),
            upper_m: 0.46,
            lower_m: 0.37,
            upper: [0.15, 0.18],
            lower: [0.11, 0.12],
            sweep: 0.4,
        },
        hind: Leg {
            root: DVec3::new(-0.17, 1.02, 0.5),
            foot: DVec3::new(-0.16, 0.0, 0.72),
            upper_m: 0.52,
            lower_m: 0.38,
            upper: [0.17, 0.24],
            lower: [0.11, 0.13],
            sweep: 0.3,
        },
        pastern_m: 0.08,
        hoof_m: 0.09,
        hoof: [0.14, 0.16],
        fold: 0.9,
        fold_per_lift: 3.0,
        // Its back is a roof of steps: narrow along the spine, wider down
        // each side, widest at the barrel under a rider's knees. Front to
        // back: the chest, the withers rising before the seat, the seat
        // sunk between them, the loins, and the croup.
        trunk: &[
            // The chest.
            Section {
                from_z: -0.85,
                to_z: -0.40,
                bottom: 0.92,
                top: 1.30,
                width: 0.46,
                cuts: [2, 2],
            },
            Section {
                from_z: -0.85,
                to_z: -0.40,
                bottom: 1.30,
                top: 1.50,
                width: 0.4,
                cuts: [2, 1],
            },
            // The withers.
            Section {
                from_z: -0.62,
                to_z: -0.36,
                bottom: 1.50,
                top: 1.6,
                width: 0.18,
                cuts: [1, 1],
            },
            // The seat, where a rider sits, on its top step.
            Section {
                from_z: -0.40,
                to_z: 0.20,
                bottom: 1.46,
                top: 1.50,
                width: 0.14,
                cuts: [1, 1],
            },
            Section {
                from_z: -0.40,
                to_z: 0.20,
                bottom: 1.38,
                top: 1.46,
                width: 0.30,
                cuts: [2, 1],
            },
            Section {
                from_z: -0.40,
                to_z: 0.20,
                bottom: 1.26,
                top: 1.38,
                width: 0.46,
                cuts: [3, 1],
            },
            Section {
                from_z: -0.40,
                to_z: 0.20,
                bottom: 0.92,
                top: 1.26,
                width: 0.58,
                cuts: [4, 2],
            },
            // The loins.
            Section {
                from_z: 0.20,
                to_z: 0.44,
                bottom: 0.98,
                top: 1.30,
                width: 0.54,
                cuts: [1, 2],
            },
            Section {
                from_z: 0.20,
                to_z: 0.44,
                bottom: 1.30,
                top: 1.44,
                width: 0.4,
                cuts: [1, 1],
            },
            Section {
                from_z: 0.20,
                to_z: 0.44,
                bottom: 1.44,
                top: 1.52,
                width: 0.20,
                cuts: [1, 1],
            },
            // The croup.
            Section {
                from_z: 0.42,
                to_z: 0.84,
                bottom: 0.98,
                top: 1.34,
                width: 0.52,
                cuts: [2, 2],
            },
            Section {
                from_z: 0.42,
                to_z: 0.84,
                bottom: 1.34,
                top: 1.50,
                width: 0.42,
                cuts: [2, 1],
            },
            Section {
                from_z: 0.42,
                to_z: 0.84,
                bottom: 1.50,
                top: 1.58,
                width: 0.24,
                cuts: [1, 1],
            },
        ],
        pivot: DVec3::new(0.0, 1.1, 0.0),
        roll_follow: 0.35,
        neck: Neck {
            root: DVec3::new(0.0, 1.38, -0.74),
            length_m: 0.78,
            into_m: 0.2,
            width_m: 0.24,
            depth_m: 0.4,
            cuts: 3,
            graze_neck: -0.85,
            graze_head: -1.45,
        },
        head: &[
            Part {
                centre: DVec3::new(0.0, 0.0, -0.15),
                size: DVec3::new(0.25, 0.28, 0.36),
                role: Role::Hair,
            },
            Part {
                centre: DVec3::new(0.0, -0.02, -0.45),
                size: DVec3::new(0.19, 0.22, 0.3),
                role: Role::Muzzle,
            },
        ],
        ears: Ears {
            at: DVec3::new(0.07, 0.16, 0.03),
            size: DVec3::new(0.05, 0.14, 0.06),
            back: 0.7,
        },
        eye: Part {
            centre: DVec3::new(0.127, 0.06, -0.12),
            size: DVec3::new(0.012, 0.045, 0.06),
            role: Role::Eye,
        },
        tail: Tail {
            root: DVec3::new(0.0, 1.46, 0.84),
            dock_m: 0.3,
            dock_width_m: 0.11,
            hair: [0.16, 0.07],
            hang: 0.75,
            hair_hang: 1.35,
        },
    },
    gaits: Gaits(&GAITS),
    coats: &COATS,
    builds: &[
        // A pony: small and stocky.
        Build {
            weight: 20,
            size: (0.66, 0.8),
            build: (1.08, 1.25),
            mane_m: (0.04, 0.16),
            tail_m: (0.42, 0.8),
            feathers: false,
            coats: &[],
        },
        Build {
            weight: 62,
            size: (0.92, 1.05),
            build: (0.92, 1.06),
            mane_m: (0.04, 0.11),
            tail_m: (0.42, 0.95),
            feathers: false,
            coats: &[],
        },
        // A draught: big and broad, its hooves feathered.
        Build {
            weight: 18,
            size: (1.06, 1.16),
            build: (1.25, 1.4),
            mane_m: (0.04, 0.16),
            tail_m: (0.42, 0.95),
            feathers: true,
            coats: &["bay", "brown", "grey", "chestnut"],
        },
    ],
    marks: Marks {
        faces: &FACES,
        socks: &[(5, 0), (1, 1), (1, 2), (1, 3)],
        patches: Patches {
            blots: (2, 5),
            radius_m: (0.18, 0.42),
            y: (0.95, 1.6),
            z: (-1.3, 0.85),
        },
        white: WHITE_MARK,
        pale_hoof: HOOF_PALE,
        skin: GREY_SKIN,
        eye: EYE,
    },
    herding: Herding {
        lives_on: &[Habitat::Grass, Habitat::Forest],
        steep: 0.6,
        region_m: 160.0,
        chance: 0.3,
        size: (4, 9),
        spread_m: 6.0,
        roam_m: 45.0,
        bout_s: (100.0, 170.0),
        wander_mps: 1.3,
        turn_per_s: 1.0,
        restless: (0.0, 1.0),
        trot_mps: 3.0,
    },
    carry: Some(Carry {
        saddle: DVec3::new(0.0, 1.5, -0.12),
        // A walk climbs what a walker climbs; a trot holds on a hillside,
        // a gallop only on a gentle one and a run on next to flat. At a
        // tap, from a halt or a walk it crouches longer and springs a short
        // hop forward; from a trot it pops over a log, and at a gallop or a
        // run it flies a fence. The run is sprint's alone.
        paces: &[
            Pace {
                speed_mps: 1.5,
                turn_per_s: 2.2,
                steep: None,
                jump: Some(Least {
                    gather_s: 0.35,
                    height_m: 0.6,
                }),
            },
            Pace {
                speed_mps: 3.6,
                turn_per_s: 1.5,
                steep: Some(0.6),
                jump: Some(Least {
                    gather_s: 0.25,
                    height_m: 0.7,
                }),
            },
            Pace {
                speed_mps: 9.0,
                turn_per_s: 0.8,
                steep: Some(0.35),
                jump: Some(Least {
                    gather_s: 0.22,
                    height_m: 0.8,
                }),
            },
            Pace {
                speed_mps: 11.7,
                turn_per_s: 0.6,
                steep: Some(0.25),
                jump: Some(Least {
                    gather_s: 0.22,
                    height_m: 0.8,
                }),
            },
        ],
        stepped: 2,
        sprint: 3,
        // A pony at four fifths, a draught at six fifths: a riding horse of
        // size 1 gallops at 9.6 m/s and runs at 12.5.
        by_size: BySize {
            sizes: (0.66, 1.16),
            shares: (0.8, 1.2),
        },
        accel_mps2: 4.0,
        decel_mps2: 6.0,
        wade_m: 0.6,
        height_m: 2.6,
        mount_s: 0.25,
        wait_s: 45.0,
        wait_m: 30.0,
        standing: Some(Least {
            gather_s: 0.45,
            height_m: 0.5,
        }),
        // Held, any jump rises to 3 m, held a second; from a halt or a
        // walk it springs on at about a trot, so a tap there flies a hop
        // of two metres and more.
        jump: Jump {
            most_m: 3.0,
            hold_s: 1.0,
            forward_mps: 4.0,
        },
        // It backs a step at a time, then faster while held, never as
        // fast as it walks.
        backing: Some(Backing {
            from_mps: 0.6,
            top_mps: 1.4,
            gain_mps2: 0.8,
            turn_per_s: 1.2,
        }),
    }),
    // Every herd has a pegasus: a white one, a black one, or one of each.
    // It flies half again as fast as a body does near the ground, gaining
    // less with height, to a few hundred metres and at most 150 m/s.
    wings: Some(Winged {
        herds: &[&["white"], &["black"], &["white", "black"]],
        // From a brown to a palomino: a mid grey is plain, a pale grey or a
        // near-black would read as a pegasus without its wings.
        plain: (0.06, 0.5),
        wing: &WING,
        flight: Flight {
            speed_mps: 18.0,
            per_m: 0.25,
            sprint: 2.5,
            top_mps: 150.0,
            // Up to the clouds' top, and over them; never yet past it.
            ceiling_m: scene::CLOUD_TOP_M as f64,
            takeoff_m: 3.0,
            // A gallop's gather and a jump's arc: the press is answered at
            // once; the wings take their own time to open after it.
            spring: Spring {
                gather_s: 0.2,
                rise_s: 0.5,
                height_m: 1.8,
                forward_mps: 3.0,
            },
            land_mps: 2.5,
            land_per_m: 1.0,
        },
    }),
    // A whinny a little higher or lower than another's, past its size.
    voice_cents: (-150.0, 150.0),
};
