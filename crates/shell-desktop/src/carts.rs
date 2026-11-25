//! The desktop side of the cabinets' games: the engine says which cartridge
//! to run into which texture (`client::Running`), and one thread of their
//! own runs them all through the emulator's native host, its shelf paced by
//! the emulator's clock, so a game never takes the frame's thread; a game
//! held, its player gone, holds its clock there, and runs on from it. Each
//! file is prepared once and its prepared bytes kept on disk ([`kept`]). Each screen
//! that changed comes back over a channel, and the newest of each game is
//! written into the renderer once a frame, and a game that stopped is said
//! to the engine. The desktop plays no sound: what the games play is let go
//! on their thread, where a device's callback would take it. One whose
//! file the engine gave up runs as a stopped cart that says so, and a cover
//! asked for shows as a cart that never runs. The thread starts the first
//! time a game is asked for and ends with the shell; one that dies is started
//! again once, and one that dies again is given up, said so, and every game
//! after fails. A runner that broke ends the thread as a death does.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{
    Receiver, RecvTimeoutError, Sender, SyncSender, TryRecvError, channel, sync_channel,
};
use std::time::{Duration, Instant};

use arcade::{HEIGHT, TICKS, WIDTH};
use arcade_host::{Broken, Emulator, Kept, Screen};
use client::{Client, LIVE_AT_ONCE, Running};
use render::{PictureFrame, Renderer};
use scene::{Image, PictureId};

/// How many times the thread is started: the first, and once more after it
/// died.
const STARTS: u32 = 2;
/// The most bytes of prepared files kept on disk: a few hundred games.
const KEPT_MOST: u64 = 64 << 20;

/// What the shell asks the thread, each game by a number of the shell's.
enum Asked {
    Open {
        number: u32,
        file: PathBuf,
        seed: u32,
    },
    /// A cartridge's cover, still, never run.
    Cover {
        number: u32,
        file: PathBuf,
        name: String,
    },
    /// A game whose file never came, its screen saying so.
    Fail(u32),
    Close(u32),
    Press(u32, u32),
    /// Runs its clock on, or holds it where it is.
    Runs(u32, bool),
    /// Its screen and palette, as an agent reads them.
    Read(u32),
}

/// What the thread answers.
enum Told {
    Opened(u32),
    Failed(u32, String),
    Screen(u32, Image),
    Stopped(u32),
    Read(u32, Screen),
}

/// A game the thread runs, as the shell knows it.
struct Game {
    picture: PictureId,
    /// Whether the engine heard it opened.
    opened: bool,
    /// Whether its texture is made: the engine heard it opened a frame ago,
    /// and the frame between made it.
    made: bool,
}

/// The thread's ends.
struct Thread {
    asks: Sender<Asked>,
    told: Receiver<Told>,
}

/// The games running, and the thread that runs them.
#[derive(Default)]
pub struct Carts {
    thread: Option<Thread>,
    /// How many threads were started, and why they were given up, once the
    /// last of them died.
    started: u32,
    given_up: Option<String>,
    games: HashMap<u32, Game>,
    numbers: HashMap<PictureId, u32>,
    /// The newest screen of each game not yet written: a screen comes as
    /// often as its game ticks, and one that comes before its texture is
    /// made waits for it, a stop screen being the last a game sends.
    screens: HashMap<u32, Image>,
    next: u32,
}

impl Carts {
    /// Runs and stops what the engine asked for this frame, tells it how
    /// each run went, and writes the newest screen of each game into the
    /// renderer.
    pub fn tick(&mut self, client: &mut Client, renderer: &Renderer) {
        for game in self.games.values_mut() {
            game.made = game.opened;
        }
        for running in client.drain_running() {
            match running {
                Running::Open {
                    picture,
                    cart,
                    seed,
                } => {
                    let Some(file) = crate::assets::locate(&cart) else {
                        client.running_opened(
                            picture,
                            Err(format!("{cart}: out of the desktop's reach")),
                        );
                        continue;
                    };
                    self.start(client, picture, |number| Asked::Open { number, file, seed });
                }
                Running::Cover {
                    picture,
                    cart,
                    name,
                } => {
                    let Some(file) = crate::assets::locate(&cart) else {
                        client.running_opened(
                            picture,
                            Err(format!("{cart}: out of the desktop's reach")),
                        );
                        continue;
                    };
                    self.start(client, picture, |number| Asked::Cover {
                        number,
                        file,
                        name,
                    });
                }
                Running::Fail(picture) => self.start(client, picture, Asked::Fail),
                Running::Close(picture) => {
                    if let Some(number) = self.numbers.remove(&picture) {
                        self.games.remove(&number);
                        self.screens.remove(&number);
                        let _ = self.ask(Asked::Close(number));
                    }
                }
                Running::Press { picture, pad } => {
                    if let Some(&number) = self.numbers.get(&picture) {
                        let _ = self.ask(Asked::Press(number, pad.buttons()));
                    }
                }
                Running::Read(picture) => {
                    if let Some(&number) = self.numbers.get(&picture) {
                        let _ = self.ask(Asked::Read(number));
                    }
                }
                Running::Hold(picture) | Running::Resume(picture) => {
                    let runs = matches!(running, Running::Resume(_));
                    if let Some(&number) = self.numbers.get(&picture) {
                        let _ = self.ask(Asked::Runs(number, runs));
                    }
                }
            }
        }
        let mut died = false;
        if let Some(thread) = &self.thread {
            loop {
                let told = match thread.told.try_recv() {
                    Ok(told) => told,
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        died = true;
                        break;
                    }
                };
                match told {
                    Told::Screen(number, image) => {
                        if self.games.contains_key(&number) {
                            self.screens.insert(number, image);
                        }
                    }
                    Told::Opened(number) => {
                        if let Some(game) = self.games.get_mut(&number) {
                            game.opened = true;
                            client.running_opened(game.picture, Ok((WIDTH, HEIGHT)));
                        }
                    }
                    Told::Failed(number, message) => {
                        self.screens.remove(&number);
                        if let Some(game) = self.games.remove(&number) {
                            self.numbers.remove(&game.picture);
                            client.running_opened(game.picture, Err(message));
                        }
                    }
                    Told::Stopped(number) => {
                        if let Some(game) = self.games.get(&number) {
                            client.running_stopped(game.picture);
                        }
                    }
                    Told::Read(number, screen) => {
                        if let Some(game) = self.games.get(&number) {
                            client.running_read(game.picture, &screen.pixels, &screen.palette);
                        }
                    }
                }
            }
        }
        if died {
            self.died(client);
        }
        let games = &self.games;
        self.screens.retain(|number, image| {
            let Some(Game {
                picture,
                made: true,
                ..
            }) = games.get(number)
            else {
                return true;
            };
            renderer.write_picture(*picture, PictureFrame::Pixels(image));
            false
        });
    }

    /// Asks the thread to start a game into `picture` under the next
    /// number, and keeps it; one it cannot ask fails at once.
    fn start(&mut self, client: &mut Client, picture: PictureId, asked: impl FnOnce(u32) -> Asked) {
        self.next = self.next.wrapping_add(1);
        let number = self.next;
        match self.ask(asked(number)) {
            Ok(()) => {
                let game = Game {
                    picture,
                    opened: false,
                    made: false,
                };
                self.games.insert(number, game);
                self.numbers.insert(picture, number);
            }
            Err(message) => client.running_opened(picture, Err(message)),
        }
    }

    /// The thread died: every game it ran fails, and it is let go, to be
    /// started again for the next game, or given up when it was the last.
    fn died(&mut self, client: &mut Client) {
        let why = "the cartridges' thread died".to_owned();
        self.thread = None;
        self.screens.clear();
        self.numbers.clear();
        for (_, game) in self.games.drain() {
            client.running_opened(game.picture, Err(why.clone()));
        }
        if self.started >= STARTS {
            log::error!("{why}; started {STARTS} times, it is given up");
            self.given_up = Some(why);
        }
    }

    /// Asks the thread, starting it the first time, or again when it died.
    fn ask(&mut self, asked: Asked) -> Result<(), String> {
        if let Some(why) = &self.given_up {
            return Err(why.clone());
        }
        if self.thread.is_none() {
            let (asks, asked_there) = channel();
            // A screen waits for the frame to take it when the frame is
            // slow: a game behind skips, and nothing piles up.
            let (told_there, told) = sync_channel(LIVE_AT_ONCE * 2);
            std::thread::Builder::new()
                .name("cartridges".into())
                .spawn(move || {
                    if let Err(broken) = run(&asked_there, &told_there) {
                        log::error!("{broken}");
                    }
                })
                .map_err(|error| format!("the cartridges' thread: {error}"))?;
            self.started += 1;
            self.thread = Some(Thread { asks, told });
        }
        let thread = self.thread.as_ref().expect("the thread, just started");
        // A thread that died is let go by `tick`, once its last word is read.
        if thread.asks.send(asked).is_err() {
            return Err("the cartridges' thread died".into());
        }
        Ok(())
    }
}

/// Where prepared files are kept: `PLANET_CACHE`, else the platform's
/// cache folder; `None` when there is neither, and every file is prepared
/// each time it opens.
fn kept() -> Option<Kept> {
    let var = |name| std::env::var_os(name).map(PathBuf::from);
    let root = match var("PLANET_CACHE") {
        Some(root) => root,
        None if cfg!(target_os = "macos") => var("HOME")?.join("Library/Caches/planet"),
        None if cfg!(windows) => var("LOCALAPPDATA")?.join("planet"),
        None => var("XDG_CACHE_HOME")
            .or_else(|| var("HOME").map(|home| home.join(".cache")))?
            .join("planet"),
    };
    let root = root.join("prepared");
    Kept::new(&root, arcade::RUNNER_NAME, KEPT_MOST)
        .inspect_err(|error| log::warn!("{}: {error}; nothing is kept", root.display()))
        .ok()
}

/// Runs the games asked for, ticking them by the emulator's clock while
/// any runs or has a screen not yet said, and sleeping while it is idle: a
/// shelf of covers and stopped games wakes for nothing but the next ask.
/// Until the shell is gone, or the runner broke.
fn run(asked: &Receiver<Asked>, told: &SyncSender<Told>) -> Result<(), Broken> {
    let began = Instant::now();
    let now_ms = || began.elapsed().as_secs_f64() * 1000.0;
    let every = Duration::from_secs(1) / TICKS;
    let mut emulator = Emulator::hosted(arcade::RUNNER)?;
    let kept = kept();
    let mut due = Instant::now();
    // When the games were last ticked: a game is held as it was then, so
    // no update falls due between that tick and the hold to run while it
    // is held.
    let mut ticked = now_ms();
    loop {
        loop {
            let ask = match emulator.idle()? {
                true => asked.recv().map_err(|_| RecvTimeoutError::Disconnected),
                false => asked.recv_timeout(due.saturating_duration_since(Instant::now())),
            };
            let said = match ask {
                Ok(Asked::Open { number, file, seed }) => match std::fs::read(&file) {
                    Ok(bytes) => {
                        let now = now_ms();
                        match &kept {
                            Some(kept) => {
                                emulator.open_kept(kept, number, &bytes, seed, now, false)?
                            }
                            None => emulator.open(number, &bytes, seed, now, false)?,
                        }
                        told.send(Told::Opened(number))
                    }
                    Err(error) => {
                        told.send(Told::Failed(number, format!("{}: {error}", file.display())))
                    }
                },
                Ok(Asked::Cover { number, file, name }) => match std::fs::read(&file) {
                    Ok(bytes) => {
                        emulator.still(number, &bytes, &name, now_ms())?;
                        told.send(Told::Opened(number))
                    }
                    Err(error) => {
                        told.send(Told::Failed(number, format!("{}: {error}", file.display())))
                    }
                },
                Ok(Asked::Fail(number)) => {
                    emulator.fail(number, now_ms())?;
                    told.send(Told::Opened(number))
                }
                Ok(Asked::Close(number)) => {
                    emulator.close(number)?;
                    Ok(())
                }
                Ok(Asked::Press(number, buttons)) => {
                    emulator.press(number, buttons)?;
                    Ok(())
                }
                Ok(Asked::Runs(number, runs)) => {
                    match runs {
                        true => emulator.pace(number, 1.0, now_ms())?,
                        false => emulator.pace(number, 0.0, ticked)?,
                    }
                    Ok(())
                }
                Ok(Asked::Read(number)) => match emulator.read(number)? {
                    Some(screen) => told.send(Told::Read(number, screen)),
                    None => Ok(()),
                },
                Err(RecvTimeoutError::Timeout) => break,
                Err(RecvTimeoutError::Disconnected) => return Ok(()),
            };
            if said.is_err() {
                return Ok(());
            }
        }
        due = Instant::now() + every;
        ticked = now_ms();
        for number in emulator.tick(ticked)? {
            let Some(rgba) = emulator.rgba(number)? else {
                continue;
            };
            let image = Image {
                width: WIDTH,
                height: HEIGHT,
                rgba,
            };
            if told.send(Told::Screen(number, image)).is_err() {
                return Ok(());
            }
            if emulator.stopped(number)? && told.send(Told::Stopped(number)).is_err() {
                return Ok(());
            }
        }
        // No audio: the runner keeps a few updates of what each game played
        // and lets the oldest go, so nothing is taken here.
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_game_whose_file_never_came_opens_as_its_stop_screen() {
        let (asks, asked) = channel();
        let (told_there, told) = sync_channel(8);
        let thread = std::thread::spawn(move || run(&asked, &told_there).unwrap());
        asks.send(Asked::Fail(1)).unwrap();
        let next = || {
            told.recv_timeout(Duration::from_secs(5))
                .expect("the thread answers")
        };
        assert!(matches!(next(), Told::Opened(1)));
        let Told::Screen(1, image) = next() else {
            panic!("its screen comes next");
        };
        // The stop screen's own ground, never a blank one.
        assert_eq!(image.rgba[..4], [24, 8, 14, 255]);
        assert!(matches!(next(), Told::Stopped(1)));
        drop(asks);
        thread.join().unwrap();
    }

    #[test]
    fn a_cover_opens_as_its_picture_and_never_runs_or_stops() {
        let (asks, asked) = channel();
        let (told_there, told) = sync_channel(8);
        let thread = std::thread::spawn(move || run(&asked, &told_there).unwrap());
        // A file with no cover: the stand-in names it.
        let file = std::env::temp_dir().join(format!("cover-{}.wasm", std::process::id()));
        std::fs::write(&file, b"\0asm\x01\0\0\0").unwrap();
        let name = "c0ffee".to_owned();
        asks.send(Asked::Cover {
            number: 1,
            file: file.clone(),
            name: name.clone(),
        })
        .unwrap();
        let next = || told.recv_timeout(Duration::from_secs(5));
        assert!(matches!(next(), Ok(Told::Opened(1))));
        let Ok(Told::Screen(1, image)) = next() else {
            panic!("its picture comes next");
        };
        let mut emulator = Emulator::hosted(arcade::RUNNER).unwrap();
        let cover = emulator.cover(b"\0asm\x01\0\0\0", &name).unwrap();
        assert_eq!(cover.said, None);
        assert_eq!(image.rgba, cover.rgba);
        // Nothing more: it never runs, plays or stops.
        assert!(told.recv_timeout(Duration::from_millis(300)).is_err());
        std::fs::remove_file(file).unwrap();
        drop(asks);
        thread.join().unwrap();
    }
}
