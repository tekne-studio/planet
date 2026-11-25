//! The browser side of the cabinets' games: the engine says which cartridge
//! to run into which texture (`client::Running`), and a worker of the
//! page's runs it (`apps/web/src/lib/engine/cartridges.worker.ts`, on the
//! emulator the engine pins), so a game never takes the frame's thread. The
//! worker ticks its games when told the page drew a frame, by the emulator's
//! clock, so it runs no faster than the page draws and not at all in a
//! hidden tab; it is told only while a game runs or a screen is still to
//! be said, so covers and stop screens alone cost no tick. Each screen that changed comes back as its pixels, handed
//! over, and the newest of each game is written into the renderer once a
//! frame; what a running game played goes to its sound, as the emulator
//! batched it, a stream the audio opens as the game starts and ends as it
//! stops. A game held, its player gone, holds in the worker and in its
//! stream, and runs on in both from where it was; a game fed runs the
//! buttons it is given, at its pace or at once, and holds where they end;
//! the pads each file plays come back with its answer; a game that stopped
//! is said to the engine, one whose file the engine gave up running as its
//! stop screen, and a cover asked for as a cart that never runs. What the controls
//! press on a game goes to the worker as its buttons, the pad's bits being
//! the contract's. The worker is made the first time a game is asked for, by the
//! maker the page gave, and ends with the engine; one that dies is made
//! again once, and one that dies again is given up, said so, and every game
//! after fails.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use client::{Client, Running};
use js_sys::{ArrayBuffer, Function, Object, Reflect, Uint8Array, Uint32Array};
use render::PictureFrame;
use scene::{Image, PictureId};
use wasm_bindgen::prelude::*;
use web_sys::{ErrorEvent, Event, MessageEvent, Worker};

use crate::audio::Audio;
use crate::games::{Game, Games, Pace};
use crate::heard::{Answer, Heard, Read, Told};
use crate::web::{State, resolve};

/// How many workers are made: the first, and one more after it died.
const WORKERS: u32 = 2;

/// The worker, and the closures that hear it, kept as long as it lives.
struct Listened {
    worker: Worker,
    _hears: Closure<dyn FnMut(MessageEvent)>,
    _fails: Closure<dyn FnMut(Event)>,
}

impl Drop for Listened {
    fn drop(&mut self) {
        self.worker.terminate();
    }
}

/// The worker and what it said since the last frame read it.
#[derive(Default)]
pub struct Carts {
    /// Makes the worker: the page's, given once the engine is made.
    make: Option<Function>,
    worker: Option<Listened>,
    heard: Rc<RefCell<Heard<ArrayBuffer>>>,
    /// How many workers were made, and why they were given up, once the
    /// last of them died.
    workers: u32,
    given_up: Option<String>,
    games: Games,
    /// The newest screen of each game not yet written: one that comes
    /// before its texture is made waits for it, a stop screen being the
    /// last a game sends.
    screens: HashMap<u32, ArrayBuffer>,
}

impl Carts {
    /// The page's maker of the worker.
    pub fn set_maker(&mut self, make: Function) {
        self.make = Some(make);
    }

    fn worker(&mut self) -> Result<&Worker, String> {
        if let Some(why) = &self.given_up {
            return Err(why.clone());
        }
        if self.worker.is_none() {
            let make = self
                .make
                .as_ref()
                .ok_or("the page gave no worker for cartridges")?;
            let worker: Worker = make
                .call0(&JsValue::NULL)
                .map_err(|error| format!("the cartridges' worker: {error:?}"))?
                .dyn_into()
                .map_err(|_| "the cartridges' maker gave no worker".to_owned())?;
            let heard = Rc::clone(&self.heard);
            let hears = Closure::<dyn FnMut(MessageEvent)>::new(move |event: MessageEvent| {
                if let Some(told) = told(&event.data()) {
                    heard.borrow_mut().hear(told);
                }
            });
            let heard = Rc::clone(&self.heard);
            let fails = Closure::<dyn FnMut(Event)>::new(move |event: Event| {
                let why = event
                    .dyn_ref::<ErrorEvent>()
                    .map(ErrorEvent::message)
                    .filter(|why| !why.is_empty())
                    .unwrap_or_else(|| "it did not load".into());
                heard.borrow_mut().hear(Told::Dead(why));
            });
            worker.set_onmessage(Some(hears.as_ref().unchecked_ref()));
            worker.set_onerror(Some(fails.as_ref().unchecked_ref()));
            self.workers += 1;
            self.worker = Some(Listened {
                worker,
                _hears: hears,
                _fails: fails,
            });
        }
        Ok(&self.worker.as_ref().expect("the worker, just made").worker)
    }

    fn post(&mut self, message: &[(&str, JsValue)]) -> Result<(), String> {
        let object = Object::new();
        for (key, value) in message {
            Reflect::set(&object, &JsValue::from_str(key), value).ok();
        }
        self.worker()?
            .post_message(&object)
            .map_err(|error| format!("the cartridges' worker: {error:?}"))
    }

    /// Asks the worker to start a game into `picture` under the next
    /// number, by the message `asked` makes of it, and keeps it; one it
    /// cannot ask fails at once.
    fn start(
        &mut self,
        client: &mut Client,
        picture: PictureId,
        pace: Pace,
        asked: impl FnOnce(u32) -> Vec<(&'static str, JsValue)>,
    ) {
        let number = self.games.next();
        match self.post(&asked(number)) {
            Ok(()) => self.games.insert(number, picture, pace),
            Err(message) => client.running_opened(picture, Err(message)),
        }
    }

    /// Runs the game in `picture` on, or holds it where it is: in the
    /// worker, which ticks it no more while it holds, and in its stream.
    fn pace(&mut self, picture: PictureId, pace: Pace, audio: &mut Audio) {
        let Some(number) = self.games.pace(picture, pace) else {
            return;
        };
        let runs = pace == Pace::Runs;
        let asked = if runs { "resume" } else { "hold" };
        let _ = self.post(&[(asked, number.into())]);
        match runs {
            true => audio.resume_game(picture),
            false => audio.hold_game(picture),
        }
    }

    /// The worker died: every game it ran fails, its stream ended, and it
    /// is let go, to be made again for the next game, or given up when it
    /// was the last.
    fn died(&mut self, client: &mut Client, audio: &mut Audio, why: &str) {
        let why = format!("the cartridges' worker died: {why}");
        self.worker = None;
        self.screens.clear();
        for game in self.games.drain() {
            audio.close_game(game.picture);
            client.running_opened(game.picture, Err(why.clone()));
        }
        if self.workers >= WORKERS {
            log::error!("{why}; made {WORKERS} times, it is given up");
            self.given_up = Some(why);
        }
    }
}

/// What a message from the worker says.
fn told(message: &JsValue) -> Option<Told<ArrayBuffer>> {
    let get = |key: &str| Reflect::get(message, &JsValue::from_str(key)).ok();
    let count = |key: &str| get(key)?.as_f64().map(|n| n as u32);
    if let Some(number) = count("frame") {
        let screen = get("rgba")?.dyn_into().ok()?;
        Some(Told::Screen { number, screen })
    } else if let Some(number) = count("opened") {
        Some(Told::Answer(Answer::Opened {
            number,
            width: count("width").unwrap_or(0),
            height: count("height").unwrap_or(0),
            plays: count("plays").map(|plays| plays as usize),
        }))
    } else if let Some(number) = count("failed") {
        let message = get("message")?.as_string().unwrap_or_default();
        Some(Told::Answer(Answer::Failed { number, message }))
    } else if let Some(number) = count("stopped") {
        Some(Told::Answer(Answer::Stopped { number }))
    } else if let Some(number) = count("read") {
        let bytes = |key: &str| -> Option<Vec<u8>> {
            let buffer: ArrayBuffer = get(key)?.dyn_into().ok()?;
            Some(Uint8Array::new(&buffer).to_vec())
        };
        let palette = bytes("palette")?
            .chunks_exact(4)
            .map(|colour| [colour[0], colour[1], colour[2], colour[3]])
            .collect();
        let read = Read {
            palette,
            pixels: bytes("pixels")?,
        };
        Some(Told::Read { number, read })
    } else if let Some(number) = count("sound") {
        let batch = get("batch")?.dyn_into::<ArrayBuffer>().ok()?;
        let batch = Uint32Array::new(&batch).to_vec();
        Some(Told::Sound { number, batch })
    } else {
        get("dead")?.as_string().map(Told::Dead)
    }
}

/// Runs and stops what the engine asked for this frame, tells it how each
/// run went, writes the newest screen of each game into the renderer, and
/// tells the worker a frame was drawn while a game runs or a screen is
/// still to be said.
pub fn tick(state: &Rc<RefCell<State>>) {
    let mut held = state.borrow_mut();
    let State {
        client,
        carts,
        renderer,
        files,
        audio,
        ..
    } = &mut *held;
    for game in carts.games.values_mut() {
        game.made = game.size.is_some();
    }
    for running in client.drain_running() {
        match running {
            Running::Open {
                picture,
                cart,
                seed,
                fed,
            } => {
                let url = resolve(&cart, files);
                carts.start(client, picture, Pace::Runs, |number| {
                    vec![
                        ("open", number.into()),
                        ("url", url.into()),
                        ("seed", seed.into()),
                        ("fed", fed.into()),
                    ]
                });
                if carts.games.number(picture).is_some() {
                    audio.open_game(picture);
                }
            }
            Running::Cover {
                picture,
                cart,
                name,
            } => {
                let url = resolve(&cart, files);
                carts.start(client, picture, Pace::Still, |number| {
                    vec![
                        ("cover", number.into()),
                        ("url", url.into()),
                        ("name", name.into()),
                    ]
                });
            }
            Running::Fail(picture) => {
                carts.start(client, picture, Pace::Still, |number| {
                    vec![("fail", number.into())]
                });
            }
            Running::Close(picture) => {
                audio.close_game(picture);
                if let Some(number) = carts.games.close(picture) {
                    carts.screens.remove(&number);
                    let _ = carts.post(&[("close", number.into())]);
                }
            }
            Running::Press { picture, pad } => {
                if let Some(number) = carts.games.number(picture) {
                    let _ =
                        carts.post(&[("press", number.into()), ("buttons", pad.buttons().into())]);
                }
            }
            Running::Read(picture) => {
                if let Some(number) = carts.games.number(picture) {
                    let _ = carts.post(&[("read", number.into())]);
                }
            }
            Running::Feed {
                picture,
                buttons,
                at_once,
            } => {
                if let Some(number) = carts.games.number(picture) {
                    let buttons = Uint32Array::from(&buttons[..]);
                    let _ = carts.post(&[
                        ("feed", number.into()),
                        ("buttons", buttons.into()),
                        ("atOnce", at_once.into()),
                    ]);
                }
            }
            Running::Hold(picture) => carts.pace(picture, Pace::Held, audio),
            Running::Resume(picture) => carts.pace(picture, Pace::Runs, audio),
        }
    }
    let heard = carts.heard.borrow_mut().take();
    for answer in heard.answers {
        match answer {
            Answer::Opened {
                number,
                width,
                height,
                plays,
            } => {
                if let Some(game) = carts.games.get_mut(number) {
                    game.size = Some((width, height));
                    if let Some(plays) = plays {
                        client.running_plays(game.picture, plays);
                    }
                    client.running_opened(game.picture, Ok((width, height)));
                }
            }
            Answer::Failed { number, message } => {
                carts.screens.remove(&number);
                if let Some(game) = carts.games.failed(number) {
                    audio.close_game(game.picture);
                    client.running_opened(game.picture, Err(message));
                }
            }
            Answer::Stopped { number } => {
                if let Some(game) = carts.games.get_mut(number) {
                    game.pace = Pace::Still;
                    audio.close_game(game.picture);
                    client.running_stopped(game.picture);
                }
            }
        }
    }
    for (number, screen) in heard.screens {
        if let Some(game) = carts.games.get_mut(number) {
            game.said = true;
            carts.screens.insert(number, screen);
        }
    }
    for (number, played) in &heard.played {
        if let Some(picture) = carts.games.heard(*number) {
            audio.play_game(picture, played);
        }
    }
    for (number, read) in heard.reads {
        if let Some(game) = carts.games.get(number) {
            client.running_read(game.picture, &read.pixels, &read.palette);
        }
    }
    if let Some(why) = heard.dead {
        carts.died(client, audio, &why);
    }
    let games = &carts.games;
    carts.screens.retain(|number, buffer| {
        let Some(Game {
            picture,
            size: Some((width, height)),
            made: true,
            ..
        }) = games.get(*number)
        else {
            return true;
        };
        let image = Image {
            width: *width,
            height: *height,
            rgba: Uint8Array::new(buffer).to_vec(),
        };
        if image.rgba.len() == (width * height * 4) as usize {
            renderer.write_picture(*picture, PictureFrame::Pixels(&image));
        }
        false
    });
    if carts.games.tick() {
        let _ = carts.post(&[("tick", true.into())]);
    }
}
