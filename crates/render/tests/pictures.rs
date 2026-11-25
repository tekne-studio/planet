//! A picture is drawn: a quad with its texture, lit, shining, and the card of
//! one not yet landed. Headless, to a PNG that is looked at, as every change
//! is. Skipped where no GPU answers.

use std::sync::Arc;

use glam::{DAffine3, DQuat, DVec3, Vec3};
use render::{Headless, PictureFrame, write_png};
use scene::{
    Camera, Effects, Frame, Image, InteractionCapsule, Moon, PictureChange, PictureDraw, PictureId,
};

/// A checkerboard with a red top left corner and a see-through band across
/// the middle, so the texture's way up and its cutout are both seen.
fn checker(width: u32, height: u32) -> Image {
    let mut rgba = Vec::with_capacity((width * height * 4) as usize);
    for y in 0..height {
        for x in 0..width {
            let dark = ((x / 16) + (y / 16)) % 2 == 0;
            let mut texel = if dark {
                [40, 60, 200, 255]
            } else {
                [240, 240, 220, 255]
            };
            if x < 32 && y < 32 {
                texel = [220, 30, 30, 255];
            }
            if (height / 2 - 4..height / 2 + 4).contains(&y) {
                texel[3] = 0;
            }
            rgba.extend(texel);
        }
    }
    Image {
        width,
        height,
        rgba,
    }
}

/// A frame from an eye standing on the north pole of a small planet,
/// looking along `-z`, with nothing drawn and no clouds.
fn standing(eye: DVec3, radius_m: f64) -> Frame {
    Frame {
        camera: Camera {
            position: eye,
            rotation: DQuat::IDENTITY,
            fov_y: 60f32.to_radians(),
            near: 0.1,
        },
        sun_direction: Vec3::new(0.3, 0.8, 0.5).normalize(),
        moon: Moon {
            position: DVec3::new(0.0, 0.0, -50_000.0),
            radius_m: 100.0,
        },
        planet_radius_m: radius_m,
        clock_s: 0.0,
        patches: Vec::new(),
        shadow_patches: Vec::new(),
        interaction: InteractionCapsule {
            start: eye,
            end: eye,
            radius_m: 0.0,
        },
        effects: Effects {
            clouds: false,
            ..Effects::default()
        },
        boxes: Vec::new(),
        skinned: Vec::new(),
        volumes: Vec::new(),
        glass: Vec::new(),
        lamps: Vec::new(),
        models: Vec::new(),
        ghost: None,
        guides: Vec::new(),
        pictures: Vec::new(),
        panes: Vec::new(),
        through: None,
        warp: None,
        overlay: None,
    }
}

#[test]
fn pictures_are_drawn() {
    let (width, height) = (640, 360);
    let Ok(mut headless) = Headless::new(width, height) else {
        eprintln!("no GPU: the picture test is skipped");
        return;
    };
    let id = PictureId(1);
    headless
        .renderer
        .apply_pictures(vec![PictureChange::Add(id, Arc::new(checker(256, 128)))]);
    // A picture that moves: a texture the shell writes a frame at a time. The
    // first frame is solid green; a frame of another size is dropped.
    let live = PictureId(2);
    headless.renderer.apply_pictures(vec![PictureChange::Live {
        id: live,
        width: 64,
        height: 32,
        sharp: false,
    }]);
    let green = Image {
        width: 64,
        height: 32,
        rgba: [20, 200, 40, 255].repeat(64 * 32),
    };
    headless
        .renderer
        .write_picture(live, PictureFrame::Pixels(&green));
    headless
        .renderer
        .write_picture(live, PictureFrame::Pixels(&checker(256, 128)));

    // A small planet, the eye standing on it at its north pole, looking along -Z.
    let radius_m = 1000.0;
    let eye = DVec3::new(0.0, radius_m + 1.6, 0.0);
    let at = |x: f64, z: f64| DAffine3::from_translation(eye + DVec3::new(x, 0.0, z));
    let frame = Frame {
        pictures: vec![
            // Lit, two to one, facing the eye.
            PictureDraw {
                picture: Some(id),
                transform: at(0.0, -3.0) * DAffine3::from_scale(DVec3::new(2.0, 1.0, 1.0)),
                shine: 0.0,
                both_sides: false,
            },
            // A screen, turned away and seen from behind: drawn, left still left.
            PictureDraw {
                picture: Some(id),
                transform: at(-2.6, -3.0)
                    * DAffine3::from_quat(DQuat::from_rotation_y(core::f64::consts::PI))
                    * DAffine3::from_scale(DVec3::new(2.0, 1.0, 1.0)),
                shine: 1.0,
                both_sides: true,
            },
            // A poster seen from behind: not drawn.
            PictureDraw {
                picture: Some(id),
                transform: at(-2.6, -1.5)
                    * DAffine3::from_quat(DQuat::from_rotation_y(core::f64::consts::PI))
                    * DAffine3::from_scale(DVec3::new(0.6, 0.3, 1.0)),
                shine: 0.0,
                both_sides: false,
            },
            // The card of a picture not yet landed.
            PictureDraw {
                picture: None,
                transform: at(2.4, -3.0) * DAffine3::from_scale(DVec3::new(1.5, 1.0, 1.0)),
                shine: 0.0,
                both_sides: false,
            },
            // A picture that moves, showing the frame last written, as a screen.
            PictureDraw {
                picture: Some(live),
                transform: at(1.0, -1.5) * DAffine3::from_scale(DVec3::new(1.0, 0.5, 1.0)),
                shine: 1.0,
                both_sides: false,
            },
        ],
        ..standing(eye, radius_m)
    };
    let pixels = headless.render(&frame);
    // Beside the other test output, to be looked at: `target/tmp/pictures.png`.
    let out = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("pictures.png");
    write_png(&out, width, height, &pixels).unwrap();

    let texel = |x: u32, y: u32| {
        let i = ((y * width + x) * 4) as usize;
        [pixels[i], pixels[i + 1], pixels[i + 2]]
    };
    // The quad in the middle is a metre tall three metres away under a 60
    // degree view: 104 pixels tall, 208 wide, about the centre.
    let centre = texel(320, 180);
    let sky = texel(320, 20);
    assert_ne!(centre, sky, "the picture covers the centre");
    let corner = texel(320 - 100, 180 - 48);
    assert!(
        corner[0] > corner[2] + 60,
        "the red corner is the top left of the quad: {corner:?}"
    );
    let band = texel(320 + 40, 180);
    let above = texel(320 + 40, 180 - 20);
    assert_ne!(band, above, "the see-through band shows what is behind");
    // The moving picture, a metre and a half away, a metre to the right: its
    // centre is 208 px right of the middle, and it is the green frame.
    let screen = texel(320 + 208, 180);
    assert!(
        screen[1] > screen[0] + 80 && screen[1] > screen[2] + 80,
        "the frame written last of the right size shows: {screen:?}"
    );
}

/// A portal's pane: dark while nothing is seen through it, and a window on
/// the world past the portal when the frame carries one: here the sun, from
/// an eye turned to face it, which the view itself has at its back.
#[test]
fn a_pane_is_a_window_on_the_world_past_the_portal() {
    let (width, height) = (640, 360);
    let Ok(mut headless) = Headless::new(width, height) else {
        eprintln!("no GPU: the pane test is skipped");
        return;
    };
    let radius_m = 1000.0;
    let eye = DVec3::new(0.0, radius_m + 1.6, 0.0);
    let camera = Camera {
        position: eye,
        rotation: DQuat::IDENTITY,
        fov_y: 60f32.to_radians(),
        near: 0.1,
    };
    // A pane two metres across, three metres ahead, facing the eye: the quad
    // faces `+z`, toward the eye looking down `-z`.
    let pane = DAffine3::from_translation(eye + DVec3::new(0.0, 0.0, -3.0))
        * DAffine3::from_scale(DVec3::new(2.0, 2.0, 1.0));
    let mut frame = Frame {
        camera,
        sun_direction: Vec3::new(0.0, 0.3, 1.0).normalize(),
        effects: Effects {
            clouds: false,
            bloom: 0.0,
            ..Effects::default()
        },
        panes: vec![scene::PaneDraw {
            transform: pane,
            seen: 0.0,
        }],
        ..standing(eye, radius_m)
    };
    let luma = |pixels: &[u8], x: u32, y: u32| {
        let i = ((y * width + x) * 4) as usize;
        u32::from(pixels[i]) + u32::from(pixels[i + 1]) + u32::from(pixels[i + 2])
    };
    let dark = headless.render(&frame);
    let out = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("pane_dark.png");
    write_png(&out, width, height, &dark).unwrap();
    assert!(
        luma(&dark, 320, 180) < 60,
        "a pane with nothing through it is dark"
    );

    // Seen through: the lens past the portal faces the sun, and sees at the
    // pane's rim as far off its line as the rim stands from the eye's, so
    // the pane shows the sun where a window would.
    frame.panes[0].seen = 1.0;
    frame.through = Some(scene::Through {
        camera: Camera {
            rotation: DQuat::from_rotation_y(core::f64::consts::PI),
            ..camera
        },
        turn: DQuat::from_rotation_y(core::f64::consts::PI),
        lens: (1.0f32 / 3.0).atan(),
        patches: Vec::new(),
        size: 0.25,
    });
    let seen = headless.render(&frame);
    let out = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("pane_seen.png");
    write_png(&out, width, height, &seen).unwrap();
    // The sun stands 17 degrees over the eye's horizon: a third of the way
    // up the view, inside the disc, where the window shows it.
    assert!(
        luma(&seen, 320, 90) > luma(&dark, 320, 90) + 100,
        "the sun past the portal shows in the pane: {} over {}",
        luma(&seen, 320, 90),
        luma(&dark, 320, 90)
    );
    assert_eq!(
        luma(&seen, 320 - 95, 180 - 95),
        luma(&dark, 320 - 95, 180 - 95),
        "beside the disc nothing changed"
    );

    // A wide lens sees 65 degrees off its line at the rim, the pane 18 off
    // the eye's: the sun, 17 over the horizon, comes to a quarter of the
    // way from the middle to the rim, some 27 pixels over the middle.
    frame.through = frame.through.map(|through| scene::Through {
        camera: Camera {
            fov_y: 140f32.to_radians(),
            ..through.camera
        },
        lens: 65f32.to_radians(),
        ..through
    });
    let wide = headless.render(&frame);
    let out = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("pane_wide.png");
    write_png(&out, width, height, &wide).unwrap();
    assert!(
        luma(&wide, 320, 153) > luma(&wide, 320, 90) + 100,
        "the sun comes nearer the middle: {} over {}",
        luma(&wide, 320, 153),
        luma(&wide, 320, 90)
    );

    // A sheet of red glass six metres across behind the pane: the disc
    // hides it, beside the disc it shows.
    let corner = |x: f32, y: f32| scene::VolumeVertex {
        position: [x, y, 0.0],
        normal: [0.0, 0.0, 1.0],
        color: [255, 0, 0, 0],
        side: [255, 0, 0, 0],
    };
    headless.renderer.apply_models(vec![scene::ModelChange::Add(
        scene::ModelId(1),
        scene::ModelMesh {
            vertices: vec![
                corner(-3.0, -3.0),
                corner(3.0, -3.0),
                corner(3.0, 3.0),
                corner(-3.0, 3.0),
            ],
            indices: vec![0, 1, 2, 0, 2, 3],
        },
    )]);
    frame.models = vec![scene::ModelDraw {
        mesh: scene::ModelId(1),
        transform: DAffine3::from_translation(eye + DVec3::new(0.0, 0.0, -6.0)),
        pass: scene::ModelPass::Glass,
        picture: None,
    }];
    let behind = headless.render(&frame);
    let out = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("pane_glass_behind.png");
    write_png(&out, width, height, &behind).unwrap();
    let rgb = |pixels: &[u8], x: u32, y: u32| {
        let i = ((y * width + x) * 4) as usize;
        [pixels[i], pixels[i + 1], pixels[i + 2]]
    };
    assert_eq!(
        rgb(&behind, 320, 200),
        rgb(&wide, 320, 200),
        "the glass behind the pane stays behind it"
    );
    assert_ne!(
        rgb(&behind, 320 - 130, 180),
        rgb(&wide, 320 - 130, 180),
        "beside the disc the glass shows"
    );
}

/// A run of a model that shows its draw's picture: one mesh, three draws,
/// each in what its draw names, white for none and for one not held.
#[test]
fn a_models_run_shows_the_picture_its_draw_names() {
    let (width, height) = (640, 360);
    let Ok(mut headless) = Headless::new(width, height) else {
        eprintln!("no GPU: the drawn run test is skipped");
        return;
    };
    let red = PictureId(1);
    let solid = |rgb: [u8; 3]| Image {
        width: 4,
        height: 4,
        rgba: [rgb[0], rgb[1], rgb[2], 255].repeat(16),
    };
    headless.renderer.apply_pictures(vec![PictureChange::Add(
        red,
        Arc::new(solid([220, 30, 30])),
    )]);
    let corner = |x: f32, y: f32| scene::TexturedVertex {
        position: [x, y, 0.0],
        normal: [0.0, 0.0, 1.0],
        uv: [x + 0.5, 0.5 - y],
        color: [255; 4],
    };
    let quad = scene::ModelId(1);
    headless
        .renderer
        .apply_models(vec![scene::ModelChange::Textured(
            quad,
            scene::TexturedMesh {
                vertices: vec![
                    corner(-0.5, -0.5),
                    corner(0.5, -0.5),
                    corner(0.5, 0.5),
                    corner(-0.5, 0.5),
                ],
                indices: vec![0, 1, 2, 0, 2, 3],
                runs: vec![scene::TexturedRun {
                    indices: 0..6,
                    image: scene::RunImage::Drawn,
                    cutoff: 0.0,
                    shine: 1.0,
                }],
                images: Vec::new(),
            },
        )]);
    let radius_m = 1000.0;
    let eye = DVec3::new(0.0, radius_m + 1.6, 0.0);
    let draw = |x: f64, picture| scene::ModelDraw {
        mesh: quad,
        transform: DAffine3::from_translation(eye + DVec3::new(x, 0.0, -3.0)),
        pass: scene::ModelPass::Solid,
        picture,
    };
    let frame = Frame {
        models: vec![
            draw(-1.2, Some(red)),
            draw(0.0, None),
            draw(1.2, Some(PictureId(9))),
        ],
        ..standing(eye, radius_m)
    };
    let pixels = headless.render(&frame);
    let out = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("drawn_run.png");
    write_png(&out, width, height, &pixels).unwrap();
    let texel = |x: u32, y: u32| {
        let i = ((y * width + x) * 4) as usize;
        [pixels[i], pixels[i + 1], pixels[i + 2]]
    };
    // A metre three metres away under 60 degrees is 104 px: the draws stand
    // 125 px apart about the middle.
    let [shown, plain, unheld] = [320 - 125, 320, 320 + 125].map(|x| texel(x, 180));
    assert!(
        shown[0] > shown[1] + 80 && shown[0] > shown[2] + 80,
        "the picture named shows: {shown:?}"
    );
    for white in [plain, unheld] {
        assert!(
            white.iter().all(|&c| c > 150) && white[0].abs_diff(white[2]) < 40,
            "with none held, white: {white:?}"
        );
    }
}

/// A picture laid over the view is the picture's own bytes, three pixels a
/// texel, whatever its sampler: a game's live texture, sharp, and a still
/// picture with its mips, side by side. Around them the view is as drawn.
#[test]
fn a_picture_laid_over_the_view_is_its_own_bytes() {
    let (width, height) = (640, 360);
    let Ok(mut headless) = Headless::new(width, height) else {
        eprintln!("no GPU: the overlay test is skipped");
        return;
    };
    // Sixteen colours scattered so that no two texels side by side agree.
    let palette: [[u8; 4]; 16] = core::array::from_fn(|i| {
        let i = i as u32;
        [i * 16 + 7, 255 - i * 13, (i * 89) % 251, 255].map(|c| c as u8)
    });
    let (texels_wide, texels_tall) = (32u32, 16u32);
    let colour = |x: u32, y: u32| palette[((x * 7 + y * 3) % 16) as usize];
    let image = Image {
        width: texels_wide,
        height: texels_tall,
        rgba: (0..texels_tall)
            .flat_map(|y| (0..texels_wide).flat_map(move |x| colour(x, y)))
            .collect(),
    };
    let (game, still) = (PictureId(7), PictureId(8));
    headless.renderer.apply_pictures(vec![
        PictureChange::Live {
            id: game,
            width: texels_wide,
            height: texels_tall,
            sharp: true,
        },
        PictureChange::Add(still, Arc::new(image.clone())),
    ]);
    headless
        .renderer
        .write_picture(game, PictureFrame::Pixels(&image));

    let radius_m = 1000.0;
    let eye = DVec3::new(0.0, radius_m + 1.6, 0.0);
    let bare = standing(eye, radius_m);
    let under = headless.render(&bare);
    const SCALE: u32 = 3;
    for (picture, left) in [(game, 40), (still, 400)] {
        let rect = [left, 100, texels_wide * SCALE, texels_tall * SCALE];
        let frame = Frame {
            overlay: Some(scene::Overlay {
                picture,
                rect,
                opacity: 1.0,
            }),
            ..standing(eye, radius_m)
        };
        let pixels = headless.render(&frame);
        let out = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("overlay_{}.png", picture.0));
        write_png(&out, width, height, &pixels).unwrap();
        let at = |pixels: &[u8], x: u32, y: u32| {
            let i = ((y * width + x) * 4) as usize;
            [pixels[i], pixels[i + 1], pixels[i + 2]]
        };
        for y in 0..rect[3] {
            for x in 0..rect[2] {
                let [r, g, b, _] = colour(x / SCALE, y / SCALE);
                assert_eq!(
                    at(&pixels, rect[0] + x, rect[1] + y),
                    [r, g, b],
                    "picture {picture:?}, pixel {x},{y} of the rectangle"
                );
            }
        }
        // Around it, the view as drawn without it.
        for (x, y) in [
            (rect[0] - 1, rect[1]),
            (rect[0] + rect[2], rect[1]),
            (rect[0], rect[1] - 1),
            (rect[0], rect[1] + rect[3]),
        ] {
            assert_eq!(
                at(&pixels, x, y),
                at(&under, x, y),
                "{picture:?} at {x},{y}"
            );
        }
    }
}
