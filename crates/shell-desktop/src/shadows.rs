//! Strips of the sun's shadows a sixtieth of a second apart, with a fixed
//! clock and seed: how shadows holding still is looked at. `bun run shot
//! --shadows out/shadows` writes, for each view of the flattest wood near,
//! three strips and how much changes from each picture to the next: the
//! sun moving as the clock moves it (A), nothing moving (B), and the eye
//! moving, the sun held (C). Only the sun, or the eye, moves: water, wind
//! and animals stay as the first picture has them.
//!
//! Each strip is drawn twice, with the shadows and without: what a change
//! with them has over the same change without is the shadows' own, so the
//! ground a moving eye sees slide is not counted as theirs.

use std::fmt::Write as _;
use std::path::Path;

use client::Client;
use gaia::fauna::Footing;
use glam::DVec3;
use render::{Headless, write_png};
use worldgen::Material;

use crate::animals::{Land, flattest_where, looking};
use crate::shot::drawn_into;

/// Pictures in a strip, a sixtieth of a second apart.
const FRAMES: usize = 60;
/// The changes from one picture to the next written as pictures, the first.
const SHOWN: usize = 8;
/// How much a change is magnified in its picture.
const MAGNIFIED: u8 = 8;
/// A change of a channel of this many levels or fewer is not counted.
const NOISE: u8 = 2;
/// The views, by name, and how high over the ground each stands looking
/// straight down, metres: none, the shot's own eye; then in the first
/// cascade, and in the second.
const VIEWS: [(&str, Option<f64>); 3] =
    [("eye", None), ("near", Some(25.0)), ("high", Some(250.0))];
/// How far a pose can look down, radians: the eye is then turned the rest.
const POSED_DOWN: f64 = 1.45;

/// What moves through a strip.
#[derive(Clone, Copy, PartialEq)]
enum Moving {
    /// The sun, as the clock moves it.
    Sun,
    /// Nothing.
    Nothing,
    /// The eye, across the picture a pixel a frame.
    Eye,
}

impl Moving {
    fn letter(self) -> &'static str {
        match self {
            Moving::Sun => "a",
            Moving::Nothing => "b",
            Moving::Eye => "c",
        }
    }
}

/// Writes into `dir`, for each view and strip, its first picture, its
/// first [`SHOWN`] changes and the most of the shadows' own change through
/// it, magnified; and `shadows.json`, how many pixels the shadows change
/// from each picture to the next.
pub fn run(client: &mut Client, size: [u32; 2], dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut headless = Headless::new(size[0], size[1])?;
    // On flat ground the eye moving from above slides all of it alike; in a
    // wood, the trees cast.
    let here = DVec3::from(client.up()) * client.generator().sphere().radius_m();
    let wood = |footing: &Footing| footing.surface == Material::Forest as u8;
    let flat = flattest_where(&Land(client.generator()), here, wood);
    let grid = client.generator().sphere().blocks();
    client.teleport(grid.surface_point(flat.point.to_array()));
    client.update(1.0 / 60.0, &mut client::Input::default());
    let clock_s = client.settled_frame().clock_s;
    let mut json = String::from("{");
    for (name, above_m) in VIEWS {
        let mut base = drawn_into(client, &mut headless);
        // A pixel's side on the ground right under the eye.
        let mut pixel_m = None;
        if let Some(above_m) = above_m {
            let ahead = base.camera.rotation * DVec3::NEG_Z;
            client.pose(above_m, -POSED_DOWN, 0.0);
            base = drawn_into(client, &mut headless);
            let up = base.camera.position.normalize();
            base.camera.rotation = looking(-up, (ahead - up * ahead.dot(up)).normalize());
            let half = f64::from(base.camera.fov_y / 2.0).tan();
            pixel_m = Some(2.0 * above_m * half / f64::from(size[1]));
        }
        for moving in [Moving::Sun, Moving::Nothing, Moving::Eye] {
            // From the body's eye a moving eye changes the whole picture: only
            // from above does it slide the ground a whole pixel.
            let across = match moving {
                Moving::Eye => match pixel_m {
                    Some(pixel_m) => base.camera.rotation * DVec3::X * pixel_m,
                    None => continue,
                },
                _ => DVec3::ZERO,
            };
            let label = format!("{name}-{}", moving.letter());
            let strip = |shadows: bool, client: &mut Client, headless: &mut Headless| {
                (0..FRAMES)
                    .map(|picture| {
                        let mut frame = base.clone();
                        frame.effects.shadows = shadows;
                        frame.camera.position += across * picture as f64;
                        if moving == Moving::Sun {
                            client.set_clock(clock_s + picture as f64 / 60.0);
                            let now = client.settled_frame();
                            frame.sun_direction = now.sun_direction;
                            frame.sun_steps = now.sun_steps;
                        }
                        headless.render(&frame)
                    })
                    .collect::<Vec<_>>()
            };
            let lit = strip(true, client, &mut headless);
            let bare = strip(false, client, &mut headless);
            client.set_clock(clock_s);
            write_png(&dir.join(format!("{label}.png")), size[0], size[1], &lit[0])?;
            let shift = usize::from(moving == Moving::Eye);
            let mut most = vec![0u8; lit[0].len()];
            let mut touched = Vec::new();
            for picture in 1..FRAMES {
                let with = changed(&lit[picture - 1], &lit[picture], size, shift);
                let without = changed(&bare[picture - 1], &bare[picture], size, shift);
                if picture <= SHOWN {
                    let out = dir.join(format!("{label}-change-{:02}.png", picture - 1));
                    write_png(&out, size[0], size[1], &magnified(&with))?;
                }
                let mut count = 0u64;
                for ((most, with), without) in most
                    .chunks_exact_mut(4)
                    .zip(with.chunks_exact(4))
                    .zip(without.chunks_exact(4))
                {
                    let own = (0..3)
                        .map(|channel| with[channel].saturating_sub(without[channel]))
                        .max()
                        .unwrap_or(0);
                    let level = most[0].max(own);
                    most[..3].fill(level);
                    count += u64::from(own > NOISE);
                }
                touched.push(count);
            }
            most.chunks_exact_mut(4).for_each(|pixel| pixel[3] = 255);
            let out = dir.join(format!("{label}-most.png"));
            write_png(&out, size[0], size[1], &magnified(&most))?;
            let mean = touched.iter().sum::<u64>() / touched.len() as u64;
            log::info!("{label}: the shadows change {mean} pixels a picture, on average");
            let comma = if json.len() > 1 { "," } else { "" };
            let _ = write!(json, "{comma}\n  \"{label}\": {touched:?}");
        }
    }
    json.push_str("\n}\n");
    let out = dir.join("shadows.json");
    std::fs::write(&out, json).map_err(|e| format!("{}: {e}", out.display()))?;
    log::info!("strips of the sun's shadows -> {}", dir.display());
    Ok(())
}

/// How far each channel of each pixel of `now` is from the one `shift`
/// pixels to its right in `before`: the ground an eye moving right a pixel
/// a frame sees in the same place. The columns past the edge stay at zero.
fn changed(before: &[u8], now: &[u8], [width, height]: [u32; 2], shift: usize) -> Vec<u8> {
    let (width, height) = (width as usize, height as usize);
    let mut change = vec![0u8; width * height * 4];
    for y in 0..height {
        for x in 0..width - shift {
            let at = (y * width + x) * 4;
            let was = (y * width + x + shift) * 4;
            for channel in 0..3 {
                change[at + channel] = now[at + channel].abs_diff(before[was + channel]);
            }
        }
    }
    change
}

/// A change made visible: its most in a channel [`MAGNIFIED`] times, past
/// the noise, grey on black.
fn magnified(change: &[u8]) -> Vec<u8> {
    change
        .chunks_exact(4)
        .flat_map(|pixel| {
            let most = pixel[..3].iter().copied().max().unwrap_or(0);
            let level = match most > NOISE {
                true => most.saturating_mul(MAGNIFIED),
                false => 0,
            };
            [level, level, level, 255]
        })
        .collect()
}
