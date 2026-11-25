//! Headless render to PNG with a fixed clock and seed: how a change is looked
//! at without opening a window.

use client::Client;
use render::{Headless, write_png};

use crate::args::{Shot, Step};
use crate::assets;

/// How many animals, nearest first, are tried for one with wings.
pub(crate) const TRIED: usize = 24;

/// Stands the body where the nearest animal drawn stands and mounts it
/// through its offer, held as an agent holds it: how a rider is looked at.
/// `winged`, it takes off on it, as F does, and tries the next nearest
/// until one has wings; the log says where that one stood.
pub(crate) fn ride(client: &mut Client, winged: bool) -> Result<(), String> {
    let mut input = client::Input::default();
    // The herds are drawn by a frame.
    client.update(1.0 / 60.0, &mut input);
    let here = glam::DVec3::from(client.up()) * client.generator().sphere().radius_m();
    let mut near: Vec<client::Near> = client
        .host("ride")
        .animals()
        .iter()
        .copied()
        .filter(|near| !near.ridden)
        .collect();
    near.sort_by(|a, b| (a.at - here).length().total_cmp(&(b.at - here).length()));
    let grid = client.generator().sphere().blocks();
    for animal in near.iter().take(if winged { TRIED } else { 1 }) {
        client.teleport(grid.surface_point(animal.at.to_array()));
        client.update(1.0 / 60.0, &mut input);
        client.drain_events();
        mount(client, animal.number, &mut input)?;
        if !winged {
            return Ok(());
        }
        // The animal is under its rider from the animals' next turn.
        for _ in 0..10 {
            client.update(1.0 / 60.0, &mut input);
        }
        client.drain_events();
        let stood = client.here().text(grid);
        client.command_json(r#"{"type":"set_mode","mode":"fly"}"#);
        client.update(1.0 / 60.0, &mut input);
        // Without wings, the key is refused and the body stays on.
        let off = client.drain_events().into_iter().any(|event| {
            matches!(
                event,
                client::Event::Rejected { .. }
                    | client::Event::ModeChanged {
                        mode: client::Mode::Fly | client::Mode::Walk
                    }
            )
        });
        if !off {
            log::info!("a pegasus, number {}, stood at {stood}", animal.number);
            return Ok(());
        }
        client.command_json(r#"{"type":"set_mode","mode":"walk"}"#);
        client.update(1.0 / 60.0, &mut input);
    }
    Err("--fly: no animal with wings near the shot".into())
}

/// Mounts the animal numbered `number` through its offer, held.
pub(crate) fn mount(
    client: &mut Client,
    number: u64,
    input: &mut client::Input,
) -> Result<(), String> {
    client.command_json(&format!(
        r#"{{"type":"act","plugin":"ride","id":{number},"hold":true}}"#
    ));
    // The hold's ring closes in its own time, a frame at a time.
    for _ in 0..120 {
        client.update(1.0 / 60.0, input);
        for event in client.drain_events() {
            match event {
                client::Event::ModeChanged {
                    mode: client::Mode::Way(_),
                } => return Ok(()),
                client::Event::Rejected { message } => return Err(format!("--ride: {message}")),
                _ => {}
            }
        }
    }
    Err("--ride: the animal nearest could not be mounted".into())
}

pub fn run(shot: Shot) -> Result<(), String> {
    // A cut through the ground needs the generator and nothing else: no GPU,
    // no avatar, no clock.
    if shot.slice_m > 0.0 {
        let generator = match shot.field.clone() {
            Some(field) => worldgen::Generator::with_field(shot.recipe.clone(), field),
            None => worldgen::Generator::new(shot.recipe.clone()),
        }
        .map_err(|e| e.to_string())?;
        return crate::slice::run(shot, &generator);
    }
    let [width, height] = shot.size;
    let mut client = match shot.field {
        Some(field) => Client::with_field(shot.recipe, field),
        None => Client::new(shot.recipe),
    }
    .map_err(|e| e.to_string())?;
    // A picture has no world to say what is on in it: it stands in for one,
    // so what a plugin builds can be looked at.
    for plugin in plugins_client::all() {
        client.plug(plugin);
    }
    client.rehearse(client::Level::Admin);
    client.set_view([width, height]);
    client.set_clear(shot.clear);
    client.set_clock(shot.clock_s);
    if let Some(effects) = &shot.effects {
        client.command_json(&format!(r#"{{"type":"set_effects","effects":{effects}}}"#));
    }
    if let Some(place) = &shot.at {
        client.go_to(place).map_err(|e| format!("--at {e}"))?;
    }
    let (pitch, boom_m) = (shot.pitch_deg.to_radians(), shot.boom_m);
    match shot.moon_gap_m {
        Some(gap_m) => client.visit_moon(gap_m, pitch, boom_m),
        None => client.pose(shot.altitude_m, pitch, boom_m),
    }
    assets::wear(&mut client, shot.avatar.as_deref());
    // Manifest first, then what it names, until nothing more is asked.
    while assets::serve(&mut client) {}
    for event in client.drain_events() {
        if let client::Event::Rejected { message } = event {
            log::warn!("{message}");
        }
    }

    // A fixed step keeps the stride, and so the picture, reproducible.
    let mut input = client::Input::default();
    let tick = |client: &mut Client, input: &mut client::Input| {
        client.update(1.0 / 60.0, input);
    };
    for step in &shot.steps {
        match step {
            Step::Command(json) => client.command_json(json),
            Step::Drag([x0, y0, x1, y1]) => {
                input.pointer = Some([*x0, *y0]);
                input.key(client::Key::Use, true);
                tick(&mut client, &mut input);
                input.pointer = Some([*x1, *y1]);
                tick(&mut client, &mut input);
                input.key(client::Key::Use, false);
                tick(&mut client, &mut input);
            }
        }
        // What a step asked for lands before the next, as a file read from
        // disk does in the window: a model taken in hand, then placed by a
        // click.
        while assets::serve(&mut client) {}
    }
    for event in client.drain_events() {
        match event {
            client::Event::Rejected { message } => log::warn!("{message}"),
            client::Event::Plugin(event) if event["type"] == "build.refused" => {
                log::warn!("no volume here: {}", event["reason"]);
            }
            _ => {}
        }
    }
    if shot.ride {
        ride(&mut client, shot.fly_s > 0.0)?;
    }
    input.key(client::Key::Forward, true);
    for _ in 0..((shot.walk_s + shot.fly_s) * 60.0) as u32 {
        client.update(1.0 / 60.0, &mut input);
    }
    client.set_clock(shot.clock_s);

    if let Some(dir) = &shot.animals {
        return crate::animals::run(&mut client, shot.size, &shot.view, dir);
    }
    if let Some(dir) = &shot.wings {
        return crate::wings::run(&mut client, shot.size, &shot.view, dir);
    }
    if let Some(dir) = &shot.saddle {
        return crate::saddle::run(&mut client, shot.size, dir);
    }
    if let Some(dir) = &shot.herd {
        return crate::herd::run(&mut client, shot.size, dir);
    }
    if let Some(dir) = &shot.takeoff {
        return crate::takeoff::run(&mut client, shot.size, dir);
    }
    let mut headless = Headless::new(width, height)?;
    if let Some(cabinet) = &shot.cabinet {
        crate::play::run(&mut client, &mut headless, cabinet)?;
        client.set_clock(shot.clock_s);
    }
    let mut frame = drawn_into(&mut client, &mut headless);
    if shot.fly_s > 0.0 && shot.view.against_sun {
        against_sun(&mut frame, shot.boom_m);
    } else if shot.view.yaw_deg != 0.0 {
        orbit(&mut frame, shot.boom_m, shot.view.yaw_deg);
    }
    if shot.measure > 0 {
        for (label, shadows, grass, clouds, bloom) in [
            ("base", false, false, false, false),
            ("shadows", true, false, false, false),
            ("shadows + grass", true, true, false, false),
            ("shadows + grass + clouds", true, true, true, false),
            ("shadows + grass + clouds + bloom", true, true, true, true),
        ] {
            let priced = client::Frame {
                effects: client::Effects {
                    shadows,
                    grass,
                    clouds,
                    bloom: if bloom {
                        frame.effects.bloom.max(0.5)
                    } else {
                        0.0
                    },
                    ..frame.effects
                },
                ..frame.clone()
            };
            let (median, p95) = headless.measure(&priced, shot.measure);
            println!(
                "{label}: median {median:.2} ms, p95 {p95:.2} ms (render + GPU wait, {width}x{height})"
            );
        }
    }
    let pixels = if shot.panel {
        headless.render_under(&frame, |over| {
            ui_native::Panel::headless(over.device, render::HEADLESS_FORMAT).picture(
                over.device,
                over.queue,
                over.encoder,
                over.target,
                over.size,
            );
        })
    } else {
        headless.render(&frame)
    };

    if let Some(parent) = shot.out.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    write_png(&shot.out, width, height, &pixels)?;
    let foliage = headless.renderer.foliage_counted();
    log::info!(
        "{} patches, foliage {:?} by form in {} draws of {} triangles -> {}",
        frame.patches.len(),
        foliage.forms,
        foliage.draws,
        foliage.triangles,
        shot.out.display()
    );
    Ok(())
}

/// How far from a body in flight the eye stands to look past it at the
/// sun, and how far under it, metres.
const SUN_AWAY_M: f64 = 9.0;
const SUN_UNDER_M: f64 = 1.5;

/// Turns the eye of `frame`, `boom_m` behind the body, to face the sun past
/// the body from its other side: the body between the eye and the sun.
fn against_sun(frame: &mut scene::Frame, boom_m: f64) {
    let camera = &mut frame.camera;
    let body = camera.position + camera.rotation * glam::DVec3::NEG_Z * boom_m;
    let up = body.normalize();
    let sun = frame.sun_direction.as_dvec3();
    let flat = (sun - up * sun.dot(up)).normalize_or(up.any_orthonormal_vector());
    let eye = body - flat * SUN_AWAY_M - up * SUN_UNDER_M;
    let back = (eye - body).normalize();
    let right = up.cross(back).normalize();
    camera.position = eye;
    camera.rotation =
        glam::DQuat::from_mat3(&glam::DMat3::from_cols(right, back.cross(right), back));
}

/// Swings the camera round the body it looks at by `yaw_deg`, about the
/// body's up, as far from it: 180 looks at it from the front.
fn orbit(frame: &mut scene::Frame, boom_m: f64, yaw_deg: f64) {
    let camera = &mut frame.camera;
    let body = camera.position + camera.rotation * glam::DVec3::NEG_Z * boom_m;
    let turn = glam::DQuat::from_axis_angle(body.normalize(), -yaw_deg.to_radians());
    camera.position = body + turn * (camera.position - body);
    camera.rotation = turn * camera.rotation;
}

/// A renderer of `size` holding what the client has to draw, and the frame
/// to draw it with.
pub(crate) fn drawn(
    client: &mut Client,
    [width, height]: [u32; 2],
) -> Result<(Headless, scene::Frame), String> {
    let mut headless = Headless::new(width, height)?;
    let frame = drawn_into(client, &mut headless);
    Ok((headless, frame))
}

/// The same, into a renderer that may already hold what was drawn before.
fn drawn_into(client: &mut Client, headless: &mut Headless) -> scene::Frame {
    let frame = client.settled_frame();
    let renderer = &mut headless.renderer;
    renderer.apply(client.drain_terrain_changes());
    renderer.apply_volumes(client.drain_volume_changes());
    renderer.apply_skinned(client.drain_skinned_changes());
    renderer.apply_pictures(client.drain_picture_changes());
    renderer.apply_models(client.drain_model_changes());
    frame
}
