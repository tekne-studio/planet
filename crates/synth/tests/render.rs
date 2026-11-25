//! A voice rendered: the same bits for a seed on every platform, within
//! -1..1, ending in silence, its energy where its pitch says. These run
//! natively and on wasm32 under node (`.cargo/config.toml`):
//! `cargo test -p synth --target wasm32-unknown-unknown`.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use synth::{Envelope, Filter, Layer, Pitch, Source, Vary, Voice, Wave};

const RATE: u32 = 48_000;

/// Every source, every filter, a glide and a vibrato: what the golden holds.
const EVERYTHING: Voice = Voice::new(
    "everything",
    Vary {
        cents: 50.0,
        gain: 0.2,
        time: 0.1,
    },
    &[
        Layer {
            at_s: 0.0,
            source: Source::Wave(Wave::Sine, Pitch::steady(440.0).vibrato(5.0, 30.0)),
            envelope: Envelope::new(0.01, 0.05, 0.1),
            filter: Filter::Open,
            gain: 0.2,
            sweep: None,
        },
        Layer {
            at_s: 0.02,
            source: Source::Wave(Wave::Triangle, Pitch::steady(220.0).glide(110.0, 0.1)),
            envelope: Envelope::new(0.0, 0.02, 0.08),
            filter: Filter::Low { hz: 800.0, q: 0.7 },
            gain: 0.2,
            sweep: None,
        },
        Layer {
            at_s: 0.04,
            source: Source::Wave(Wave::Saw, Pitch::steady(330.0).glide(990.0, 0.05)),
            envelope: Envelope::new(0.005, 0.0, 0.06),
            filter: Filter::High { hz: 200.0, q: 0.7 },
            gain: 0.2,
            sweep: None,
        },
        Layer {
            at_s: 0.06,
            source: Source::Wave(Wave::Square, Pitch::steady(660.0)),
            envelope: Envelope::new(0.002, 0.01, 0.03),
            filter: Filter::Open,
            gain: 0.15,
            sweep: None,
        },
        Layer {
            at_s: 0.0,
            source: Source::Noise,
            envelope: Envelope::new(0.001, 0.0, 0.05),
            filter: Filter::Band { hz: 3000.0, q: 3.0 },
            gain: 0.2,
            sweep: None,
        },
    ],
);

/// The samples' bits hashed, FNV-1a.
fn fingerprint(samples: &[f32]) -> u64 {
    samples.iter().fold(0xCBF2_9CE4_8422_2325, |hash, sample| {
        sample
            .to_bits()
            .to_le_bytes()
            .iter()
            .fold(hash, |hash, byte| {
                (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01B3)
            })
    })
}

fn rendered(voice: &Voice, seed: u32) -> Vec<f32> {
    let mut out = vec![0.0; voice.length(seed, RATE)];
    voice.render(seed, RATE, &mut out);
    out
}

/// How much of `hz` is in the samples: Goertzel's single bin.
fn power(samples: &[f32], hz: f32) -> f32 {
    let coefficient = 2.0 * (core::f32::consts::TAU * hz / RATE as f32).cos();
    let (mut s1, mut s2) = (0.0f32, 0.0f32);
    for &x in samples {
        let s = x + coefficient * s1 - s2;
        s2 = s1;
        s1 = s;
    }
    s1 * s1 + s2 * s2 - coefficient * s1 * s2
}

/// The loudest pitch between two, by steps of `step_hz`.
fn loudest(samples: &[f32], from_hz: f32, to_hz: f32, step_hz: f32) -> f32 {
    let steps = ((to_hz - from_hz) / step_hz) as usize;
    (0..=steps)
        .map(|i| from_hz + i as f32 * step_hz)
        .map(|hz| (hz, power(samples, hz)))
        .fold(
            (0.0, f32::MIN),
            |best, next| if next.1 > best.1 { next } else { best },
        )
        .0
}

fn tone(wave: Wave, pitch: Pitch, seconds: f32) -> Voice {
    Voice::made(
        "tone",
        Vary::NONE,
        vec![Layer {
            at_s: 0.0,
            source: Source::Wave(wave, pitch),
            envelope: Envelope::new(0.005, seconds, 0.005),
            filter: Filter::Open,
            gain: 0.5,
            sweep: None,
        }],
    )
}

#[test]
fn a_seed_gives_the_same_samples_on_every_platform() {
    let goldens: Vec<u64> = [0, 1, 7]
        .into_iter()
        .map(|seed| fingerprint(&rendered(&EVERYTHING, seed)))
        .collect();
    assert_eq!(
        goldens,
        [0xF125D156021FDDC2, 0xE88778FCE23A499F, 0x36294AC8A0B841D5,],
        "a change to how a voice renders changes these on purpose"
    );
}

/// A voice that holds: noise darkened, and a hum under it.
const HELD: Voice = Voice::new(
    "held",
    Vary::NONE,
    &[
        Layer {
            at_s: 0.0,
            source: Source::Noise,
            envelope: Envelope::new(0.01, 3.0, 0.2),
            filter: Filter::Low { hz: 600.0, q: 0.7 },
            gain: 0.4,
            sweep: None,
        },
        Layer {
            at_s: 0.0,
            source: Source::Wave(Wave::Sine, Pitch::steady(110.0)),
            envelope: Envelope::new(0.01, 3.0, 0.2),
            filter: Filter::Open,
            gain: 0.1,
            sweep: None,
        },
    ],
);

fn rms(samples: &[f32]) -> f32 {
    (samples.iter().map(|x| x * x).sum::<f32>() / samples.len() as f32).sqrt()
}

#[test]
fn a_loop_gives_the_same_samples_on_every_platform() {
    let looped = HELD.looped(0, RATE, 2.0);
    assert_eq!(looped.len(), 2 * RATE as usize);
    assert_eq!(
        fingerprint(&looped),
        0x18EAFA4F5198B23F,
        "a change to how a voice loops changes this on purpose"
    );
}

/// A tone that holds, its cycle no divisor of a loop: cut round with no
/// crossfade, its end and its start meet 0.6 of a cycle apart.
const HUM: Voice = Voice::new(
    "hum",
    Vary::NONE,
    &[Layer {
        at_s: 0.0,
        source: Source::Wave(Wave::Sine, Pitch::steady(110.3)),
        envelope: Envelope::new(0.01, 3.0, 0.2),
        filter: Filter::Open,
        gain: 0.5,
        sweep: None,
    }],
);

/// Played round, its end runs on into its start: no step there larger
/// than its steps elsewhere. A tone, where a lost crossfade is a click;
/// noise steps that far anywhere.
#[test]
fn a_loop_has_no_seam() {
    let looped = HUM.looped(3, RATE, 2.0);
    let step = |a: f32, b: f32| (b - a).abs();
    let largest = looped
        .windows(2)
        .map(|pair| step(pair[0], pair[1]))
        .fold(0.0, f32::max);
    let seam = step(looped[looped.len() - 1], looped[0]);
    assert!(seam <= largest, "{seam} at the seam, {largest} elsewhere");
}

/// A noise looped starts as loud as its middle: faded at equal power, as
/// two sounds that do not move together add.
#[test]
fn a_loop_of_noise_is_as_loud_where_it_fades() {
    let looped = HELD.looped(3, RATE, 2.0);
    let fade = (synth::LOOP_FADE_S * RATE as f32) as usize;
    let (start, middle) = (rms(&looped[..fade]), rms(&looped[fade..]));
    assert!(
        (0.8..1.25).contains(&(start / middle)),
        "{start} against {middle}"
    );
}

/// Pitched, every oscillator moves by the ratio, and a voice of another
/// pitch is another voice.
#[test]
fn a_voice_pitched_sounds_higher_by_its_ratio() {
    let voice = tone(Wave::Sine, Pitch::steady(440.0), 0.5);
    let higher = voice.pitched(1.5);
    assert_ne!(voice.key(), higher.key());
    let samples = rendered(&higher, 0);
    assert_eq!(loudest(&samples, 100.0, 2000.0, 10.0), 660.0);
}

/// A swell and a fade, and a filter's edge sliding down: what the second
/// golden holds.
const MOVING: Voice = Voice::new(
    "moving",
    Vary {
        cents: 50.0,
        gain: 0.2,
        time: 0.1,
    },
    &[
        Layer::new(
            0.0,
            Source::Wave(Wave::Square, Pitch::steady(160.0).glide(45.0, 0.25)),
            Envelope::new(0.002, 0.24, 0.008).sloping(1.0, 0.0),
            Filter::Open,
            0.25,
        ),
        Layer::new(
            0.0,
            Source::Noise,
            Envelope::new(0.002, 0.3, 0.008).sloping(0.2, 1.0),
            Filter::Low { hz: 6000.0, q: 0.7 },
            0.25,
        )
        .swept(800.0, 0.3),
    ],
);

#[test]
fn a_seed_gives_the_same_samples_on_every_platform_moving_too() {
    let goldens: Vec<u64> = [0, 1, 7]
        .into_iter()
        .map(|seed| fingerprint(&rendered(&MOVING, seed)))
        .collect();
    assert_eq!(
        goldens,
        [0x9877314451BDF26B, 0x5E3030A5F687241B, 0xC1BAA4543640AA19],
        "a change to how a voice renders changes these on purpose"
    );
}

/// The loudness of each tenth of the samples, the root of its mean square.
fn tenths(samples: &[f32]) -> Vec<f32> {
    samples
        .chunks(samples.len() / 10)
        .take(10)
        .map(|part| (part.iter().map(|s| s * s).sum::<f32>() / part.len() as f32).sqrt())
        .collect()
}

#[test]
fn a_hold_slopes_from_where_it_starts_to_where_it_ends() {
    let held = |from: f32, to: f32| {
        Voice::made(
            "held",
            Vary::NONE,
            vec![Layer::new(
                0.0,
                Source::Wave(Wave::Sine, Pitch::steady(500.0)),
                Envelope::new(0.0, 1.0, 0.0).sloping(from, to),
                Filter::Open,
                0.5,
            )],
        )
    };
    let swell = tenths(&rendered(&held(0.0, 1.0), 0));
    assert!(swell.windows(2).all(|pair| pair[1] > pair[0]), "{swell:?}");
    assert!(swell[0] < 0.05 && swell[9] > 0.3, "{swell:?}");
    let fade = tenths(&rendered(&held(1.0, 0.25), 0));
    assert!(fade.windows(2).all(|pair| pair[1] < pair[0]), "{fade:?}");
    // From about a half times 0.707 to a quarter of it.
    assert!((fade[0] / fade[9] - 3.4).abs() < 0.6, "{fade:?}");
}

#[test]
fn a_fade_with_no_time_to_slope_releases_from_where_the_attack_ends() {
    // Five milliseconds in and five out, nothing held: what a ten
    // millisecond tone that fades is. Its release starts where its attack
    // ended, so it falls as one that stays does, never with a jump.
    let short = |from: f32, to: f32| {
        Voice::made(
            "short",
            Vary::NONE,
            vec![Layer::new(
                0.0,
                Source::Wave(Wave::Sine, Pitch::steady(500.0)),
                Envelope::new(0.005, 0.0, 0.005).sloping(from, to),
                Filter::Open,
                0.5,
            )],
        )
    };
    assert_eq!(rendered(&short(1.0, 0.0), 0), rendered(&short(1.0, 1.0), 0));
}

#[test]
fn a_sweep_slides_the_filters_edge_by_equal_ratios() {
    let swept = Voice::made(
        "whoosh",
        Vary::NONE,
        vec![
            Layer::new(
                0.0,
                Source::Noise,
                Envelope::new(0.0, 1.0, 0.0),
                Filter::Low { hz: 6000.0, q: 0.7 },
                0.5,
            )
            .swept(375.0, 1.0),
        ],
    );
    let samples = rendered(&swept, 3);
    let second = RATE as usize;
    let early = &samples[..second / 10];
    let late = &samples[second * 9 / 10..second];
    // 3000 Hz is under the edge at the start and four octaves over it at
    // the end; 200 Hz is under it throughout.
    let high = power(early, 3000.0) / power(late, 3000.0);
    assert!(high > 100.0, "{high}");
    let low = power(early, 200.0) / power(late, 200.0);
    assert!((0.1..10.0).contains(&low), "{low}");
}

#[test]
fn the_same_seed_the_same_samples_another_seed_others() {
    assert_eq!(rendered(&EVERYTHING, 3), rendered(&EVERYTHING, 3));
    assert_ne!(rendered(&EVERYTHING, 3), rendered(&EVERYTHING, 4));
}

#[test]
fn no_sample_leaves_minus_one_to_one() {
    // Gains summing to three and a filter that rings: held all the same.
    let loud = Voice::made(
        "loud",
        Vary::NONE,
        vec![
            Layer {
                at_s: 0.0,
                source: Source::Wave(Wave::Square, Pitch::steady(120.0)),
                envelope: Envelope::new(0.0, 0.2, 0.0),
                filter: Filter::Low { hz: 400.0, q: 20.0 },
                gain: 1.5,
                sweep: None,
            },
            Layer {
                at_s: 0.0,
                source: Source::Noise,
                envelope: Envelope::new(0.0, 0.2, 0.0),
                filter: Filter::Open,
                gain: 1.5,
                sweep: None,
            },
        ],
    );
    for voice in [&EVERYTHING, &loud] {
        for seed in 0..8 {
            let samples = rendered(voice, seed);
            assert!(samples.iter().all(|s| (-1.0..=1.0).contains(s)));
        }
    }
}

#[test]
fn a_voice_ends_in_silence() {
    for seed in 0..8 {
        let samples = rendered(&EVERYTHING, seed);
        let last_ms = &samples[samples.len() - RATE as usize / 1000..];
        let peak = last_ms.iter().fold(0.0f32, |peak, s| peak.max(s.abs()));
        assert!(peak < 1e-3, "seed {seed}: {peak} in its last millisecond");
        // And nothing is cut: the buffer is as long as the longest layer.
        let mut longer = vec![0.0; samples.len() + 480];
        EVERYTHING.render(seed, RATE, &mut longer);
        assert!(longer[samples.len()..].iter().all(|s| *s == 0.0));
    }
}

#[test]
fn a_layer_let_go_at_once_still_lands_on_silence() {
    let cut = Voice::made(
        "cut",
        Vary::NONE,
        vec![Layer {
            at_s: 0.0,
            source: Source::Wave(Wave::Square, Pitch::steady(220.0)),
            envelope: Envelope::new(0.0, 0.1, 0.0),
            filter: Filter::Open,
            gain: 0.5,
            sweep: None,
        }],
    );
    let samples = rendered(&cut, 0);
    // The hold, then two milliseconds down to zero.
    assert_eq!(samples.len(), 4800 + 96);
    let last = samples[samples.len() - 1].abs();
    assert!(last < 1e-3, "{last} at the end");
}

#[test]
fn numbers_out_of_bounds_sound_short_and_within_bounds() {
    let wild = Voice::made(
        "wild",
        Vary {
            cents: f32::NAN,
            gain: 1e30,
            time: f32::INFINITY,
        },
        vec![
            Layer {
                at_s: f32::INFINITY,
                source: Source::Wave(Wave::Saw, Pitch::steady(f32::NAN).glide(1e9, f32::INFINITY)),
                envelope: Envelope::new(f32::NAN, f32::INFINITY, -1.0),
                filter: Filter::Low {
                    hz: f32::INFINITY,
                    q: f32::NAN,
                },
                gain: f32::NAN,
                sweep: None,
            },
            Layer {
                at_s: 0.0,
                source: Source::Noise,
                envelope: Envelope::new(0.01, 0.05, 0.05),
                filter: Filter::Band { hz: -5.0, q: 0.0 },
                gain: f32::INFINITY,
                sweep: None,
            },
        ],
    );
    for seed in 0..4 {
        let length = wild.length(seed, RATE);
        assert!(length <= 4 * (synth::LONGEST_S * RATE as f32) as usize + 4);
        let mut samples = vec![0.0; length];
        wild.render(seed, RATE, &mut samples);
        assert!(samples.iter().all(|s| (-1.0..=1.0).contains(s)));
    }
}

#[test]
fn a_length_is_its_longest_layer() {
    let voice = tone(Wave::Sine, Pitch::steady(440.0), 0.1);
    // 5 ms in, 100 ms held, 5 ms out.
    assert_eq!(voice.length(0, RATE), 240 + 4800 + 240);
    assert_eq!(rendered(&voice, 0).len(), 5280);
}

#[test]
fn every_wave_sounds_at_its_pitch() {
    for wave in [Wave::Sine, Wave::Triangle, Wave::Saw, Wave::Square] {
        let samples = rendered(&tone(wave, Pitch::steady(440.0), 0.5), 0);
        assert_eq!(loudest(&samples, 100.0, 4000.0, 10.0), 440.0, "{wave:?}");
    }
}

#[test]
fn a_glide_moves_the_pitch_by_equal_ratios() {
    // Two octaves up in a second: one up at half a second.
    let voice = tone(Wave::Sine, Pitch::steady(400.0).glide(1600.0, 1.0), 1.2);
    let samples = rendered(&voice, 0);
    let at = |s: f32| {
        let start = (s * RATE as f32) as usize;
        loudest(&samples[start..start + 2400], 200.0, 2000.0, 10.0)
    };
    assert!((at(0.02) - 420.0).abs() < 60.0, "{}", at(0.02));
    assert!((at(0.475) - 800.0).abs() < 60.0, "{}", at(0.475));
    assert!((at(1.05) - 1600.0).abs() < 60.0, "{}", at(1.05));
}

#[test]
fn noise_through_a_band_sits_in_the_band() {
    let voice = Voice::made(
        "band",
        Vary::NONE,
        vec![Layer {
            at_s: 0.0,
            source: Source::Noise,
            envelope: Envelope::new(0.0, 0.5, 0.0),
            filter: Filter::Band { hz: 2000.0, q: 8.0 },
            gain: 1.0,
            sweep: None,
        }],
    );
    let samples = rendered(&voice, 9);
    let inside = power(&samples, 2000.0);
    for outside in [300.0, 600.0, 6000.0, 12000.0] {
        assert!(
            inside > 30.0 * power(&samples, outside),
            "{outside} Hz against 2000"
        );
    }
}
