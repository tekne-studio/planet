//! Pictures of a pegasus brought down on a tree, with a fixed clock and
//! seed: how coming down on a crown is looked at. `bun run shot --perch
//! out/perch` mounts the nearest pegasus and finds near it a tree standing
//! alone on the gentlest ground whose trunk rises 4 m and more, and the tree
//! as tall with the most trunks within [`STAND`] blocks of its own. It puts
//! the pegasus aloft over the middle of a trunk's top, lands it as F does
//! and holds no key until it has stood still a second, at most `LAST_S`;
//! then forward is held `OFF_S`. The cases: over the lone tree facing
//! across the sun (`across`) and facing it (`sunward`); over the tree in
//! the stand (`stand`); and over a deck laid on pillars a cell over the
//! lone tree's crown (`deck`), where it lands as with no tree. Every sixth
//! frame from the press is drawn from its side, the sun behind the eye, and
//! from behind it, the eye standing where it is first set: each frame is
//! `<case>/<view>-<ms>.png`, and `perch.json` lists each one's height over
//! the ground and how far its feet are from the trunk's middle.

use std::fmt::Write as _;
use std::path::Path;

use client::{Client, Input, Key, Seat};
use gaia::fauna::Ground;
use gaia::flora::{Flora, Plant};
use glam::DVec3;
use render::write_png;
use topology::{BLOCK_M, Column, SurfacePoint};
use voxel::{Gesture, Span};

use crate::animals::Land;
use crate::shot::drawn;
use crate::takeoff::{picture, tick};

/// How high over the trunk's top, or the deck's, the pegasus is set,
/// metres.
const OVER_M: f64 = 4.0;
/// How much the trunk rises over the ground at least, metres.
const TALL_M: f64 = 4.0;
/// How far around the pegasus a tree is looked for, squares of 32 blocks
/// each way; how near another may stand to a lone one, blocks; and how far
/// from it the ground is read for how much it rises, metres.
const SQUARES: i64 = 8;
const SQUARE: u32 = 32;
const ALONE: u16 = 16;
const FLAT_M: f64 = 12.0;
/// How near the trunks of a stand are counted, blocks.
const STAND: u16 = 6;
/// How far the deck reaches from the lone tree's root each way, inside the
/// root's plot, blocks.
const DECK: i32 = 8;
/// Drawn every this many frames of 60 a second.
const EVERY: u32 = 6;
/// The longest it is drawn landing, and how long it walks off, seconds;
/// how long it stands still to be done landing.
const LAST_S: f64 = 8.0;
const OFF_S: f64 = 1.5;
const STILL_S: f64 = 1.0;
/// From its side and from behind: how far away the eye stands, how high
/// over the floor it ends on, over the ground under the eye, and how high
/// over that floor at the trunk it looks at, metres.
const SIDE: [f64; 3] = [16.0, 3.0, 4.0];
const BEHIND: [f64; 3] = [16.0, 4.0, 4.0];
/// In the stand, both from under the crowns.
const AMONG: [f64; 3] = [8.0, 1.6, 2.0];

/// A tree it is brought down on: its root column and its trunk's top,
/// metres over the datum.
#[derive(Clone, Copy)]
struct Tree {
    root: Column,
    top_m: f64,
    /// Where its crown ends, blocks over the datum.
    crown: i32,
}

/// Writes the frames into `dir`.
pub fn run(client: &mut Client, size: [u32; 2], dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut input = Input::default();
    crate::shot::ride(client, true)?;
    let generator = client.generator().clone();
    let (grid, radius_m) = (generator.sphere().blocks(), generator.sphere().radius_m());
    let land = Land(&generator);
    let flora = Flora::grow(&generator);
    let plants = near(&land, &flora, client.host("ride").body().position);
    let lone =
        alone(&land, &flora, &plants).ok_or("--perch: no tree standing alone near the pegasus")?;
    let stand = stand(&flora, &plants).ok_or("--perch: no stand of trees near the pegasus")?;
    let (mut headless, base) = drawn(client, size)?;
    let sun = base.sun_direction.as_dvec3();
    let mut listed = String::from("[\n");
    for case in ["across", "sunward", "stand", "deck"] {
        let tree = if case == "stand" { stand } else { lone };
        let middle = SurfacePoint::center_of(tree.root);
        let up = DVec3::from(grid.direction(middle));
        let foot = land.under(up).point;
        let ground_m = foot.length() - radius_m;
        // How high the floor it ends on is over the ground at the trunk.
        let floor_m = match case {
            "deck" => decked(client, tree)? - ground_m,
            _ => 0.0,
        };
        log::info!(
            "{case}: a trunk at {:?}, {:.1} m over the ground, ending {floor_m:.1} m over it",
            tree.root,
            tree.top_m - ground_m
        );
        let tangent = |v: DVec3| (v - up * v.dot(up)).normalize_or(up.any_orthonormal_vector());
        let across = up.cross(tangent(sun));
        let facing = if case == "sunward" {
            tangent(sun)
        } else {
            across
        };
        let mut pose = client.here();
        pose.column = tree.root;
        let over_m = tree.top_m.max(ground_m + floor_m) + OVER_M;
        pose.h = Some((over_m / BLOCK_M).round() as i16);
        pose.bearing_deg = topology::bearing_deg(up.to_array(), facing.to_array());
        pose.pitch_deg = Some(0.0);
        client.go_to(&pose.text(grid))?;
        if !client.host("ride").aloft() {
            return Err(format!(
                "--perch {case}: the pegasus is not aloft over the tree"
            ));
        }
        let right = facing.cross(up);
        let side = match right.dot(sun) >= 0.0 {
            true => right,
            false => -right,
        };
        let [beside, behind] = if case == "stand" {
            [AMONG; 2]
        } else {
            [SIDE, BEHIND]
        };
        let views = [("side", beside, side), ("behind", behind, -facing)];
        let folder = dir.join(case);
        std::fs::create_dir_all(&folder).map_err(|e| format!("{}: {e}", folder.display()))?;
        client.drain_events();
        client.command_json(r#"{"type":"set_mode","mode":"ride.horse"}"#);
        let (mut frame, mut still, mut was) = (0_u32, 0_u32, None);
        let last = (LAST_S * 60.0) as u32;
        let off = last + (OFF_S * 60.0) as u32;
        let mut walking = None;
        loop {
            if frame % EVERY == 0 {
                let at = client.host("ride").body().position;
                let over_m = (at - land.under(at).point).dot(up);
                let aside_m = at.normalize().angle_between(up) * radius_m;
                let aloft = client.host("ride").aloft();
                let ms = frame * 1000 / 60;
                for (view, [away, eye, look], from) in views {
                    let target = foot + up * (floor_m + look);
                    let eye = land.under(foot + from * away).point + up * (floor_m + eye);
                    let picture = picture(client, &mut headless, eye, target, up);
                    let out = folder.join(format!("{view}-{ms:05}.png"));
                    write_png(&out, size[0], size[1], &picture)?;
                }
                let _ = writeln!(
                    listed,
                    r#"  {{"case":"{case}","ms":{ms},"over_m":{over_m:.3},"aside_m":{aside_m:.3},"aloft":{aloft},"walking":{}}},"#,
                    walking.is_some()
                );
            }
            if walking.is_some_and(|from| frame >= from + (off - last)) {
                break;
            }
            let at = client.host("ride").body().position;
            let stands = !client.host("ride").aloft()
                && was.is_some_and(|was: DVec3| (at - was).length() < 1e-3);
            still = if stands { still + 1 } else { 0 };
            was = Some(at);
            if walking.is_none() && (still as f64 >= STILL_S * 60.0 || frame >= last) {
                walking = Some(frame);
                input.key(Key::Forward, true);
            }
            tick(client, &mut input, 0.0);
            frame += 1;
        }
        input.key(Key::Forward, false);
        tick(client, &mut input, 0.0);
        for event in client.drain_events() {
            if let client::Event::Rejected { message } = event {
                log::warn!("{case}: {message}");
            }
        }
        // Aloft again for the next.
        client.command_json(r#"{"type":"set_mode","mode":"fly"}"#);
        tick(client, &mut input, 2.0);
        log::info!("{case} -> {}", folder.display());
    }
    let listed = listed.trim_end_matches(",\n").to_owned() + "\n]\n";
    let out = dir.join("perch.json");
    std::fs::write(&out, listed).map_err(|e| format!("{}: {e}", out.display()))?;
    Ok(())
}

/// The plants rooted within [`SQUARES`] squares of `at`.
fn near(land: &Land, flora: &Flora, at: DVec3) -> Vec<Plant> {
    let generator = land.0;
    let grid = generator.sphere().blocks();
    let here = grid.surface_point(at.to_array());
    let side = i64::from(grid.side());
    let square = |n: f64| (n as i64).div_euclid(i64::from(SQUARE));
    let [u, v] = [here.u, here.v].map(square);
    let corners = (-SQUARES..=SQUARES)
        .flat_map(|j| (-SQUARES..=SQUARES).map(move |i| [u + i, v + j]))
        .filter(|corner| {
            corner
                .iter()
                .all(|&n| n >= 0 && (n + 1) * i64::from(SQUARE) <= side)
        });
    corners
        .flat_map(|[i, j]| {
            let corner = [i, j].map(|n| (n * i64::from(SQUARE)) as u32);
            flora.plants(generator, here.sector, corner, SQUARE)
        })
        .collect()
}

/// The tree among `plants` on the gentlest ground whose trunk is one
/// column rising [`TALL_M`] and more, with no other within [`ALONE`]
/// blocks. How gentle is the most the ground rises or falls [`FLAT_M`]
/// from it.
fn alone(land: &Land, flora: &Flora, plants: &[Plant]) -> Option<Tree> {
    let grid = land.0.sphere().blocks();
    let alone = |plant: &Plant| {
        plants
            .iter()
            .all(|other| other == plant || apart(other, plant) > ALONE)
    };
    let rise_m = |plant: &Plant| {
        let up = DVec3::from(grid.direction(SurfacePoint::center_of(plant.root.column)));
        let foot = land.under(up).point;
        let east = up.any_orthonormal_vector();
        let north = up.cross(east);
        [east, north, -east, -north]
            .map(|way| (land.under(foot + way * FLAT_M).point - foot).dot(up).abs())
            .into_iter()
            .fold(0.0, f64::max)
    };
    plants
        .iter()
        .filter(|&&plant| flora.trunk(plant).count() == 1 && alone(&plant))
        .filter_map(|&plant| tall(flora, plant))
        .min_by(|a, b| rise_m(&a.0).total_cmp(&rise_m(&b.0)))
        .map(|(_, tree)| tree)
}

/// The tree among `plants` with a trunk rising [`TALL_M`] and more and the
/// most other trunks within [`STAND`] blocks of its root.
fn stand(flora: &Flora, plants: &[Plant]) -> Option<Tree> {
    let plants: Vec<Plant> = (plants.iter().copied())
        .filter(|&plant| flora.trunk(plant).next().is_some())
        .collect();
    let neighbours = |plant: &Plant| {
        plants
            .iter()
            .filter(|other| *other != plant && apart(other, plant) <= STAND)
            .count()
    };
    plants
        .iter()
        .filter_map(|&plant| tall(flora, plant))
        .max_by_key(|(plant, _)| neighbours(plant))
        .map(|(_, tree)| tree)
}

/// How far apart two plants' roots are, the longer way, blocks.
fn apart(one: &Plant, other: &Plant) -> u16 {
    let [one, other] = [one.root.column, other.root.column];
    one.u.abs_diff(other.u).max(one.v.abs_diff(other.v))
}

/// A plant as a [`Tree`], when its trunk rises [`TALL_M`] and more.
fn tall(flora: &Flora, plant: Plant) -> Option<(Plant, Tree)> {
    let top = flora.trunk(plant).map(|(_, layers)| layers.end).max()?;
    let shape = &flora.shapes()[usize::from(plant.shape)];
    let root = i32::from(plant.root.h);
    let ground_m = f64::from(root + 1) * BLOCK_M;
    let top_m = f64::from(root + top) * BLOCK_M;
    let crown = root + shape.low()[2] + shape.size()[2] as i32;
    let tree = Tree {
        root: plant.root.column,
        top_m,
        crown,
    };
    (top_m - ground_m >= TALL_M).then_some((plant, tree))
}

/// Opens the volume over `tree`'s root and lays a deck over it, [`DECK`]
/// blocks each way inside the root's plot, its slab a cell over the
/// crown, on a pillar at each corner from under the ground there: a cell in the
/// root's column over its crown leaves the tree standing. How high the
/// deck's top is over the datum, metres.
fn decked(client: &mut Client, tree: Tree) -> Result<f64, String> {
    let generator = client.generator().clone();
    let root = SurfacePoint::center_of(tree.root);
    let seat = Seat::Sector(root.sector);
    let plot = seat::plot_of(root).map(|n| n << seat::PLOT_BITS);
    let [u, v] = [tree.root.u, tree.root.v].map(i32::from);
    let side = 1 << seat::PLOT_BITS;
    let from = [(u - DECK).max(plot[0]), (v - DECK).max(plot[1])];
    let to = [
        (u + DECK).min(plot[0] + side - 1),
        (v + DECK).min(plot[1] + side - 1),
    ];
    let slab = tree.crown + 1;
    let mut gestures = vec![Gesture::Create {
        span: Span::between([from[0], from[1], slab], [to[0], to[1], slab]),
        paint: 1,
    }];
    for x in [from[0], to[0]] {
        for y in [from[1], to[1]] {
            let column = SurfacePoint::new(root.sector, f64::from(x), f64::from(y));
            let foot = seat::ground(&generator, seat, column).floor() as i32 - 1;
            gestures.push(Gesture::Create {
                span: Span::between([x, y, foot], [x, y, slab - 1]),
                paint: 4,
            });
        }
    }
    let mut host = client.host("ride");
    host.open(seat, root)
        .map_err(|e| format!("--perch: {e:?}"))?;
    host.apply(seat, &gestures)
        .map_err(|e| format!("--perch: {e:?}"))?;
    Ok(f64::from(slab + 1) * BLOCK_M)
}
