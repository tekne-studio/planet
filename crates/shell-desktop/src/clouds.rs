//! Pictures of a pegasus flown up to the clouds, with a fixed clock and
//! seed: how flight at height is looked at. `bun run shot --clouds
//! out/clouds` mounts the nearest pegasus, takes off on it, and moves it
//! aloft, over where it rose, to each of [`HEIGHTS`]: under the clouds,
//! at their base, in them and at its ceiling, their top. At each it waits
//! until what streams there has come, then draws the rider's view from
//! behind looking up, level and down, and from the rider's eyes; and it
//! prices a frame there, the client's step and the render's. Last it flies
//! level through the clouds' lower half, forward held, and draws the
//! rider's view each second, into `through/<s>.png`. A shot's
//! `--effects` sets the weather: `{"cloud_cover":0.9}` fills the layer, so
//! the eye is inside a cloud. Each picture is
//! `<height>/<view>.png`, and `clouds.json` lists the heights with what the
//! readout said and the prices.

use std::fmt::Write as _;
use std::path::Path;
use std::time::Instant;

use client::{Client, Input, Key};
use render::write_png;

use crate::shot::{drawn, drawn_into};

/// The heights flown to, metres over the sea: the ceiling a pegasus had,
/// under the clouds, in their base, in their middle, and their top.
const HEIGHTS: [u32; 5] = [500, 1000, 1200, 2000, scene::CLOUD_TOP_M as u32];
/// The rider's views, by the camera's pitch, degrees, and whether the
/// camera is at the eyes. The camera stays the client's own, so what it
/// draws is what it streamed for that camera.
const VIEWS: [(&str, f64, bool); 4] = [
    ("up", 20.0, false),
    ("level", 0.0, false),
    ("down", -30.0, false),
    ("eyes", 0.0, true),
];
/// The longest a height is waited on for what streams there, seconds.
const SETTLE_MOST_S: f64 = 30.0;
/// How long a view is held before it is drawn, seconds.
const SPOKEN_S: f64 = 3.0;
/// How many frames a price is read over.
const PRICED: u32 = 120;
/// How high it flies through the clouds, metres over the sea, and for how
/// many seconds.
const THROUGH_M: f64 = 1400.0;
const THROUGH_S: u32 = 40;

/// Writes the pictures into `dir`, the camera from behind `boom_m` back.
pub fn run(client: &mut Client, size: [u32; 2], boom_m: f64, dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut input = Input::default();
    crate::shot::ride(client, true)?;
    tick(client, &mut input, 3.0);
    let generator = client.generator().clone();
    let grid = generator.sphere().blocks();
    let (mut headless, _) = drawn(client, size)?;
    let mut listed = String::from("[\n");
    for height_m in HEIGHTS {
        let folder = dir.join(format!("{height_m:04}"));
        std::fs::create_dir_all(&folder).map_err(|e| format!("{}: {e}", folder.display()))?;
        let (mut pace, mut altitude_m) = (None, 0.0);
        let mut settled_s = None;
        for (view, pitch_deg, eyes) in VIEWS {
            let mut pose = client.here();
            pose.h = Some(blocks(f64::from(height_m)));
            pose.pitch_deg = Some(pitch_deg);
            pose.boom_m = Some(if eyes { 0.0 } else { boom_m });
            client.go_to(&pose.text(grid))?;
            // What streams there comes once, at the first view.
            let mut waited_s = 0.0;
            while settled_s.is_none() && waited_s < SETTLE_MOST_S {
                tick(client, &mut input, 0.0);
                waited_s += 1.0 / 60.0;
                let events = client.drain_events();
                if events.iter().any(|e| matches!(e, client::Event::Settled)) {
                    settled_s = Some(waited_s);
                }
            }
            // Once the readout has spoken, it says what it says there; and
            // what streams as the eye turns has come.
            tick(client, &mut input, SPOKEN_S);
            for event in client.drain_events() {
                if let client::Event::Stats {
                    pace: said,
                    altitude_m: over_m,
                    ..
                } = event
                {
                    (pace, altitude_m) = (said, over_m);
                }
            }
            let frame = drawn_into(client, &mut headless);
            let out = folder.join(format!("{view}.png"));
            write_png(&out, size[0], size[1], &headless.render(&frame))?;
        }
        if !client.host("ride").aloft() {
            return Err(format!("--clouds: the pegasus came down at {height_m} m"));
        }
        let body = client.host("ride").body();

        // The price there: the client's step, then the render's of the
        // rider's view from the eyes.
        let started = Instant::now();
        tick(client, &mut input, f64::from(PRICED) / 60.0);
        let update_ms = started.elapsed().as_secs_f64() * 1000.0 / f64::from(PRICED);
        let frame = drawn_into(client, &mut headless);
        let (render_ms, render_p95_ms) = headless.measure(&frame, PRICED as usize);
        let over_sea_m = body.position.length() - generator.sphere().radius_m();
        let _ = writeln!(
            listed,
            "  {{\"height_m\":{height_m},\"over_sea_m\":{over_sea_m:.1},\"over_ground_m\":{altitude_m:.1},\
             \"pace\":\"{}\",\"settled_s\":{},\"update_ms\":{update_ms:.2},\
             \"render_ms\":{render_ms:.2},\"render_p95_ms\":{render_p95_ms:.2}}},",
            pace.unwrap_or(""),
            settled_s.map_or("null".to_owned(), |s| format!("{s:.2}")),
        );
        log::info!(
            "{height_m} m: the readout says {pace:?}, update {update_ms:.2} ms, \
             render {render_ms:.2} ms -> {}",
            folder.display()
        );
    }
    let listed = listed.trim_end_matches(",\n").to_owned() + "\n]\n";
    let out = dir.join("clouds.json");
    std::fs::write(&out, listed).map_err(|e| format!("{}: {e}", out.display()))?;

    let folder = dir.join("through");
    std::fs::create_dir_all(&folder).map_err(|e| format!("{}: {e}", folder.display()))?;
    let mut pose = client.here();
    pose.h = Some(blocks(THROUGH_M));
    pose.pitch_deg = Some(0.0);
    pose.boom_m = Some(boom_m);
    client.go_to(&pose.text(grid))?;
    tick(client, &mut input, SPOKEN_S);
    input.key(Key::Forward, true);
    for s in 0..THROUGH_S {
        tick(client, &mut input, 1.0);
        let frame = drawn_into(client, &mut headless);
        let out = folder.join(format!("{s:02}.png"));
        write_png(&out, size[0], size[1], &headless.render(&frame))?;
    }
    input.key(Key::Forward, false);
    log::info!(
        "through, {:.0} m over the sea at {:.0} m/s -> {}",
        client.host("ride").body().position.length() - generator.sphere().radius_m(),
        client.host("ride").in_ways()[0].speed_mps,
        folder.display()
    );
    Ok(())
}

/// A height over the sea, metres, as a place says it.
fn blocks(height_m: f64) -> i16 {
    (height_m / topology::BLOCK_M).round() as i16
}

/// One frame of 60 a second, or as many as `s` seconds hold.
fn tick(client: &mut Client, input: &mut Input, s: f64) {
    for _ in 0..(s * 60.0).round().max(1.0) as u32 {
        client.update(1.0 / 60.0, input);
    }
}
