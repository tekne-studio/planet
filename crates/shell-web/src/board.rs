//! The voice board, `/voices` in the web app: every voice of the core, to
//! hear and tune by ear, and trials, recipes for the plugins that will own
//! them. Nothing in the world strikes a trial.

use synth::Filter::{Band, Low, Open};
use synth::Source::{Noise, Wave};
use synth::Wave::{Saw, Sine, Triangle};
use synth::{Envelope, Layer, Pitch, Vary, Voice};

/// The trials, in the order the board shows them.
pub static TRIALS: &[&Voice] = &[&CRICKET, &CHIRPS, &EAGLE, &WHALE];

/// The core's voices, in the order the board shows them: what a body
/// sounds on purpose, then the hooves, a jump, a rider alighting, the
/// wings, a pegasus set down from them, and the neigh.
pub fn core() -> impl Iterator<Item = &'static Voice> {
    let calls = client::voices::CALLS.iter().map(|&(voice, _)| voice);
    calls
        .chain(client::voices::HOOVES.iter().map(|&(voice, _)| voice))
        .chain([
            &client::voices::SPRING,
            &client::voices::LANDING,
            &client::voices::ALIGHT,
            &client::voices::WINGBEAT,
            &client::voices::WIND,
            &client::voices::TOUCHDOWN,
            &client::voices::NEIGH,
        ])
}

/// The voice the board calls `name`: the core's first, then a trial.
pub fn called(name: &str) -> Option<&'static Voice> {
    core()
        .chain(TRIALS.iter().copied())
        .find(|voice| voice.name == name)
}

/// One pulse of a cricket's chirp, at `at_s`.
const fn pulse(at_s: f32) -> Layer {
    Layer::new(
        at_s,
        Wave(Sine, Pitch::steady(4600.0)),
        Envelope::new(0.002, 0.01, 0.008),
        Open,
        0.35,
    )
}

/// Two chirps of three pulses each, a wing drawn over the other.
pub static CRICKET: Voice = Voice::new(
    "cricket",
    Vary {
        cents: 40.0,
        gain: 0.1,
        time: 0.05,
    },
    &[
        pulse(0.0),
        pulse(0.03),
        pulse(0.06),
        pulse(0.3),
        pulse(0.33),
        pulse(0.36),
    ],
);

/// One falling chirp of a small bird, at `at_s`.
const fn chirp(at_s: f32) -> Layer {
    Layer::new(
        at_s,
        Wave(Sine, Pitch::steady(6000.0).glide(3500.0, 0.05)),
        Envelope::new(0.003, 0.02, 0.03),
        Open,
        0.4,
    )
}

/// A small bird: three quick chirps down, one trill up.
pub static CHIRPS: Voice = Voice::new(
    "small bird's chirps",
    Vary {
        cents: 80.0,
        gain: 0.15,
        time: 0.1,
    },
    &[
        chirp(0.0),
        chirp(0.09),
        chirp(0.18),
        Layer::new(
            0.3,
            Wave(
                Sine,
                Pitch::steady(3500.0)
                    .glide(6500.0, 0.08)
                    .vibrato(30.0, 50.0),
            ),
            Envelope::new(0.005, 0.05, 0.04),
            Open,
            0.35,
        ),
    ],
);

/// A raptor's long scream, falling, rough at the edges.
pub static EAGLE: Voice = Voice::new(
    "eagle's cry",
    Vary {
        cents: 60.0,
        gain: 0.1,
        time: 0.1,
    },
    &[
        Layer::new(
            0.0,
            Wave(
                Saw,
                Pitch::steady(3200.0).glide(2200.0, 0.9).vibrato(12.0, 40.0),
            ),
            Envelope::new(0.05, 0.6, 0.3),
            Band { hz: 2800.0, q: 2.0 },
            0.35,
        ),
        Layer::new(
            0.0,
            Wave(Sine, Pitch::steady(3200.0).glide(2200.0, 0.9)),
            Envelope::new(0.05, 0.6, 0.3),
            Open,
            0.2,
        ),
        Layer::new(
            0.0,
            Noise,
            Envelope::new(0.05, 0.6, 0.3),
            Band { hz: 3000.0, q: 3.0 },
            0.08,
        ),
    ],
);

/// A whale far off: a long rise, a longer fall, the sea under both.
pub static WHALE: Voice = Voice::new(
    "whale's song",
    Vary {
        cents: 100.0,
        gain: 0.1,
        time: 0.1,
    },
    &[
        Layer::new(
            0.0,
            Wave(
                Sine,
                Pitch::steady(180.0).glide(420.0, 1.5).vibrato(3.0, 20.0),
            ),
            Envelope::new(0.4, 1.0, 0.8),
            Open,
            0.4,
        ),
        Layer::new(
            1.6,
            Wave(
                Sine,
                Pitch::steady(420.0).glide(120.0, 1.8).vibrato(2.5, 25.0),
            ),
            Envelope::new(0.3, 1.2, 1.0),
            Open,
            0.4,
        ),
        Layer::new(
            0.0,
            Wave(Triangle, Pitch::steady(90.0).glide(85.0, 3.0)),
            Envelope::new(0.6, 2.0, 1.0),
            Low { hz: 300.0, q: 0.7 },
            0.15,
        ),
        Layer::new(
            0.0,
            Noise,
            Envelope::new(1.0, 1.5, 1.0),
            Low { hz: 400.0, q: 0.7 },
            0.03,
        ),
    ],
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_voice_on_the_board_answers_to_its_own_name() {
        for voice in core().chain(TRIALS.iter().copied()) {
            assert!(
                called(voice.name).is_some_and(|found| std::ptr::eq(found, voice)),
                "{} twice",
                voice.name
            );
        }
        assert!(called("snort").is_none());
    }
}
