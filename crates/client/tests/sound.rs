//! Voices struck at a place: the whistle, by key and by the seam, and what
//! it sounds like.

use client::{Client, Command, Event, Group, Input, Key, Recipe, Strike, VARIANTS, voices};
use glam::DVec3;

fn frames(client: &mut Client, input: &mut Input, seconds: f64) -> Vec<Strike> {
    let mut struck = Vec::new();
    for _ in 0..(seconds * 60.0) as u32 {
        client.update(1.0 / 60.0, input);
        struck.extend(client.drain_strikes());
    }
    struck
}

#[test]
fn nothing_sounds_unasked_and_each_strike_takes_the_next_seed() {
    let mut client = Client::new(Recipe::new(1)).unwrap();
    let mut input = Input::default();
    // Standing and walking, the body is silent.
    assert!(frames(&mut client, &mut input, 1.0).is_empty());
    input.key(Key::Forward, true);
    assert!(frames(&mut client, &mut input, 2.0).is_empty());
    input.key(Key::Forward, false);

    // Whistled again and again, no two in a row are one sample, each a call.
    let mut seeds = Vec::new();
    for _ in 0..VARIANTS + 1 {
        client.command(Command::Sound {
            voice: "whistle".into(),
        });
        for strike in frames(&mut client, &mut input, 0.1) {
            assert_eq!(strike.group, Group::Calls);
            seeds.push(strike.seed);
        }
    }
    assert_eq!(seeds.len(), VARIANTS as usize + 1);
    assert!(seeds.windows(2).all(|pair| pair[0] != pair[1]), "{seeds:?}");
    assert!(seeds.iter().all(|&seed| seed < VARIANTS));
}

#[test]
fn a_whistle_sounds_at_the_mouth_and_an_unknown_voice_is_refused() {
    let mut client = Client::new(Recipe::new(1)).unwrap();
    let mut input = Input::default();
    frames(&mut client, &mut input, 0.5);
    input.key(Key::Whistle, true);
    let struck = frames(&mut client, &mut input, 0.1);
    assert_eq!(struck.len(), 1);
    assert_eq!(struck[0].voice, voices::WHISTLE);
    assert_eq!(struck[0].reach_m, 150.0);
    // From the mouth: a metre and a half over the ground the body stands on.
    let (mouth, up) = (struck[0].position, DVec3::from(client.up()));
    assert!(mouth.normalize().dot(up) > 1.0 - 1e-9);
    let ground_m = client.generator().sample(up.to_array()).height_m;
    let above_m = mouth.length() - client.sphere().radius_m() - ground_m;
    assert!((above_m - 1.5).abs() < 0.1, "{above_m} m over the ground");

    // An agent asks the same by the seam.
    client.command_json(r#"{"type":"sound","voice":"whistle"}"#);
    assert_eq!(frames(&mut client, &mut input, 0.1).len(), 1);

    client.command(Command::Sound {
        voice: "neigh".into(),
    });
    let refused = client
        .drain_events()
        .into_iter()
        .any(|event| matches!(event, Event::Rejected { message } if message.contains("neigh")));
    assert!(refused);
}

#[test]
fn a_strike_nobody_takes_is_let_go_a_frame_later() {
    let mut client = Client::new(Recipe::new(1)).unwrap();
    let mut input = Input::default();
    client.command(Command::Sound {
        voice: "whistle".into(),
    });
    client.update(1.0 / 60.0, &mut input);
    client.update(1.0 / 60.0, &mut input);
    assert!(client.drain_strikes().is_empty());
}

const RATE: u32 = 48_000;

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

#[test]
fn a_whistle_sounds_where_its_pitch_says() {
    for seed in 0..client::VARIANTS {
        let mut samples = vec![0.0; voices::WHISTLE.length(seed, RATE)];
        voices::WHISTLE.render(seed, RATE, &mut samples);
        // Up from 1400 Hz to 2500 by 0.22 s, held; then from 2500 down to
        // 1300 over 0.4 s from 0.32 s.
        let up = |s: f32| 1400.0 * (2500.0f32 / 1400.0).powf((s / 0.22).min(1.0));
        let down = |s: f32| 2500.0 * (1300.0f32 / 2500.0).powf(((s - 0.32) / 0.4).min(1.0));
        for (at_s, hz) in [
            (0.1, up(0.1)),
            (0.24, up(0.24)),
            (0.45, down(0.45)),
            (0.7, down(0.7)),
        ] {
            let start = (at_s * RATE as f32) as usize - 512;
            let window = &samples[start..start + 1024];
            let loudest = (40..=400)
                .map(|step| step as f32 * 10.0)
                .max_by(|a, b| power(window, *a).total_cmp(&power(window, *b)))
                .unwrap();
            assert!(
                (loudest / hz - 1.0).abs() < 0.08,
                "seed {seed} at {at_s} s: loudest {loudest} Hz, the pitch says {hz}"
            );
        }
    }
}

#[test]
fn with_no_pet_the_stats_say_a_whistle_calls_no_one() {
    let mut client = Client::new(Recipe::new(1)).unwrap();
    let mut input = Input::default();
    frames(&mut client, &mut input, 1.0);
    let calls: Vec<bool> = client
        .drain_events()
        .into_iter()
        .filter_map(|event| match event {
            Event::Stats { calls, .. } => Some(calls),
            _ => None,
        })
        .collect();
    assert!(!calls.is_empty() && calls.iter().all(|&calls| !calls));
}
