//! The desktop shell: the offline explorer in a window, a headless shot, a
//! cartridge in a window, or the layout a cabinet's skin is painted on. A
//! cartridge run with no window is the arcade's.
//!
//! ```text
//! planet [--seed HEX] [--bits N] [--avatar NAME] [--at U,V]
//! planet shot --out FILE [--seed HEX] [--bits N] [--avatar NAME] [--size WxH] [--clock S]
//!             [--day MIN] [--altitude M] [--pitch DEG] [--boom M] [--walk S] [--at U,V] [--moon M]
//!             [--command JSON]... [--drag X0,Y0,X1,Y1]... [--ride] [--fly S [--against-sun]]
//!             [--yaw DEG] [--animals DIR | --wings DIR [--against-sun] [--look SEED]
//!             | --saddle DIR | --herd DIR | --takeoff DIR | --leap DIR | --perch DIR | --deck DIR
//!             | --clouds DIR | --shadows DIR | --flora DIR]
//! planet cart FILE.wasm [--seed N] [--scale N]
//! planet cabinet-template FILE.png
//! ```

mod animals;
mod args;
mod assets;
mod cart;
mod carts;
mod clouds;
mod deck;
mod flora;
mod herd;
mod leap;
mod moving;
mod perch;
mod play;
mod saddle;
mod shadows;
mod shot;
mod slice;
mod takeoff;
mod window;
mod wings;

use std::path::Path;
use std::process::ExitCode;

fn main() -> ExitCode {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn,planet=info"))
        .init();
    let result = match args::parse(std::env::args().skip(1)) {
        Ok(args::Invocation::Window {
            recipe,
            field,
            avatar,
            at,
        }) => window::run(recipe, field, avatar, at),
        Ok(args::Invocation::Shot(shot)) => shot::run(*shot),
        Ok(args::Invocation::Cart(run)) => cart::run(run),
        Ok(args::Invocation::CabinetTemplate(out)) => cabinet_template(&out),
        Err(message) => Err(message),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("planet: {message}");
            ExitCode::FAILURE
        }
    }
}

/// Writes the layout a cabinet's skin is painted on, for a person to paint
/// over and drop on a cabinet: each place named by its name, in capitals,
/// in the cartridges' font.
fn cabinet_template(out: &Path) -> Result<(), String> {
    let client::cabinet::Template { image, labels } = client::cabinet::template();
    let mut picture = image::RgbaImage::from_raw(image.width, image.height, image.rgba)
        .ok_or("the template is no picture")?;
    let mut emulator = arcade_host::Emulator::hosted(arcade::RUNNER).map_err(|e| e.to_string())?;
    for label in labels {
        let line = emulator.text(label.name).map_err(|e| e.to_string())?;
        write(&mut picture, &label, &line);
    }
    picture
        .save(out)
        .map_err(|e| format!("{}: {e}", out.display()))?;
    println!("cabinet skin template -> {}", out.display());
    Ok(())
}

/// A label's name, written in `line`, in the middle of its rectangle:
/// twice the font's size where it fits across it, else at its size.
fn write(picture: &mut image::RgbaImage, label: &client::cabinet::Label, line: &arcade_host::Text) {
    let [x0, y0, x1, y1] = label.rect;
    let scale = if (line.width * 2) as f32 <= x1 - x0 - 4.0 {
        2
    } else {
        1
    };
    let left = ((x0 + x1 - (line.width * scale) as f32) / 2.0) as i64;
    let top = ((y0 + y1 - (line.height * scale) as f32) / 2.0) as i64;
    let [r, g, b] = label.ink;
    let inked = (0..line.height)
        .flat_map(|row| (0..line.width).map(move |col| (col, row)))
        .filter(|&(col, row)| line.ink[(row * line.width + col) as usize] == 1);
    for (col, row) in inked {
        for (dx, dy) in (0..scale).flat_map(|dx| (0..scale).map(move |dy| (dx, dy))) {
            let x = left + i64::from(col * scale + dx);
            let y = top + i64::from(row * scale + dy);
            if let (Ok(x), Ok(y)) = (u32::try_from(x), u32::try_from(y))
                && x < picture.width()
                && y < picture.height()
            {
                picture.put_pixel(x, y, image::Rgba([r, g, b, u8::MAX]));
            }
        }
    }
}
