//! The core's voices, as data: what a body sounds on purpose, a hoof
//! coming down on each ground, a jump's spring and landing, a wing beating
//! down, and a pegasus set down from the wing. Tuned by ear on the board
//! (`/voices`).

use synth::Filter::{Band, High, Low, Open};
use synth::Source::{Noise, Wave};
use synth::Wave::{Saw, Sine, Triangle};
use worldgen::Material;

use crate::animals::BUILT;
use synth::{Envelope, Layer, Pitch, Vary, Voice};

/// What a body sounds on purpose, by the name [`crate::Command::Sound`]
/// gives, and how far each carries: every voice of the core.
pub static CALLS: &[(&Voice, f64)] = &[(&WHISTLE, gaia::fauna::HEARS_M)];

/// Two notes, up then down, as one calls a horse: a breath under both.
pub static WHISTLE: Voice = Voice::new(
    "whistle",
    Vary {
        cents: 30.0,
        gain: 0.1,
        time: 0.05,
    },
    &[
        Layer::new(
            0.0,
            Wave(Sine, Pitch::steady(1400.0).glide(2500.0, 0.22)),
            Envelope::new(0.03, 0.17, 0.06),
            Open,
            0.55,
        ),
        Layer::new(
            0.32,
            Wave(
                Sine,
                Pitch::steady(2500.0).glide(1300.0, 0.4).vibrato(6.0, 15.0),
            ),
            Envelope::new(0.02, 0.3, 0.12),
            Open,
            0.55,
        ),
        Layer::new(
            0.0,
            Noise,
            Envelope::new(0.02, 0.7, 0.1),
            Band { hz: 2000.0, q: 1.5 },
            0.04,
        ),
    ],
);

/// A hoof coming down on each ground, and the grounds it sounds for, by
/// the number a footfall carries: a material's, or [`BUILT`].
pub static HOOVES: &[(&Voice, &[u8])] = &[
    (
        &HOOF_ON_GRASS,
        &[
            Material::Grass as u8,
            Material::Forest as u8,
            Material::Plot as u8,
        ],
    ),
    (
        &HOOF_ON_SAND,
        &[
            Material::Sand as u8,
            Material::Seabed as u8,
            Material::Regolith as u8,
        ],
    ),
    (&HOOF_ON_ROCK, &[Material::Rock as u8, BUILT]),
    (&HOOF_ON_SNOW, &[Material::Snow as u8]),
];

/// How far a hoof is heard, metres: under a rider, and in a free herd,
/// quieter, since a strike falls off to silence at its reach.
pub const HOOF_REACH_M: (f64, f64) = (40.0, 20.0);

/// The voice of a hoof coming down on a ground, by the number a footfall
/// carries, as hard as its gait's force: a walk soft, a gallop full. None
/// where a hoof makes no sound of its own, in water.
pub fn hoof(surface: u8, force: f64) -> Option<Voice> {
    HOOVES
        .iter()
        .find(|(_, grounds)| grounds.contains(&surface))
        .map(|&(voice, _)| voice.scaled(force as f32))
}

/// A great wing beating down: air pushed in a soft low rush, a thump
/// under it, and the feathers' rustle at its end.
pub static WINGBEAT: Voice = Voice::new(
    "wingbeat",
    Vary {
        cents: 80.0,
        gain: 0.15,
        time: 0.1,
    },
    &[
        Layer::new(
            0.0,
            Noise,
            Envelope::new(0.07, 0.04, 0.22),
            Low { hz: 380.0, q: 0.8 },
            0.6,
        ),
        Layer::new(
            0.03,
            Wave(Sine, Pitch::steady(75.0).glide(48.0, 0.12)),
            Envelope::new(0.02, 0.0, 0.14),
            Open,
            0.3,
        ),
        Layer::new(
            0.1,
            Noise,
            Envelope::new(0.03, 0.02, 0.12),
            Band { hz: 2400.0, q: 0.9 },
            0.08,
        ),
    ],
);

/// How far a wingbeat is heard, metres.
pub const WINGBEAT_REACH_M: f64 = 60.0;

/// A horse's whinny: a high cry falling and shaking, a breath under it,
/// and a low nicker at its end. Pitched for each horse by [`neigh`]. On
/// the board only: nothing in the world strikes it yet.
pub static NEIGH: Voice = Voice::new(
    "neigh",
    Vary {
        cents: 40.0,
        gain: 0.1,
        time: 0.08,
    },
    &[
        Layer::new(
            0.0,
            Wave(
                Saw,
                Pitch::steady(680.0).glide(420.0, 0.85).vibrato(12.0, 80.0),
            ),
            Envelope::new(0.05, 0.45, 0.35),
            Band { hz: 1200.0, q: 1.0 },
            0.32,
        ),
        Layer::new(
            0.01,
            Wave(
                Saw,
                Pitch::steady(1360.0).glide(840.0, 0.85).vibrato(12.0, 80.0),
            ),
            Envelope::new(0.06, 0.4, 0.3),
            Band { hz: 2600.0, q: 1.4 },
            0.12,
        ),
        Layer::new(
            0.0,
            Noise,
            Envelope::new(0.03, 0.5, 0.35),
            Band { hz: 1800.0, q: 0.7 },
            0.06,
        ),
        Layer::new(
            0.75,
            Wave(
                Saw,
                Pitch::steady(300.0).glide(200.0, 0.3).vibrato(9.0, 60.0),
            ),
            Envelope::new(0.03, 0.15, 0.2),
            Low { hz: 900.0, q: 0.7 },
            0.18,
        ),
    ],
);

/// The neigh of a horse born with `traits`: higher for a small one and
/// lower for a large one, by the square root of its size, and as far from
/// its kind's as its own voice sits.
pub fn neigh(traits: &gaia::fauna::Traits) -> Voice {
    let cents = traits.voice_cents.unwrap_or(0.0);
    let ratio = libm::exp2(cents / 1200.0) / traits.size.max(0.1).sqrt();
    NEIGH.pitched(ratio as f32)
}

/// How far a jump varies: no two alike, the hooves' spacing least.
const LEAP: Vary = Vary {
    cents: 90.0,
    gain: 0.1,
    time: 0.06,
};

/// A horse driving off the ground into a jump: its weight thrown up from
/// the haunches in a low push, both hind hooves biting the ground a beat
/// apart, and the earth they kick up.
pub static SPRING: Voice = Voice::new(
    "spring",
    LEAP,
    &[
        Layer::new(
            0.0,
            Wave(Sine, Pitch::steady(70.0).glide(115.0, 0.12)),
            Envelope::new(0.015, 0.05, 0.14),
            Open,
            0.45,
        ),
        Layer::new(
            0.0,
            Wave(Sine, Pitch::steady(150.0).glide(70.0, 0.05)),
            Envelope::new(0.001, 0.0, 0.08),
            Open,
            0.3,
        ),
        Layer::new(
            0.0,
            Noise,
            Envelope::new(0.001, 0.01, 0.07),
            Low { hz: 900.0, q: 0.7 },
            0.3,
        ),
        Layer::new(
            0.045,
            Wave(Sine, Pitch::steady(140.0).glide(65.0, 0.05)),
            Envelope::new(0.001, 0.0, 0.08),
            Open,
            0.25,
        ),
        Layer::new(
            0.045,
            Noise,
            Envelope::new(0.001, 0.01, 0.07),
            Low { hz: 900.0, q: 0.7 },
            0.25,
        ),
        Layer::new(
            0.03,
            Noise,
            Envelope::new(0.01, 0.04, 0.22),
            Band { hz: 2200.0, q: 0.8 },
            0.1,
        )
        .swept(1200.0, 0.25),
    ],
);

/// A horse coming down from a jump: a deep thud of its whole weight, the
/// fore hooves striking first, a beat apart, the hind ones after, and the
/// earth thrown forward under them.
pub static LANDING: Voice = Voice::new(
    "landing",
    LEAP,
    &[
        Layer::new(
            0.0,
            Wave(Sine, Pitch::steady(85.0).glide(38.0, 0.18)),
            Envelope::new(0.002, 0.04, 0.3),
            Open,
            0.5,
        ),
        Layer::new(
            0.0,
            Wave(Sine, Pitch::steady(160.0).glide(70.0, 0.05)),
            Envelope::new(0.001, 0.0, 0.09),
            Open,
            0.25,
        ),
        Layer::new(
            0.0,
            Noise,
            Envelope::new(0.001, 0.015, 0.09),
            Low { hz: 1000.0, q: 0.7 },
            0.3,
        ),
        Layer::new(
            0.035,
            Noise,
            Envelope::new(0.001, 0.01, 0.08),
            Low { hz: 800.0, q: 0.7 },
            0.22,
        ),
        Layer::new(
            0.16,
            Wave(Sine, Pitch::steady(130.0).glide(60.0, 0.05)),
            Envelope::new(0.001, 0.0, 0.08),
            Open,
            0.22,
        ),
        Layer::new(
            0.16,
            Noise,
            Envelope::new(0.001, 0.01, 0.07),
            Low { hz: 800.0, q: 0.7 },
            0.18,
        ),
        Layer::new(
            0.2,
            Noise,
            Envelope::new(0.001, 0.01, 0.07),
            Low { hz: 800.0, q: 0.7 },
            0.15,
        ),
        Layer::new(
            0.02,
            Noise,
            Envelope::new(0.01, 0.06, 0.3),
            Band { hz: 1800.0, q: 0.7 },
            0.1,
        )
        .swept(700.0, 0.35),
    ],
);

/// How far a jump is heard, metres, its spring and its landing: past a
/// hoof ridden, short of a call. A pegasus set down from the wing carries
/// as far.
pub const LEAP_REACH_M: f64 = 60.0;

/// A pegasus set down from the wing: the last of the air under its wings
/// as they fold, its fore hooves taking its weight a beat apart, the hind
/// ones after, and its whole mass settling, deeper and longer than a
/// jump's thud. As hard as it comes down: [`touchdown`].
pub static TOUCHDOWN: Voice = Voice::new(
    "touchdown",
    LEAP,
    &[
        Layer::new(
            0.0,
            Noise,
            Envelope::new(0.004, 0.03, 0.3),
            Low { hz: 520.0, q: 0.6 },
            0.2,
        )
        .swept(220.0, 0.3),
        Layer::new(
            0.0,
            Wave(Sine, Pitch::steady(72.0).glide(30.0, 0.24)),
            Envelope::new(0.003, 0.06, 0.4),
            Open,
            0.5,
        ),
        Layer::new(
            0.0,
            Wave(Sine, Pitch::steady(160.0).glide(70.0, 0.05)),
            Envelope::new(0.001, 0.0, 0.09),
            Open,
            0.22,
        ),
        Layer::new(
            0.0,
            Noise,
            Envelope::new(0.001, 0.015, 0.09),
            Low { hz: 1000.0, q: 0.7 },
            0.28,
        ),
        Layer::new(
            0.04,
            Noise,
            Envelope::new(0.001, 0.01, 0.08),
            Low { hz: 800.0, q: 0.7 },
            0.22,
        ),
        Layer::new(
            0.14,
            Wave(Sine, Pitch::steady(130.0).glide(58.0, 0.05)),
            Envelope::new(0.001, 0.0, 0.08),
            Open,
            0.22,
        ),
        Layer::new(
            0.14,
            Noise,
            Envelope::new(0.001, 0.01, 0.07),
            Low { hz: 800.0, q: 0.7 },
            0.2,
        ),
        Layer::new(
            0.18,
            Noise,
            Envelope::new(0.001, 0.01, 0.07),
            Low { hz: 800.0, q: 0.7 },
            0.17,
        ),
    ],
);

/// The share of a pegasus's speed along the ground that comes down on its
/// hooves as it is set down, its fore legs reaching forward for the ground
/// about this steeply. The core eases every flight down to one slow sink,
/// so this is what tells a set-down at a gallop from one at a halt.
const TOUCHDOWN_CARRIED: f64 = 0.3;

/// How hard a set-down is heard from, metres a second, softer unheard, and
/// how hard is heard full: one at a halt is a touch, one at a gallop or onto
/// rising ground a thud.
const TOUCHDOWN_MPS: (f64, f64) = (1.0, 6.0);

/// How many loudnesses a set-down is heard at: a few, so a shell keeps one
/// buffer of each and its seeds turn, as a hoof's at its gait's force.
const TOUCHDOWN_STEPS: f64 = 8.0;

/// A pegasus born with `traits` set down from the wing, `landing`: as hard
/// as it sank onto the ground and a share of how fast it went along it,
/// pitched over the square root of its size as a jump. None for a touch
/// too soft to thump.
pub fn touchdown(traits: &gaia::fauna::Traits, landing: &gaia::fauna::Landing) -> Option<Voice> {
    let hard_mps = landing.sink_mps + TOUCHDOWN_CARRIED * landing.speed_mps;
    let (soft_mps, full_mps) = TOUCHDOWN_MPS;
    let hard = ((hard_mps - soft_mps) / (full_mps - soft_mps)).clamp(0.0, 1.0);
    let level = libm::ceil(hard * TOUCHDOWN_STEPS) / TOUCHDOWN_STEPS;
    (level > 0.0).then(|| leap(&TOUCHDOWN, traits, level))
}

/// A rider come down on its feet from a leap off what it rode: both soles
/// a beat apart under its weight, and a scuff.
pub static ALIGHT: Voice = Voice::new(
    "alight",
    LEAP,
    &[
        Layer::new(
            0.0,
            Wave(Sine, Pitch::steady(110.0).glide(55.0, 0.08)),
            Envelope::new(0.002, 0.0, 0.1),
            Open,
            0.45,
        ),
        Layer::new(
            0.0,
            Noise,
            Envelope::new(0.002, 0.0, 0.06),
            Low { hz: 900.0, q: 0.7 },
            0.35,
        ),
        Layer::new(
            0.05,
            Wave(Sine, Pitch::steady(120.0).glide(60.0, 0.06)),
            Envelope::new(0.001, 0.0, 0.08),
            Open,
            0.3,
        ),
        Layer::new(
            0.05,
            Noise,
            Envelope::new(0.002, 0.0, 0.05),
            Low { hz: 900.0, q: 0.7 },
            0.25,
        ),
        Layer::new(
            0.02,
            Noise,
            Envelope::new(0.01, 0.02, 0.1),
            Band { hz: 2500.0, q: 0.8 },
            0.08,
        ),
    ],
);

/// How far a rider landing on its feet is heard, metres: a body's steps.
pub const ALIGHT_REACH_M: f64 = 20.0;

/// A jump's spring or landing, `voice`, as a horse born with `traits`
/// makes it `force` hard: lower and heavier the larger it is, by the
/// square root of its size, as a neigh, and louder the harder it goes.
pub fn leap(voice: &Voice, traits: &gaia::fauna::Traits, force: f64) -> Voice {
    let ratio = 1.0 / traits.size.max(0.1).sqrt();
    voice.pitched(ratio as f32).scaled(force as f32)
}

/// Air past wings held out to glide: a soft low rush and a breath over it,
/// sustained, looped from its hold.
pub static WIND: Voice = Voice::new(
    "wind",
    Vary::NONE,
    &[
        Layer::new(
            0.0,
            Noise,
            Envelope::new(0.01, 3.0, 0.2),
            Low { hz: 520.0, q: 0.6 },
            0.5,
        ),
        Layer::new(
            0.0,
            Noise,
            Envelope::new(0.01, 3.0, 0.2),
            Band { hz: 1400.0, q: 0.7 },
            0.12,
        ),
    ],
);

/// How far the wind of a glide is heard, metres: as the hoof ridden.
pub const WIND_REACH_M: f64 = 40.0;
/// What the wind a rider's wings sound is kept by among the voices
/// sustained, its name's bytes: one a rider, mixed in.
pub const WIND_KEY: u64 = u64::from_be_bytes(*b"wind\0\0\0\0");

/// Between these speeds a glide's wind goes from its softest and darkest
/// to its loudest and brightest, metres a second.
const WIND_MPS: (f64, f64) = (18.0, 150.0);

/// How loud the wind is at the slowest glide and at the fastest, wings
/// held all the way out: well under a wingbeat.
const WIND_LEVEL: (f64, f64) = (0.13, 0.38);

/// How loud and how bright the wind of a glide sounds, 0 to 1 each:
/// louder and brighter with speed, as far as the wings are held out.
pub fn wind(glide: &gaia::fauna::Glide) -> (f32, f32) {
    let fast = ((glide.speed_mps - WIND_MPS.0) / (WIND_MPS.1 - WIND_MPS.0)).clamp(0.0, 1.0);
    let level = glide.held_out * (WIND_LEVEL.0 + (WIND_LEVEL.1 - WIND_LEVEL.0) * fast);
    (level as f32, (0.25 + 0.75 * fast) as f32)
}

/// How far a hoof varies: no two falls alike.
const HOOF: Vary = Vary {
    cents: 120.0,
    gain: 0.15,
    time: 0.1,
};

/// A horse's hoof in turf: a deep thud, the grass torn after.
pub static HOOF_ON_GRASS: Voice = Voice::new(
    "hoof on grass",
    HOOF,
    &[
        Layer::new(
            0.0,
            Wave(Sine, Pitch::steady(120.0).glide(60.0, 0.06)),
            Envelope::new(0.001, 0.0, 0.09),
            Open,
            0.5,
        ),
        Layer::new(
            0.0,
            Noise,
            Envelope::new(0.002, 0.0, 0.06),
            Low { hz: 700.0, q: 0.7 },
            0.35,
        ),
        Layer::new(
            0.04,
            Noise,
            Envelope::new(0.01, 0.01, 0.08),
            Band { hz: 3000.0, q: 0.8 },
            0.15,
        ),
    ],
);

/// Sinking in sand: a dull fall and a long hiss of grains.
pub static HOOF_ON_SAND: Voice = Voice::new(
    "hoof on sand",
    HOOF,
    &[
        Layer::new(
            0.0,
            Wave(Sine, Pitch::steady(100.0).glide(55.0, 0.05)),
            Envelope::new(0.001, 0.0, 0.07),
            Open,
            0.4,
        ),
        Layer::new(
            0.0,
            Noise,
            Envelope::new(0.01, 0.03, 0.12),
            Band { hz: 1600.0, q: 0.9 },
            0.4,
        ),
        Layer::new(
            0.03,
            Noise,
            Envelope::new(0.01, 0.0, 0.08),
            High { hz: 5000.0, q: 0.7 },
            0.1,
        ),
    ],
);

/// Iron on stone: a sharp clack, the hoof's body under it, the toe after.
pub static HOOF_ON_ROCK: Voice = Voice::new(
    "hoof on rock",
    HOOF,
    &[
        Layer::new(
            0.0,
            Noise,
            Envelope::new(0.001, 0.0, 0.035),
            Band { hz: 2800.0, q: 3.0 },
            0.6,
        ),
        Layer::new(
            0.0,
            Wave(Triangle, Pitch::steady(220.0).glide(140.0, 0.05)),
            Envelope::new(0.001, 0.0, 0.07),
            Open,
            0.35,
        ),
        Layer::new(
            0.045,
            Noise,
            Envelope::new(0.001, 0.0, 0.02),
            Band { hz: 3400.0, q: 4.0 },
            0.25,
        ),
    ],
);

/// Packed snow giving under a hoof.
pub static HOOF_ON_SNOW: Voice = Voice::new(
    "hoof on snow",
    HOOF,
    &[
        Layer::new(
            0.0,
            Wave(Sine, Pitch::steady(90.0).glide(50.0, 0.06)),
            Envelope::new(0.002, 0.0, 0.08),
            Open,
            0.35,
        ),
        Layer::new(
            0.0,
            Noise,
            Envelope::new(0.02, 0.05, 0.1),
            Band { hz: 1000.0, q: 1.2 },
            0.45,
        ),
        Layer::new(
            0.04,
            Noise,
            Envelope::new(0.01, 0.01, 0.06),
            High { hz: 4000.0, q: 0.7 },
            0.12,
        ),
    ],
);

#[cfg(test)]
mod tests {
    use super::*;
    use glam::DVec3;

    fn glide(held_out: f64, speed_mps: f64) -> gaia::fauna::Glide {
        gaia::fauna::Glide {
            at: DVec3::ZERO,
            held_out,
            speed_mps,
        }
    }

    /// A small horse neighs higher than a large one, and its own voice
    /// moves it past its size; one kept without a voice is its kind's.
    #[test]
    fn a_neigh_is_pitched_by_size_and_voice() {
        let born = gaia::fauna::Traits::drawn(gaia::fauna::KINDS[0], 7, None);
        let hz = |size: f64, voice_cents: Option<f64>| {
            let traits = gaia::fauna::Traits {
                size,
                voice_cents,
                ..born.clone()
            };
            match neigh(&traits).layers[0].source {
                Wave(_, pitch) => pitch.hz,
                Noise => unreachable!("the cry is an oscillator"),
            }
        };
        assert!(hz(0.7, Some(0.0)) > hz(1.0, Some(0.0)));
        assert!(hz(1.0, Some(0.0)) > hz(1.15, Some(0.0)));
        assert!((hz(1.0, Some(0.0)) - 680.0).abs() < 1e-3);
        assert!(
            (hz(1.0, Some(1200.0)) - 1360.0).abs() < 1e-2,
            "an octave up"
        );
        assert_eq!(hz(1.0, None), hz(1.0, Some(0.0)));
    }

    /// A jump sounds lower for a large horse and higher for a small one,
    /// by the square root of its size, and louder the harder it goes; its
    /// layers that sound together never clip.
    #[test]
    fn a_jump_is_pitched_by_size_and_as_loud_as_its_force() {
        let born = gaia::fauna::Traits::drawn(gaia::fauna::KINDS[0], 7, None);
        let sized = |size: f64| gaia::fauna::Traits {
            size,
            ..born.clone()
        };
        let hz = |voice: &Voice| match voice.layers[0].source {
            Wave(_, pitch) => pitch.hz,
            Noise => unreachable!("the thud is an oscillator"),
        };
        for voice in [&SPRING, &LANDING] {
            let (pony, draught) = (
                leap(voice, &sized(0.7), 1.0),
                leap(voice, &sized(1.15), 1.0),
            );
            assert!(hz(&pony) > hz(voice) && hz(voice) > hz(&draught));
            let soft = leap(voice, &sized(1.0), 0.6);
            assert!((soft.layers[0].gain - 0.6 * voice.layers[0].gain).abs() < 1e-6);
            let mut samples = vec![0.0; voice.length(0, 48_000)];
            voice.render(0, 48_000, &mut samples);
            let peak = samples.iter().fold(0.0f32, |most, s| most.max(s.abs()));
            assert!(peak > 0.3 && peak < 1.0, "{}: {peak}", voice.name);
        }
    }

    /// A rider alighting is heard under a horse's landing, and never clips.
    #[test]
    fn a_rider_alights_softer_than_a_horse_lands() {
        let peak = |voice: &Voice| {
            let mut samples = vec![0.0; voice.length(0, 48_000)];
            voice.render(0, 48_000, &mut samples);
            samples.iter().fold(0.0f32, |most, s| most.max(s.abs()))
        };
        let (alight, landing) = (peak(&ALIGHT), peak(&LANDING));
        assert!(alight > 0.2 && alight < landing, "{alight} {landing}");
    }

    /// A voice rendered for a seed: its loudest sample, and its energy, the
    /// sum of its samples squared.
    fn heard(voice: &Voice) -> (f32, f32) {
        let mut samples = vec![0.0; voice.length(0, 48_000)];
        voice.render(0, 48_000, &mut samples);
        let peak = samples.iter().fold(0.0f32, |most, s| most.max(s.abs()));
        (peak, samples.iter().map(|s| s * s).sum())
    }

    /// A pegasus set down at a halt, as the core sets every flight down, is
    /// not heard; a sink as hard, carried along at a gallop, is; and the
    /// harder it sinks, the louder, up to the full voice. Rendered, it is
    /// heavier than a jump's landing, more of it and deeper, and never
    /// clips.
    #[test]
    fn a_set_down_is_as_loud_as_it_comes_down_hard() {
        let traits = gaia::fauna::Traits::drawn(gaia::fauna::KINDS[0], 7, Some("white"));
        let touchdown = |sink_mps: f64, speed_mps: f64| {
            let landing = gaia::fauna::Landing {
                at: DVec3::ZERO,
                sink_mps,
                speed_mps,
                winged: true,
            };
            touchdown(&traits, &landing).map_or(0.0, |voice| heard(&voice).0)
        };
        assert_eq!(touchdown(0.66, 0.0), 0.0, "a touch at a halt");
        let sinks = [0.66, 1.4, 2.1];
        let at_gallop: Vec<f32> = sinks.iter().map(|&sink| touchdown(sink, 12.5)).collect();
        assert!(at_gallop[0] > 0.0, "set down at a gallop: {at_gallop:?}");
        assert!(
            at_gallop.windows(2).all(|pair| pair[1] > pair[0]),
            "louder as it sinks harder: {at_gallop:?}"
        );
        assert!(touchdown(3.0, 0.0) > 0.0, "a hard sink at a halt");
        let full = leap(&TOUCHDOWN, &traits, 1.0);
        assert_eq!(touchdown(20.0, 12.5), heard(&full).0, "held at full");

        let ((peak, energy), (_, landing_energy)) = (heard(&TOUCHDOWN), heard(&LANDING));
        assert!(peak < 1.0, "{peak}");
        assert!(energy > landing_energy, "{energy} {landing_energy}");
        let thud = |voice: &Voice| {
            let waves = voice.layers.iter().filter_map(|layer| match layer.source {
                Wave(_, pitch) => Some(pitch.hz),
                Noise => None,
            });
            waves.fold(f32::MAX, f32::min)
        };
        assert!(thud(&TOUCHDOWN) < thud(&LANDING), "deeper than a jump's");
    }

    /// The wind of a glide is as loud as the wings are held out, louder and
    /// brighter the faster it flies, and never as loud as a wingbeat.
    #[test]
    fn the_wind_rises_with_speed_and_stays_under_the_beat() {
        assert_eq!(wind(&glide(0.0, 150.0)).0, 0.0, "wings beating");
        let (slow, fast) = (wind(&glide(1.0, 18.0)), wind(&glide(1.0, 150.0)));
        assert!(
            slow.0 > 0.0 && fast.0 > slow.0 && fast.1 > slow.1,
            "{slow:?} {fast:?}"
        );
        assert_eq!(wind(&glide(1.0, 400.0)), fast, "held at the fastest");
        let peak = |voice: &Voice| voice.layers.iter().map(|layer| layer.gain).sum::<f32>();
        assert!(fast.0 * peak(&WIND) < 0.5 * peak(&WINGBEAT));
    }

    /// Every ground a hoof stands on has its voice, and only one; water has
    /// none.
    #[test]
    fn a_hoof_sounds_on_every_ground_but_water() {
        for id in 0..=u8::MAX {
            let grounds = HOOVES.iter().filter(|(_, grounds)| grounds.contains(&id));
            assert!(grounds.count() <= 1, "ground {id} has two voices");
        }
        assert!(hoof(Material::Water as u8, 1.0).is_none());
        for material in [
            Material::Sand,
            Material::Grass,
            Material::Forest,
            Material::Rock,
            Material::Snow,
            Material::Seabed,
            Material::Regolith,
            Material::Plot,
        ] {
            assert!(hoof(material as u8, 1.0).is_some(), "{material:?}");
        }
        assert_eq!(hoof(BUILT, 1.0).map(|v| v.name), Some("hoof on rock"));
    }
}
