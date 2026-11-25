//! Command line parsing. Small enough that a dependency would be heavier.

use std::path::PathBuf;

use worldgen::{Field, Recipe, Source, parse_seed};

pub enum Invocation {
    Window {
        recipe: Recipe,
        field: Option<Field>,
        avatar: Option<String>,
        /// Where to stand in sector 0, each `0..=1`. `None` is the spawn.
        at: Option<String>,
    },
    Shot(Box<Shot>),
    Cart(CartRun),
    /// Writes the layout a cabinet's skin is painted on to this PNG.
    CabinetTemplate(PathBuf),
}

/// A cartridge in a window through the emulator's native host, the keys its buttons.
pub struct CartRun {
    /// The cartridge's `.wasm`.
    pub file: PathBuf,
    pub seed: u32,
    /// Screen pixels a window's pixel is wide.
    pub scale: u32,
}

pub struct Shot {
    pub recipe: Recipe,
    /// The ground the recipe names, already read. `None` for a seed world.
    pub field: Option<Field>,
    /// File stem under `assets/avatars`.
    pub avatar: Option<String>,
    pub out: PathBuf,
    pub size: [u32; 2],
    /// Seconds on the world clock: fixes the sun.
    pub clock_s: f64,
    pub altitude_m: f64,
    pub pitch_deg: f64,
    pub boom_m: f64,
    /// Seconds of walking forward before the shot, to catch a gait mid stride.
    pub walk_s: f64,
    /// Mount the nearest animal before walking: the walk is its first pace.
    pub ride: bool,
    /// Seconds of flying forward on the nearest animal with wings, taken
    /// off, in place of the walk; with `--against-sun` the eye faces the
    /// sun past it.
    pub fly_s: f64,
    /// Where to stand in sector 0, each `0..=1`. Default: the world's spawn.
    pub at: Option<String>,
    /// Fly to this many metres under the moon instead.
    pub moon_gap_m: Option<f64>,
    /// Fixed-scene render timing, with effects toggled for comparison.
    pub measure: usize,
    /// Paint the native settings panel, open, over the picture.
    pub panel: bool,
    /// Effects as the seam takes them: `{"tone_map":"agx","bloom":1}`.
    pub effects: Option<String>,
    /// Paint a vertical cut through the ground instead of rendering it, this
    /// many metres wide. Zero renders the world as usual.
    pub slice_m: f64,
    /// What a hand does before the shot, in the order given.
    pub steps: Vec<Step>,
    /// Draw an animal's strides into this folder instead of the shot, seen
    /// as `view` says.
    pub animals: Option<PathBuf>,
    /// Where the sheets of a horse's wings go: `crate::wings`.
    pub wings: Option<PathBuf>,
    /// Where the sheets of a rider in the saddle go: `crate::saddle`.
    pub saddle: Option<PathBuf>,
    /// Where the sheet of a herd built over goes: `crate::herd`.
    pub herd: Option<PathBuf>,
    /// Where the frames of a pegasus taking off go: `crate::takeoff`.
    pub takeoff: Option<PathBuf>,
    pub view: crate::animals::View,
    /// A cabinet placed before the body, its game running, played or not.
    pub cabinet: Option<crate::play::Cabinet>,
    /// The part of the view the shell leaves clear of what it lays over it,
    /// pixels from its top left: what a page with keys over a phone says.
    pub clear: Option<[f64; 4]>,
}

/// One thing a hand does before a shot is taken.
pub enum Step {
    /// A command as the seam takes it: `{"type":"build.take","tool":"create"}`.
    Command(String),
    /// Press the pointer's button at one point of the view and let go at
    /// another, fractions from the top left: a stroke of the tool in hand.
    Drag([f32; 4]),
}

/// The seed every preview starts from unless told otherwise.
const DEFAULT_SEED: u64 = 1;

pub fn parse(args: impl Iterator<Item = String>) -> Result<Invocation, String> {
    let mut args = args.peekable();
    if args.next_if(|arg| arg == "cart").is_some() {
        return cart(args).map(Invocation::Cart);
    }
    if args.next_if(|arg| arg == "cabinet-template").is_some() {
        let out = args
            .next()
            .ok_or("cabinet-template needs the PNG to write")?;
        return Ok(Invocation::CabinetTemplate(PathBuf::from(out)));
    }
    let is_shot = args.next_if(|arg| arg == "shot").is_some();

    let mut recipe = Recipe::new(DEFAULT_SEED);
    let mut field = None;
    let mut shot = Shot {
        recipe: recipe.clone(),
        field: None,
        avatar: None,
        out: PathBuf::new(),
        size: [1280, 720],
        clock_s: 0.0,
        altitude_m: 0.0,
        pitch_deg: -14.0,
        boom_m: 6.0,
        walk_s: 0.0,
        ride: false,
        fly_s: 0.0,
        at: None,
        moon_gap_m: None,
        measure: 0,
        panel: false,
        effects: None,
        slice_m: 0.0,
        steps: Vec::new(),
        animals: None,
        wings: None,
        saddle: None,
        herd: None,
        takeoff: None,
        view: crate::animals::View::default(),
        cabinet: None,
        clear: None,
    };
    let (mut played, mut left, mut paired) = (false, false, false);
    let mut pressed = Vec::new();
    let mut ahead_m = 1.0;

    while let Some(flag) = args.next() {
        let mut value = || args.next().ok_or(format!("{flag} needs a value"));
        match flag.as_str() {
            "--seed" => recipe.seed = parse_seed(&value()?).map_err(|e| e.to_string())?,
            // The body's size, `4..=16`. A small world fits whole inside the
            // near field, which is the only place there is ground yet.
            "--bits" => recipe.sector_bits = number(&value()?, "--bits")?,
            // The file names the ground, and the recipe follows it: a field
            // world is only ever the world that field was baked for. A file
            // on this disk is trusted to be the ground its id says.
            "--field" => {
                let path = value()?;
                let bytes = std::fs::read(&path).map_err(|e| format!("{path}: {e}"))?;
                let read = Field::parse(&bytes).map_err(|e| format!("{path}: {e}"))?;
                recipe.params.source = Source::Field(read.id());
                field = Some(read);
            }
            "--avatar" => shot.avatar = Some(value()?),
            "--out" if is_shot => shot.out = PathBuf::from(value()?),
            "--size" if is_shot => {
                let text = value()?;
                let (w, h) = text.split_once('x').ok_or("--size wants WxH")?;
                shot.size = [number(w, "--size")?, number(h, "--size")?];
            }
            "--measure" if is_shot => {
                shot.measure = number::<usize>(&value()?, "--measure")?.min(1000)
            }
            "--panel" if is_shot => shot.panel = true,
            "--slice" if is_shot => shot.slice_m = number(&value()?, "--slice")?,
            "--effects" if is_shot => shot.effects = Some(value()?),
            "--command" if is_shot => shot.steps.push(Step::Command(value()?)),
            "--drag" if is_shot => {
                let text = value()?;
                let parts: Vec<f32> = text
                    .split(',')
                    .map(|part| number(part, "--drag"))
                    .collect::<Result<_, _>>()?;
                let points: [f32; 4] = parts
                    .try_into()
                    .map_err(|_| "--drag wants X0,Y0,X1,Y1".to_owned())?;
                shot.steps.push(Step::Drag(points));
            }
            "--clock" if is_shot => shot.clock_s = number(&value()?, "--clock")?,
            "--altitude" if is_shot => shot.altitude_m = number(&value()?, "--altitude")?,
            "--pitch" if is_shot => shot.pitch_deg = number(&value()?, "--pitch")?,
            // The one place flag both invocations take: a picture and a walk
            // are worth nothing to each other if they cannot be aimed alike.
            // A place code, as the HUD and the address bar show it. Two
            // fractions of sector 0 used to go here, which could not name the
            // other five and which nobody could read back off a screenshot.
            "--at" => shot.at = Some(value()?),
            "--moon" if is_shot => shot.moon_gap_m = Some(number(&value()?, "--moon")?),
            "--walk" if is_shot => shot.walk_s = number(&value()?, "--walk")?,
            "--ride" if is_shot => shot.ride = true,
            "--fly" if is_shot => {
                shot.ride = true;
                shot.fly_s = number(&value()?, "--fly")?;
            }
            "--boom" if is_shot => shot.boom_m = number(&value()?, "--boom")?,
            "--animals" if is_shot => shot.animals = Some(PathBuf::from(value()?)),
            "--wings" if is_shot => shot.wings = Some(PathBuf::from(value()?)),
            "--saddle" if is_shot => shot.saddle = Some(PathBuf::from(value()?)),
            "--herd" if is_shot => shot.herd = Some(PathBuf::from(value()?)),
            "--takeoff" if is_shot => shot.takeoff = Some(PathBuf::from(value()?)),
            "--yaw" if is_shot => shot.view.yaw_deg = number(&value()?, "--yaw")?,
            "--against-sun" if is_shot => shot.view.against_sun = true,
            "--cabinet" if is_shot => {
                let text = value()?;
                let cart = client::ContentId::parse(&text)
                    .ok_or(format!("--cabinet wants a cartridge's sha256, not {text}"))?;
                shot.cabinet = Some(crate::play::Cabinet {
                    cart,
                    ahead_m: 0.0,
                    yaw_deg: 0.0,
                    played: false,
                    left: false,
                    paired: false,
                    pressed: Vec::new(),
                });
            }
            "--ahead" if is_shot => ahead_m = number(&value()?, "--ahead")?,
            "--play" if is_shot => played = true,
            "--leave" if is_shot => left = true,
            "--pair" if is_shot => paired = true,
            "--press" if is_shot => pressed = value()?.split(',').map(str::to_owned).collect(),
            "--clear" if is_shot => {
                let parts: Vec<f64> = value()?
                    .split(',')
                    .map(|part| number(part, "--clear"))
                    .collect::<Result<_, _>>()?;
                let part: [f64; 4] = parts
                    .try_into()
                    .map_err(|_| "--clear wants LEFT,TOP,RIGHT,BOTTOM".to_owned())?;
                shot.clear = Some(part);
            }
            "--look" if is_shot => shot.view.seed = number(&value()?, "--look")?,
            other => return Err(format!("unknown argument {other}")),
        }
    }

    if !is_shot {
        return Ok(Invocation::Window {
            recipe,
            field,
            avatar: shot.avatar,
            at: shot.at,
        });
    }
    if !played && (left || !pressed.is_empty()) {
        return Err("--leave and --press need --play".into());
    }
    match &mut shot.cabinet {
        Some(cabinet) => {
            cabinet.ahead_m = ahead_m;
            cabinet.yaw_deg = if played { 0.0 } else { shot.view.yaw_deg };
            cabinet.played = played;
            cabinet.left = left;
            cabinet.paired = paired;
            cabinet.pressed = pressed;
        }
        None if played || paired => return Err("--play and --pair need --cabinet".into()),
        None => {}
    }
    if shot.out.as_os_str().is_empty() {
        return Err("shot needs --out FILE".into());
    }
    shot.recipe = recipe;
    shot.field = field;
    Ok(Invocation::Shot(Box::new(shot)))
}

fn cart(mut args: impl Iterator<Item = String>) -> Result<CartRun, String> {
    let file = args
        .next()
        .filter(|file| !file.starts_with("--"))
        .ok_or("cart needs the cartridge's .wasm first")?;
    let mut run = CartRun {
        file: PathBuf::from(file),
        seed: 1,
        scale: 4,
    };
    while let Some(flag) = args.next() {
        let mut value = || args.next().ok_or(format!("{flag} needs a value"));
        match flag.as_str() {
            "--seed" => run.seed = number(&value()?, "--seed")?,
            "--scale" => run.scale = number::<u32>(&value()?, "--scale")?.clamp(1, 16),
            "--buttons" | "--frames" | "--out" | "--every" | "--at" | "--wav" => {
                return Err(format!(
                    "{flag} is a run with no window: the console's runner runs it (`bun run cart` in TKN-32)"
                ));
            }
            other => return Err(format!("unknown argument {other}")),
        }
    }
    Ok(run)
}

fn number<T: std::str::FromStr>(text: &str, flag: &str) -> Result<T, String> {
    text.parse()
        .map_err(|_| format!("{flag}: cannot read {text:?} as a number"))
}
