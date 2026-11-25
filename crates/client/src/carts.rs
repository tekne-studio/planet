//! The games on cabinets' screens. A cabinet in view and within the play
//! distance that nobody plays shows its game's cover, still: the client
//! asks the shell to show it ([`Running::Cover`]), read from the file and
//! never run, one picture a cartridge that every cabinet of it shows, so
//! a room of idle cabinets costs one texture and plays no sound. A game
//! played ([`Carts::play`]) runs: the client asks the shell to run it from
//! a seed into a texture of its cabinet's ([`Running::Open`]), and the
//! shell runs it off the frame's thread and writes every frame it makes
//! into the renderer, as it does a moving picture's (`crate::pictures`),
//! so the engine never sees a frame. A game is a live picture, in the
//! budget it shares with the moving pictures ([`crate::live`]).
//!
//! A game its player leaves waits for them: it stays open, and the same
//! player sitting again within the window takes it back ([`GAME_WAITS_S`], or
//! [`GAME_WAITS_AWAY_S`] for one who went away from the world). What the game
//! does while it waits is its players' policy, apart from the waiting, so a
//! game of several can run on while one of them is away: a game of one
//! player is held ([`Running::Hold`]), no update run and its sound stopped
//! where it is, and runs on from there as the player sits again
//! ([`Running::Resume`]), as if never left. Past the window the game stops
//! and its cover takes its place again. A game waiting keeps its place in
//! the live budget and gives it to a game wanted first.
//!
//! A game whose file plays several pads, as its cover says
//! ([`crate::Client::running_plays`]), is fed: it runs only the updates its
//! player's plugin gives it, each update's buttons for every pad
//! ([`Carts::feed`]), and holds where they end. What it was given is
//! counted here ([`Carts::given`]): the plugin that plays it, which agrees
//! every update with the other players' machines, gives it the rest. It is
//! never held or pressed by the client: whether it runs while its player is
//! away is that plugin's to say, by feeding it or not.
//! A game that stopped for good shows why until it is let go, out of view
//! a while, and is played no more until then. Until a screen has shown a
//! frame, it is the cabinet's dark glass. A file that does not come shows
//! a stopped game's screen saying so ([`Running::Fail`]): a cover's is
//! asked for again after a while, and a game's, once the live pictures
//! gave it up, never.
//!
//! The controls lent to a cabinet press its game's buttons
//! ([`Carts::press`]), said to the shell whenever they change and let go
//! the frame nobody presses them, so no button outlives the person
//! pressing it. Each client runs its own copy of a game: a person plays
//! theirs, and those near see the cover on theirs. A game running is a
//! sound at its screen ([`SoundSource::Game`]), heard within
//! [`HEARD_M`]: the shell plays what the game played into it.

use std::collections::{HashMap, VecDeque};

use glam::{DAffine3, DVec3};
use part::{ContentId, PartId, Shape};
use scene::{PictureChange, PictureDraw, PictureId};

use crate::controls::Pad;
use crate::live::{Failure, Live, Order, Source};
use crate::parts::Parts;
use crate::pictures::Screen;
use crate::{Event, Sound, SoundSource};

/// The pictures of games are named over this, apart from those of
/// `pictures`.
const NAMES: u64 = 1 << 62;

/// Frames a stopped game's last frame stays out of view before it is let
/// go: ten seconds at sixty.
const SHOWN_UNSEEN_FRAMES: u64 = 600;

/// How much a game's screen shines: its own colour, as a screen shows it.
const SCREEN_SHINE: f32 = 1.0;

/// How far a game is heard, metres: across a room of cabinets, and gone
/// before the next room.
pub(crate) const HEARD_M: f64 = 12.0;

/// How long a game waits for the player who left it in the world, by its
/// offer, the key that leaves or anything else that ends a game played,
/// seconds: a slip of a key, and a step back to the cabinet.
pub const GAME_WAITS_S: f64 = 10.0;

/// How long a game waits for a player who went away from the world, the
/// page hidden or covered or its window left, seconds: a while to answer
/// someone and come back. Longer than [`GAME_WAITS_S`]: one called away
/// has not walked off, and a game held costs no tick and no sound.
pub const GAME_WAITS_AWAY_S: f64 = 60.0;

/// The updates a second a game runs at: the console's.
pub const UPDATES_PER_S: u32 = arcade::TICKS;

/// The pads a game may read, each a player's own: the console's.
pub const PADS: usize = arcade::PADS;

/// The most updates a game is given in all, the longest session the
/// console keeps: the console's.
pub const GAME_LONGEST: u64 = arcade::LONGEST as u64;

/// A game the shell runs, as the client wants it: run when the client wants
/// it seen, stopped when it stops wanting it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Running {
    /// Run the cartridge at `cart` from `seed` into `picture`, with no button
    /// held. The shell answers with the size of its screen
    /// ([`crate::Client::running_opened`]), and the renderer gets a
    /// texture of it if it has none.
    ///
    /// A game `fed` runs only the updates it is fed ([`Running::Feed`]),
    /// and holds from its first until it is.
    Open {
        picture: PictureId,
        cart: String,
        seed: u32,
        fed: bool,
    },
    /// Give the game fed in `picture` its next updates, one an update, each
    /// every pad's buttons in the update's word: it runs them at its pace
    /// after those it was given before, and holds once it ran them all. Run
    /// `at_once`, it runs them now, as fast as it can, and plays none of
    /// their sound: a game catching up.
    Feed {
        picture: PictureId,
        buttons: Vec<u32>,
        at_once: bool,
    },
    /// Stop it. Its texture stays, showing the last frame, until the client
    /// lets it go.
    Close(PictureId),
    /// Hold `pad` down on the game in `picture`, in the place of what was
    /// held, from its next update on. A control pressed and let go between
    /// two updates is held for the next one all the same. A game opens with
    /// nothing held.
    Press { picture: PictureId, pad: Pad },
    /// Show in `picture` the cover of the cartridge at `cart`, still, as
    /// the cartridge's file carries it, or a stand-in naming the file when
    /// it carries none: answered as an open is, with the size of its
    /// screen. Nothing of the file runs. `name` is the file's as the
    /// stand-in shows it: the hash of its bytes. A cover whose file did not
    /// come is shown so ([`Running::Fail`]) into the same picture, and asked
    /// again after a [`Running::Close`] of it.
    Cover {
        picture: PictureId,
        cart: String,
        name: String,
    },
    /// Read the screen of the game in `picture` and its palette, as it is:
    /// the shell answers with [`crate::Client::running_read`].
    Read(PictureId),
    /// Hold the game in `picture` where it is: no update runs, none is made
    /// up later, its sound eases out and stops where it is, and its screen
    /// keeps its last frame.
    Hold(PictureId),
    /// Run the game held in `picture` on from where it was held, its sound
    /// eased in where it stopped: the updates and the sound go on as if it
    /// was never held.
    Resume(PictureId),
    /// Show in `picture` that its game's file never came, as a game that
    /// stopped shows why it did: answered as an open is, with the size of
    /// its screen, and then as stopped. A [`Running::Close`] lets it go.
    Fail(PictureId),
}

/// A game is known by its cabinet, this client's name of the part, and its
/// cartridge: a cabinet given another game is another game.
pub(crate) type GameId = (u64, ContentId);

/// One cabinet's game.
struct Game {
    picture: PictureId,
    /// What it runs from: the cabinet's name in the world, the same for
    /// every client.
    seed: u32,
    /// Whether the renderer holds its texture: a game that showed a frame,
    /// until its cover takes its place.
    made: bool,
    /// The frame it was last in view in.
    seen: u64,
    /// Whether it stopped for good: the shell said it broke its contract or
    /// ran out of its budget, or was told its file never came. The shell
    /// keeps its screen, saying why, and it is played no more, until it is
    /// let go.
    stopped: bool,
    /// Whether the shell was told its file never came ([`Running::Fail`]):
    /// no longer a live picture.
    failed: bool,
    /// What the shell was last told is held on it.
    held: Pad,
    /// Where its screen stood when last drawn: where it is heard.
    at: DVec3,
    /// The size of its screen, once the shell said it.
    size: Option<(u32, u32)>,
    /// Its player gone, while it waits for them.
    left: Option<Left>,
    /// What a game fed was given since it opened: fed games only.
    fed: Option<Fed>,
}

/// What a game fed was given since it opened.
#[derive(Clone, Copy, Default)]
struct Fed {
    /// The updates given.
    given: u64,
    /// The buttons of the update given last, every pad's.
    last: u32,
    /// Whether the shell answered its open: what is fed before then is
    /// never given.
    ready: bool,
}

/// A player gone from a game that waits for them: since when on the
/// games' clock, and for how long, seconds.
#[derive(Clone, Copy)]
struct Left {
    at_s: f64,
    for_s: f64,
}

/// A cartridge's cover, still ([`Running::Cover`]): one picture a
/// cartridge, shown on every cabinet of it nobody plays, so a room of one
/// game asks the shell once and holds one texture.
struct Cover {
    picture: PictureId,
    /// Whether the renderer holds its texture.
    made: bool,
    /// The size of its screen, once the shell said it.
    size: Option<(u32, u32)>,
    /// The frame a cabinet of it was last in view within the play distance.
    seen: u64,
    /// Whether the shell shows that its file never came, in its place
    /// ([`Running::Fail`]), until it is asked again.
    missing: bool,
    /// The pads its file plays, once the shell read it.
    plays: Option<usize>,
    /// Its asks that failed in a row, with the last: its file did not come.
    /// It is asked again after a while, as a live picture is.
    failure: Option<Failure>,
}

/// The game a cabinet runs, by the part: its cartridge, while it has one.
pub(crate) fn game_of(parts: &Parts, id: PartId) -> Option<GameId> {
    let (_, part) = parts.get(id)?;
    match part.shape {
        Shape::Cabinet(cabinet) => Some((id.0, cabinet.cart?)),
        _ => None,
    }
}

/// Whether the cabinet of the game `key` is still in the world with the
/// same cartridge.
pub(crate) fn runs(parts: &Parts, key: GameId) -> bool {
    game_of(parts, PartId(key.0)) == Some(key)
}

/// A screen asked for, as the shell reads it.
pub(crate) struct Read {
    cabinet: u64,
    width: u32,
    height: u32,
    /// Whether its pixels were asked for.
    indices: bool,
}

impl Read {
    /// What is said of the screen read: its pixels, an index into the
    /// palette each, and the palette's colours.
    pub(crate) fn said(self, pixels: &[u8], palette: &[[u8; 4]]) -> Event {
        // FNV-1a, 64 bits: the same screen hashes the same on every runner.
        let hash = palette
            .iter()
            .flatten()
            .chain(pixels)
            .fold(0xcbf2_9ce4_8422_2325_u64, |hash, &byte| {
                (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3)
            });
        // One digit a pixel, base 32: a digit a colour of a game's 32.
        const DIGITS: &[u8; 32] = b"0123456789abcdefghijklmnopqrstuv";
        Event::Screen {
            part: self.cabinet,
            width: self.width,
            height: self.height,
            hash: format!("{hash:016x}"),
            palette: palette
                .iter()
                .map(|[r, g, b, _]| format!("#{r:02x}{g:02x}{b:02x}"))
                .collect(),
            indices: self.indices.then(|| {
                pixels
                    .iter()
                    .map(|&index| char::from(DIGITS[usize::from(index) % DIGITS.len()]))
                    .collect()
            }),
        }
    }
}

/// The games of the cabinets this client draws, and their cartridges'
/// covers. A frame's work is in proportion to the games drawn, pressed and
/// held in it, never to every game kept: those out of view wait in
/// `unseen`, in the order they left. The covers are one a cartridge seen
/// within ten seconds, a handful, and each frame looks at them all.
#[derive(Default)]
pub struct Carts {
    games: HashMap<GameId, Game>,
    /// Each game by its picture, as the shell names it.
    pictures: HashMap<PictureId, GameId>,
    covers: HashMap<ContentId, Cover>,
    /// Each cover by its picture.
    covered: HashMap<PictureId, ContentId>,
    next: u64,
    frame: u64,
    changes: Vec<PictureChange>,
    running: Vec<Running>,
    /// What is pressed on each game this frame.
    pressed: HashMap<GameId, Pad>,
    /// The games the shell was told something is held on.
    holding: Vec<GameId>,
    /// The games in view this frame, and in the last.
    drawn: Vec<GameId>,
    shown: Vec<GameId>,
    /// The games that left view, each with the last frame it was in view,
    /// oldest first: one still out of view a while later is let go.
    unseen: VecDeque<(u64, GameId)>,
    /// The game pressed last frame: the one played.
    played: Option<GameId>,
    /// The games someone plays, each with the frame it was last asked
    /// to run in ([`Carts::play`]).
    playing: HashMap<GameId, u64>,
    /// The screens asked for and not yet read, by the picture the shell
    /// reads: each cabinet that asked, and whether with pixels. A cover's
    /// picture is read once for every cabinet of its cartridge that asked.
    reads: HashMap<PictureId, Vec<(u64, bool)>>,
    /// The games waiting for a player who left, in the order they left.
    waiting: Vec<GameId>,
    /// The games fed to run again from their start ([`Carts::restart`]).
    restarts: Vec<GameId>,
    /// The games' clock, seconds of the frames the client ran: a window
    /// is as long on a slow screen as on a fast one, and a hidden page's
    /// time counts at its first frame back.
    now_s: f64,
}

impl Carts {
    /// The screen of the game `cart` on the cabinet this client names
    /// `cabinet` and the world `named`, placed by `world_from_quad`, this
    /// frame, or `None` and the cabinet's glass shows. In view within the
    /// play distance it shows its cartridge's cover, or, played, is wanted
    /// live and runs from `named`, the same seed on every client, its
    /// frames shown once the shell made them. Its last frame shows until
    /// the cover takes its place, and a game stopped for good shows why
    /// until it is let go.
    pub(crate) fn draw(
        &mut self,
        live: &mut Live,
        (cabinet, named): (u64, u64),
        cart: ContentId,
        world_from_quad: DAffine3,
        screen: &Screen,
    ) -> Option<PictureDraw> {
        let key = (cabinet, cart);
        let source = Source::Game(cabinet, cart);
        let seed = seed(named);
        let width_m = world_from_quad.matrix3.x_axis.length();
        let height_m = world_from_quad.matrix3.y_axis.length();
        let radius_m = 0.5 * libm::hypot(width_m, height_m);
        let centre = world_from_quad.translation;
        let (px, in_view) = screen.measure(centre, radius_m, height_m);
        let near = in_view && (centre - screen.camera.position).length() <= screen.play_m;
        let frame = self.frame;
        if near {
            self.want_cover(cart);
            let (next, pictures, drawn) = (&mut self.next, &mut self.pictures, &mut self.drawn);
            self.games.entry(key).or_insert_with(|| {
                *next += 1;
                let picture = PictureId(NAMES | *next);
                pictures.insert(picture, key);
                drawn.push(key);
                Game {
                    picture,
                    seed,
                    made: false,
                    seen: frame,
                    stopped: false,
                    failed: false,
                    held: Pad::NONE,
                    at: centre,
                    size: None,
                    left: None,
                    fed: None,
                }
            });
        }
        let game = self.games.get_mut(&key)?;
        game.at = centre;
        let played = self
            .playing
            .get(&key)
            .is_some_and(|&asked| frame - asked <= 1);
        if near && played && !game.stopped {
            live.want(source, px);
        }
        if near && !game.failed && live.given_up(source) {
            game.failed = true;
            game.stopped = true;
            self.running.push(Running::Fail(game.picture));
        }
        if in_view && game.seen != frame {
            game.seen = frame;
            self.drawn.push(key);
        }
        let cover = self.covers.get(&cart).filter(|cover| near && cover.made);
        let picture = if game.made && (game.stopped || live.is_open(source)) {
            game.picture
        } else if let Some(cover) = cover {
            // Its last frame gives way to the cover, and its texture goes.
            if core::mem::take(&mut game.made) {
                game.size = None;
                self.changes.push(PictureChange::Remove(game.picture));
            }
            cover.picture
        } else if game.made {
            game.picture
        } else {
            return None;
        };
        Some(PictureDraw {
            picture: Some(picture),
            transform: world_from_quad,
            shine: SCREEN_SHINE,
            both_sides: false,
        })
    }

    /// Wants the cover of `cart` shown this frame: asked for the first
    /// time, and asked again once a failure waited its while.
    fn want_cover(&mut self, cart: ContentId) {
        let frame = self.frame;
        let (next, covered, running) = (&mut self.next, &mut self.covered, &mut self.running);
        let cover = self.covers.entry(cart).or_insert_with(|| {
            *next += 1;
            let picture = PictureId(NAMES | *next);
            covered.insert(picture, cart);
            running.push(Running::Cover {
                picture,
                cart: part::Cabinet::cart_name(cart),
                name: cart.hex(),
            });
            Cover {
                picture,
                made: false,
                size: None,
                seen: frame,
                missing: false,
                plays: None,
                failure: None,
            }
        });
        cover.seen = frame;
        if cover.missing
            && cover
                .failure
                .as_ref()
                .is_some_and(|failure| !failure.waits(frame))
        {
            // The shell lets its stop screen go first.
            cover.missing = false;
            self.reads.remove(&cover.picture);
            running.push(Running::Close(cover.picture));
            running.push(Running::Cover {
                picture: cover.picture,
                cart: part::Cabinet::cart_name(cart),
                name: cart.hex(),
            });
        }
    }

    /// The shell's answer to a [`Running::Open`], a [`Running::Cover`] or a
    /// [`Running::Fail`]: the size of the screen, or why it does not show,
    /// said too of a game running when the runner it ran in died. A game
    /// that failed is asked for again after a while; a cover whose file did
    /// not come shows so, and is asked for again after a while too.
    pub(crate) fn opened(
        &mut self,
        picture: PictureId,
        size: Result<(u32, u32), String>,
        live: &mut Live,
    ) -> Result<(), String> {
        if let Some(cart) = self.covered.get(&picture) {
            return self.cover_opened(*cart, size);
        }
        let Some(&(cabinet, cart)) = self.pictures.get(&picture) else {
            return Ok(());
        };
        let Some(game) = self.games.get_mut(&(cabinet, cart)) else {
            return Ok(());
        };
        // Stopped before it answered: the shell heard the stop. A failed
        // game's answer is its screen saying so, outside the live budget.
        if !game.failed && !live.answered(Source::Game(cabinet, cart), size.is_ok()) {
            return Ok(());
        }
        let (width, height) = size?;
        game.size = Some((width, height));
        if let Some(fed) = &mut game.fed {
            fed.ready = true;
        }
        if !game.made {
            // A game's pixels are squares, drawn large as such.
            self.changes.push(PictureChange::Live {
                id: picture,
                width,
                height,
                sharp: true,
            });
            game.made = true;
        }
        Ok(())
    }

    /// The shell read the file of the cover or the game in `picture`: it
    /// plays `plays` pads.
    pub(crate) fn plays_read(&mut self, picture: PictureId, plays: usize) {
        let cart = match self.covered.get(&picture) {
            Some(&cart) => cart,
            None => match self.pictures.get(&picture) {
                Some(&(_, cart)) => cart,
                None => return,
            },
        };
        if let Some(cover) = self.covers.get_mut(&cart) {
            cover.plays = Some(plays);
        }
    }

    /// The pads the game `key` plays, once its cover was read.
    pub(crate) fn plays(&self, key: GameId) -> Option<usize> {
        self.covers.get(&key.1)?.plays
    }

    /// The updates given to the game `key`, fed, since it opened, while the
    /// shell runs it: `None` for a game not fed, not open, not yet
    /// answered, or stopped.
    pub(crate) fn given(&self, key: GameId, live: &Live) -> Option<u64> {
        let game = self.games.get(&key).filter(|game| !game.stopped)?;
        let fed = game.fed.filter(|fed| fed.ready)?;
        live.is_open(Source::Game(key.0, key.1))
            .then_some(fed.given)
    }

    /// Gives the game `key`, fed, its next updates' buttons, run at its pace
    /// or `at_once` ([`Running::Feed`]): nothing for a game [`Carts::given`]
    /// says nothing of.
    pub(crate) fn feed(&mut self, key: GameId, buttons: &[u32], at_once: bool, live: &Live) {
        if buttons.is_empty() || self.given(key, live).is_none() {
            return;
        }
        let Some(game) = self.games.get_mut(&key) else {
            return;
        };
        if let (Some(fed), Some(&last)) = (&mut game.fed, buttons.last()) {
            fed.given += buttons.len() as u64;
            fed.last = last;
        }
        self.running.push(Running::Feed {
            picture: game.picture,
            buttons: buttons.to_vec(),
            at_once,
        });
    }

    /// Runs the game `key`, fed, again from its start, as the frame ends:
    /// it stops, and opens again as its player wants it, given nothing.
    pub(crate) fn restart(&mut self, key: GameId) {
        if self.games.get(&key).is_some_and(|game| game.fed.is_some()) {
            self.restarts.push(key);
        }
    }

    fn cover_opened(
        &mut self,
        cart: ContentId,
        size: Result<(u32, u32), String>,
    ) -> Result<(), String> {
        let Some(cover) = self.covers.get_mut(&cart) else {
            return Ok(());
        };
        let (width, height) = match size {
            Ok(size) => size,
            Err(why) => {
                // Its stop screen says the file did not come. A stop screen
                // that fails too leaves the glass dark.
                if !cover.missing {
                    cover
                        .failure
                        .get_or_insert_with(Failure::default)
                        .again(self.frame);
                    cover.missing = true;
                    self.reads.remove(&cover.picture);
                    self.running.push(Running::Fail(cover.picture));
                }
                return Err(why);
            }
        };
        if !cover.missing {
            cover.failure = None;
        }
        cover.size = Some((width, height));
        if !cover.made {
            self.changes.push(PictureChange::Live {
                id: cover.picture,
                width,
                height,
                sharp: true,
            });
            cover.made = true;
        }
        Ok(())
    }

    /// The shell says the game in `picture` stopped for good: a game that
    /// broke its contract, or ran out of its budget.
    pub(crate) fn stopped(&mut self, picture: PictureId) {
        if let Some(game) = self.game_mut(picture) {
            game.stopped = true;
        }
    }

    /// The games running within [`HEARD_M`] of `eye`, each heard at its
    /// screen as last drawn: at most the live budget's, as many as run.
    pub(crate) fn heard<'a>(
        &'a self,
        live: &'a Live,
        eye: DVec3,
    ) -> impl Iterator<Item = Sound> + 'a {
        self.games
            .iter()
            .filter(move |&(&(cabinet, cart), game)| {
                !game.stopped
                    && live.is_open(Source::Game(cabinet, cart))
                    && game.at.distance(eye) <= HEARD_M
            })
            .map(|(_, game)| Sound {
                source: SoundSource::Game(game.picture),
                position: game.at,
                reach_m: HEARD_M,
                volume: 1.0,
            })
    }

    fn game_mut(&mut self, picture: PictureId) -> Option<&mut Game> {
        let key = self.pictures.get(&picture)?;
        self.games.get_mut(key)
    }

    /// Wants the game `key` running this frame: someone plays it, and asks
    /// every frame they do. It opens in the live budget, and its frames
    /// take the cover's place once made; asked no more, its player left
    /// it, and it waits for them ([`GAME_WAITS_S`]); asked again meanwhile, the
    /// player sat again.
    pub(crate) fn play(&mut self, key: GameId) {
        self.playing.insert(key, self.frame);
    }

    /// The player went away from the world, the page hidden or its window
    /// left: every game played is left at once, each waiting for them
    /// [`GAME_WAITS_AWAY_S`]. Called as it happens, a frame drawn after or
    /// not, with whether a cabinet still runs a game ([`Carts::end_frame`]).
    pub(crate) fn away(&mut self, live: &mut Live, runs: impl Fn(GameId) -> bool) {
        let left: Vec<GameId> = self.playing.drain().map(|(key, _)| key).collect();
        for key in left {
            self.leave(key, GAME_WAITS_AWAY_S, live, runs(key));
        }
    }

    /// The player of the game `key` left it: a game running waits for them
    /// `for_s` seconds and, played by one, is held meanwhile; one the shell
    /// has not opened yet, or whose cabinet no longer `runs` it, has
    /// nothing to come back to, and stops.
    fn leave(&mut self, key: GameId, for_s: f64, live: &mut Live, runs: bool) {
        let source = Source::Game(key.0, key.1);
        let Some(game) = self.games.get_mut(&key) else {
            return;
        };
        if game.stopped || game.left.is_some() || !live.is_open(source) {
            return;
        }
        if !runs || !live.is_playing(source) {
            live.closed(source);
            self.reads.remove(&game.picture);
            self.running.push(Running::Close(game.picture));
            return;
        }
        game.left = Some(Left {
            at_s: self.now_s,
            for_s,
        });
        live.hold(source);
        self.waiting.push(key);
        // A game of one player holds while it waits; a game fed runs as it
        // is fed.
        if game.fed.is_none() {
            self.running.push(Running::Hold(game.picture));
        }
    }

    /// Whether the game `key` waits for the player who left it: sitting
    /// again takes it back where it was.
    pub(crate) fn waits(&self, key: GameId) -> bool {
        self.games.get(&key).is_some_and(|game| game.left.is_some())
    }

    /// The games waiting for a player who left whose screen was in view
    /// the frame just closed: each cabinet, where its screen stands, and
    /// the seconds left of its window. A mark never stands where no screen
    /// shows.
    pub(crate) fn waiting(&self) -> impl Iterator<Item = (u64, DVec3, f64, f64)> + '_ {
        let closed = self.frame.checked_sub(1);
        self.waiting.iter().filter_map(move |key| {
            let game = self
                .games
                .get(key)
                .filter(|game| Some(game.seen) == closed)?;
            let left = game.left?;
            let remains_s = (left.at_s + left.for_s - self.now_s).max(0.0);
            Some((key.0, game.at, remains_s, left.for_s))
        })
    }

    /// Whether the game `key` may be played: it did not stop for good, and
    /// its file came the last time it was asked for, as its cover or to
    /// run. A game stopped is played no more until it is let go, out of
    /// view a while.
    pub(crate) fn playable(&self, key: GameId) -> bool {
        !self.games.get(&key).is_some_and(|game| game.stopped)
            && self
                .covers
                .get(&key.1)
                .is_none_or(|cover| cover.failure.is_none())
    }

    /// The picture the cabinet of the game `key` shows, the one the shell
    /// holds ([`Carts::held`]), and its size in texels: what the renderer
    /// lays over the view while it is played, its cover until its first
    /// frame is made.
    pub(crate) fn picture_of(&self, key: GameId, live: &Live) -> Option<(PictureId, (u32, u32))> {
        let picture = self.held(key, live)?;
        let size = match self.covered.get(&picture) {
            Some(cart) => self.covers.get(cart)?.size,
            None => self.games.get(&key)?.size,
        };
        Some((picture, size?))
    }

    /// The picture whose screen the shell holds for the game `key`: its
    /// own while it runs or says why it stopped, else its cartridge's
    /// cover, once made.
    fn held(&self, key: GameId, live: &Live) -> Option<PictureId> {
        let game = self.games.get(&key)?;
        if game.made && (game.stopped || live.is_open(Source::Game(key.0, key.1))) {
            return Some(game.picture);
        }
        let cover = self.covers.get(&key.1).filter(|cover| cover.made)?;
        Some(cover.picture)
    }

    /// Asks the shell for the screen of the game `key`, or, with none, of
    /// the game played, with its pixels when `indices`: its frame, its
    /// cover, or why it stopped. Refused for a screen not shown here.
    pub(crate) fn read(
        &mut self,
        key: Option<GameId>,
        indices: bool,
        live: &Live,
    ) -> Result<(), &'static str> {
        let key = key.or(self.played).ok_or("no game is played")?;
        let picture = self
            .held(key, live)
            .ok_or("that cabinet's screen is not shown here")?;
        let asked = self.reads.entry(picture).or_insert_with(|| {
            self.running.push(Running::Read(picture));
            Vec::new()
        });
        match asked.iter_mut().find(|(cabinet, _)| *cabinet == key.0) {
            Some((_, with)) => *with |= indices,
            None => asked.push((key.0, indices)),
        }
        Ok(())
    }

    /// The shell read the screen in `picture`: what it is, for each cabinet
    /// that asked for it.
    pub(crate) fn was_read(&mut self, picture: PictureId) -> Vec<Read> {
        let asked = self.reads.remove(&picture).unwrap_or_default();
        let size = match self.covered.get(&picture) {
            Some(cart) => self.covers.get(cart).and_then(|cover| cover.size),
            None => self
                .pictures
                .get(&picture)
                .and_then(|key| self.games.get(key))
                .and_then(|game| game.size),
        };
        let Some((width, height)) = size else {
            return Vec::new();
        };
        asked
            .into_iter()
            .map(|(cabinet, indices)| Read {
                cabinet,
                width,
                height,
                indices,
            })
            .collect()
    }

    /// The cabinet whose game is played here, and what each of its
    /// stations holds on it, as its game was last told: a fed game's every
    /// pad as its last update was given, another's this client's at player
    /// 1's and nothing at the others.
    pub(crate) fn pads_played(&self) -> Option<(PartId, [Pad; arcade::PADS])> {
        let key = self.played?;
        let game = self.games.get(&key)?;
        let pads = match game.fed {
            Some(fed) => core::array::from_fn(|pad| Pad::of_pad(fed.last, pad)),
            None => core::array::from_fn(|pad| if pad == 0 { game.held } else { Pad::NONE }),
        };
        Some((PartId(key.0), pads))
    }

    /// Holds `pad` down on the game `key` this frame. What nobody presses
    /// in a frame is let go at its end.
    pub(crate) fn press(&mut self, key: GameId, pad: Pad) {
        let held = self.pressed.entry(key).or_default();
        *held = *held | pad;
    }

    /// Closes a frame `dt` seconds after the last: the games among the
    /// live pictures' `orders` are run and stopped, a game its player left
    /// waits for them, one waiting past its window or whose cabinet no
    /// longer `runs` it stops, then one they sat at again goes on, what is
    /// pressed on each is said when it changed, and a game's last frame, or
    /// a cover, is let go once out of view a while. `runs` says whether a
    /// cabinet is still in the world with the same cartridge: a game whose
    /// cabinet went or was given another can be taken back into nothing.
    /// Called once every frame after every draw, with the orders of the
    /// live pictures' frame.
    pub(crate) fn end_frame(
        &mut self,
        orders: &[Order],
        live: &mut Live,
        dt: f64,
        runs: impl Fn(GameId) -> bool,
    ) {
        self.now_s += dt.max(0.0);
        for order in orders {
            match *order {
                Order::Open(source @ Source::Game(cabinet, cart)) => {
                    let Some(game) = self.games.get_mut(&(cabinet, cart)) else {
                        live.closed(source);
                        continue;
                    };
                    game.held = Pad::NONE;
                    // A game of several pads is fed: its players' machines
                    // agree on each update before it runs.
                    let plays = self.covers.get(&cart).and_then(|cover| cover.plays);
                    game.fed = plays.is_some_and(|plays| plays > 1).then(Fed::default);
                    self.running.push(Running::Open {
                        picture: game.picture,
                        cart: part::Cabinet::cart_name(cart),
                        seed: game.seed,
                        fed: game.fed.is_some(),
                    });
                }
                Order::Close(Source::Game(cabinet, cart)) => {
                    let Some(game) = self.games.get_mut(&(cabinet, cart)) else {
                        continue;
                    };
                    // A game waiting gave its place: it waits no more.
                    game.left = None;
                    game.fed = None;
                    // A game stopped for good keeps its screen with the
                    // shell, saying why, until it is let go.
                    if !game.stopped {
                        // Stopped, a read asked for is never answered.
                        self.reads.remove(&game.picture);
                        self.running.push(Running::Close(game.picture));
                    }
                }
                Order::Open(Source::Moving(_)) | Order::Close(Source::Moving(_)) => {}
            }
        }
        for key in core::mem::take(&mut self.restarts) {
            let source = Source::Game(key.0, key.1);
            let Some(game) = self.games.get_mut(&key) else {
                continue;
            };
            if game.fed.take().is_some() && live.is_open(source) {
                game.left = None;
                live.closed(source);
                self.reads.remove(&game.picture);
                self.running.push(Running::Close(game.picture));
            }
        }
        let frame = self.frame;
        // A game asked last frame and not in this one, its player left.
        let mut left = Vec::new();
        self.playing.retain(|&key, asked| {
            let kept = *asked == frame;
            if !kept {
                left.push(key);
            }
            kept
        });
        for key in left {
            self.leave(key, GAME_WAITS_S, live, runs(key));
        }
        // The window ends before a player sits again: the first frame back
        // from a hidden page carries all the time it hid.
        let (games, running, reads, now_s) = (
            &mut self.games,
            &mut self.running,
            &mut self.reads,
            self.now_s,
        );
        self.waiting.retain(|&key| {
            let Some(game) = games.get_mut(&key) else {
                return false;
            };
            let Some(left) = game.left else {
                return false;
            };
            if now_s - left.at_s <= left.for_s && runs(key) {
                return true;
            }
            // It stops as a game nobody plays does.
            game.left = None;
            live.closed(Source::Game(key.0, key.1));
            reads.remove(&game.picture);
            running.push(Running::Close(game.picture));
            false
        });
        // Asked again while it waits, its player sat again: wanted from
        // now, so no game wanted meanwhile takes its place.
        for &key in self.playing.keys() {
            if let Some(game) = self.games.get_mut(&key)
                && game.left.take().is_some()
            {
                live.resume(Source::Game(key.0, key.1));
                if game.fed.is_none() {
                    self.running.push(Running::Resume(game.picture));
                }
            }
        }
        // What is pressed now, and what was held and is let go.
        let pressed = core::mem::take(&mut self.pressed);
        self.played = pressed.keys().min().copied();
        let holding = core::mem::take(&mut self.holding);
        let pads = pressed.iter().map(|(&key, &pad)| (key, pad)).chain(
            holding
                .into_iter()
                .filter(|key| !pressed.contains_key(key))
                .map(|key| (key, Pad::NONE)),
        );
        for (key, pad) in pads {
            // A game fed is given every pad's buttons, never pressed.
            let Some(game) = self.games.get_mut(&key).filter(|game| game.fed.is_none()) else {
                continue;
            };
            if pad != game.held && live.is_open(Source::Game(key.0, key.1)) {
                game.held = pad;
                self.running.push(Running::Press {
                    picture: game.picture,
                    pad,
                });
            }
            if game.held != Pad::NONE {
                self.holding.push(key);
            }
        }
        // A game in view last frame and not in this one left it now.
        for key in core::mem::replace(&mut self.shown, core::mem::take(&mut self.drawn)) {
            if let Some(game) = self.games.get(&key)
                && game.seen != frame
            {
                self.unseen.push_back((game.seen, key));
            }
        }
        while let Some(&(seen, key)) = self.unseen.front()
            && frame - seen > SHOWN_UNSEEN_FRAMES
        {
            self.unseen.pop_front();
            // Seen again since, it left again later and waits there.
            let Some(game) = self.games.get(&key).filter(|game| game.seen == seen) else {
                continue;
            };
            if live.is_open(Source::Game(key.0, key.1)) {
                self.unseen.push_back((frame, key));
                continue;
            }
            if game.stopped {
                self.running.push(Running::Close(game.picture));
            }
            self.pictures.remove(&game.picture);
            self.reads.remove(&game.picture);
            if game.made {
                self.changes.push(PictureChange::Remove(game.picture));
            }
            self.games.remove(&key);
        }
        let (running, changes, covered, reads) = (
            &mut self.running,
            &mut self.changes,
            &mut self.covered,
            &mut self.reads,
        );
        self.covers.retain(|_, cover| {
            let kept = frame - cover.seen <= SHOWN_UNSEEN_FRAMES;
            if !kept {
                running.push(Running::Close(cover.picture));
                covered.remove(&cover.picture);
                reads.remove(&cover.picture);
                if cover.made {
                    changes.push(PictureChange::Remove(cover.picture));
                }
            }
            kept
        });
        self.frame += 1;
    }

    /// Stops every game and lets go of every screen: the world they stood
    /// in is gone.
    pub(crate) fn clear(&mut self, live: &Live) {
        self.pictures.clear();
        self.covered.clear();
        self.reads.clear();
        self.played = None;
        self.playing.clear();
        self.holding.clear();
        self.drawn.clear();
        self.shown.clear();
        self.unseen.clear();
        self.waiting.clear();
        self.restarts.clear();
        for ((cabinet, cart), game) in self.games.drain() {
            if game.stopped || live.is_open(Source::Game(cabinet, cart)) {
                self.running.push(Running::Close(game.picture));
            }
            if game.made {
                self.changes.push(PictureChange::Remove(game.picture));
            }
        }
        for (_, cover) in self.covers.drain() {
            self.running.push(Running::Close(cover.picture));
            if cover.made {
                self.changes.push(PictureChange::Remove(cover.picture));
            }
        }
    }

    pub fn drain_changes(&mut self) -> Vec<PictureChange> {
        core::mem::take(&mut self.changes)
    }

    /// What the shell runs and stops, in the order the client decided.
    pub fn drain_running(&mut self) -> Vec<Running> {
        core::mem::take(&mut self.running)
    }
}

/// The seed a cabinet's game starts from: its name in the world, so every
/// client starts the same game the same way. A game of one player then runs
/// by its own clock on each client, alike and never in step; a game fed
/// runs the same buttons on each of its players' machines, and so the
/// same frames.
fn seed(cabinet: u64) -> u32 {
    (cabinet ^ cabinet >> 32) as u32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::live::{GIVE_UP_AFTER, ONE_KIND_AT_MOST, RETRY_FRAMES, UNWANTED_FRAMES};
    use glam::{DQuat, DVec3};

    #[test]
    fn a_screen_read_is_one_digit_of_base_32_a_pixel() {
        let read = Read {
            cabinet: 9,
            width: 160,
            height: 120,
            indices: true,
        };
        // Colour n is [8 n, 0, 0]; pixel i is i, its high bits past 32.
        let palette: Vec<[u8; 4]> = (0..32).map(|n| [8 * n, 0, 0, 255]).collect();
        let pixels: Vec<u8> = (0..160 * 120).map(|i| i as u8).collect();
        let Event::Screen {
            palette, indices, ..
        } = read.said(&pixels, &palette)
        else {
            unreachable!("a read is said as a screen")
        };
        assert_eq!(palette[31], "#f80000");
        let indices = indices.unwrap();
        assert_eq!(&indices[..34], "0123456789abcdefghijklmnopqrstuv01");
        // 255 is said by its low five bits, as `rgba` colours it.
        assert_eq!(indices.as_bytes()[255], b'v');
    }
    use scene::Camera;

    /// The games as the client holds them, with the live pictures' budget.
    #[derive(Default)]
    struct Held {
        carts: Carts,
        live: Live,
        /// The games whose cabinet went, or was given another cartridge.
        gone: Vec<GameId>,
    }

    impl Held {
        /// The game on the cabinet `cabinet`, named so by the world too.
        fn draw(
            &mut self,
            cabinet: u64,
            cart: ContentId,
            world_from_quad: DAffine3,
            screen: &Screen,
        ) -> Option<PictureDraw> {
            let names = (cabinet, cabinet);
            self.carts
                .draw(&mut self.live, names, cart, world_from_quad, screen)
        }

        /// The same, played: asked to run this frame.
        fn play(
            &mut self,
            cabinet: u64,
            cart: ContentId,
            world_from_quad: DAffine3,
            screen: &Screen,
        ) -> Option<PictureDraw> {
            self.carts.play((cabinet, cart));
            self.draw(cabinet, cart, world_from_quad, screen)
        }

        fn end_frame(&mut self) {
            self.end_frame_after(FRAME_S);
        }

        fn end_frame_after(&mut self, dt: f64) {
            let orders = self.live.end_frame();
            let gone = &self.gone;
            self.carts
                .end_frame(&orders, &mut self.live, dt, |key| !gone.contains(&key));
        }

        /// A game played, its first frame made, its cover's too: what each
        /// is shown by.
        fn running(&mut self, cabinet: u64, cart: ContentId) -> (PictureId, PictureId) {
            self.play(cabinet, cart, at(3.0), &screen());
            self.end_frame();
            let running = self.drain_running();
            let picture = opened(&running)[0];
            if let Some(&cover) = covered(&running).first() {
                self.opened(cover, Ok((160, 120))).unwrap();
            }
            self.opened(picture, Ok((160, 120))).unwrap();
            self.drain_changes();
            let cover = self.covers[&cart].picture;
            (picture, cover)
        }

        fn opened(
            &mut self,
            picture: PictureId,
            size: Result<(u32, u32), String>,
        ) -> Result<(), String> {
            self.carts.opened(picture, size, &mut self.live)
        }
    }

    impl core::ops::Deref for Held {
        type Target = Carts;
        fn deref(&self) -> &Carts {
            &self.carts
        }
    }

    impl core::ops::DerefMut for Held {
        fn deref_mut(&mut self) -> &mut Carts {
            &mut self.carts
        }
    }

    /// A frame's seconds: exact in binary, so a window ends on a known
    /// frame.
    const FRAME_S: f64 = 1.0 / 64.0;

    /// The frames a game waits for its player, the last one included.
    fn waits_frames(window_s: f64) -> u32 {
        (window_s / FRAME_S) as u32
    }

    fn screen() -> Screen {
        Screen {
            camera: Camera {
                position: DVec3::ZERO,
                rotation: DQuat::IDENTITY,
                fov_y: 1.0,
                near: 0.1,
            },
            aspect: 1.5,
            height_px: 1000.0,
            play_m: 30.0,
        }
    }

    /// A screen half a metre tall, `away_m` in front of the eye.
    fn at(away_m: f64) -> DAffine3 {
        DAffine3::from_scale_rotation_translation(
            DVec3::new(0.66, 0.5, 1.0),
            DQuat::IDENTITY,
            DVec3::new(0.0, 0.0, -away_m),
        )
    }

    fn cart(n: u8) -> ContentId {
        ContentId([n; 32])
    }

    fn opened(running: &[Running]) -> Vec<PictureId> {
        running
            .iter()
            .filter_map(|run| match run {
                Running::Open { picture, .. } => Some(*picture),
                Running::Close(_)
                | Running::Cover { .. }
                | Running::Press { .. }
                | Running::Read(_)
                | Running::Fail(_)
                | Running::Hold(_)
                | Running::Resume(_)
                | Running::Feed { .. } => None,
            })
            .collect()
    }

    fn covered(running: &[Running]) -> Vec<PictureId> {
        running
            .iter()
            .filter_map(|run| match run {
                Running::Cover { picture, .. } => Some(*picture),
                _ => None,
            })
            .collect()
    }

    fn shown(drawn: Option<PictureDraw>) -> Option<PictureId> {
        drawn.and_then(|draw| draw.picture)
    }

    #[test]
    fn a_game_nobody_plays_shows_its_cover_once_and_runs_nothing() {
        let mut carts = Held::default();
        assert!(carts.draw(7, cart(1), at(3.0), &screen()).is_none());
        carts.end_frame();
        let running = carts.drain_running();
        let [
            Running::Cover {
                picture,
                cart: file,
                name,
            },
        ] = running.as_slice()
        else {
            panic!("{running:?}");
        };
        assert_eq!(*file, part::Cabinet::cart_name(cart(1)));
        assert_eq!(*name, cart(1).hex());
        // Answered outside the live budget: its texture is made and shown.
        carts.opened(*picture, Ok((160, 120))).unwrap();
        assert!(matches!(
            carts.drain_changes().as_slice(),
            [PictureChange::Live { id, width: 160, height: 120, sharp: true }] if id == picture
        ));
        for _ in 0..UNWANTED_FRAMES * 2 {
            assert_eq!(
                shown(carts.draw(7, cart(1), at(3.0), &screen())),
                Some(*picture)
            );
            carts.end_frame();
        }
        assert!(carts.drain_running().is_empty(), "asked once, never run");
        assert!(!carts.live.is_open(Source::Game(7, cart(1))));
        // An agent reads the cover as it reads a game.
        assert_eq!(
            carts.carts.read(Some((7, cart(1))), false, &carts.live),
            Ok(())
        );
        assert_eq!(carts.drain_running(), [Running::Read(*picture)]);
    }

    #[test]
    fn every_cabinet_of_one_cartridge_shows_one_cover() {
        let mut carts = Held::default();
        for cabinet in 1..=3 {
            carts.draw(cabinet, cart(1), at(2.0 + cabinet as f64), &screen());
        }
        carts.draw(4, cart(2), at(3.0), &screen());
        carts.end_frame();
        let covers = covered(&carts.drain_running());
        assert_eq!(covers.len(), 2, "one a cartridge");
        for &picture in &covers {
            carts.opened(picture, Ok((160, 120))).unwrap();
        }
        assert_eq!(carts.drain_changes().len(), 2, "one texture a cartridge");
        let pictures: Vec<_> = (1..=3)
            .map(|cabinet| shown(carts.draw(cabinet, cart(1), at(2.0 + cabinet as f64), &screen())))
            .collect();
        assert_eq!(pictures, [Some(covers[0]); 3]);
        assert_eq!(
            shown(carts.draw(4, cart(2), at(3.0), &screen())),
            Some(covers[1])
        );
        carts.end_frame();
        // Two cabinets read it: the shell reads it once, said for each.
        for cabinet in [1, 2, 1] {
            carts
                .carts
                .read(Some((cabinet, cart(1))), cabinet == 2, &carts.live)
                .unwrap();
        }
        assert_eq!(carts.drain_running(), [Running::Read(covers[0])]);
        let reads = carts.carts.was_read(covers[0]);
        let said: Vec<_> = reads
            .iter()
            .map(|read| (read.cabinet, read.indices))
            .collect();
        assert_eq!(said, [(1, false), (2, true)]);
        assert!(carts.carts.was_read(covers[0]).is_empty(), "answered once");
    }

    #[test]
    fn a_cover_whose_file_did_not_come_says_so_and_is_asked_again() {
        let mut carts = Held::default();
        carts.draw(1, cart(1), at(3.0), &screen());
        carts.end_frame();
        let picture = covered(&carts.drain_running())[0];
        assert!(carts.opened(picture, Err("404".into())).is_err());
        assert_eq!(carts.drain_running(), [Running::Fail(picture)]);
        assert!(!carts.playable((1, cart(1))), "no Play offered for it");
        // Its stop screen shows while it waits.
        carts.opened(picture, Ok((160, 120))).unwrap();
        assert_eq!(carts.drain_changes().len(), 1);
        for _ in 0..RETRY_FRAMES {
            assert_eq!(
                shown(carts.draw(1, cart(1), at(3.0), &screen())),
                Some(picture)
            );
            carts.end_frame();
        }
        assert!(carts.drain_running().is_empty());
        carts.draw(1, cart(1), at(3.0), &screen());
        carts.end_frame();
        let running = carts.drain_running();
        assert_eq!(running[0], Running::Close(picture));
        assert_eq!(covered(&running), [picture]);
        // It comes this time: its texture stays, and the game plays.
        carts.opened(picture, Ok((160, 120))).unwrap();
        assert!(carts.drain_changes().is_empty());
        assert!(carts.playable((1, cart(1))));
    }

    #[test]
    fn a_game_played_shows_its_cover_until_its_own_frames_are_made() {
        let mut carts = Held::default();
        carts.draw(7, cart(1), at(3.0), &screen());
        carts.end_frame();
        let cover = covered(&carts.drain_running())[0];
        carts.opened(cover, Ok((160, 120))).unwrap();
        carts.drain_changes();
        assert_eq!(
            shown(carts.play(7, cart(1), at(3.0), &screen())),
            Some(cover)
        );
        carts.end_frame();
        let running = carts.drain_running();
        let [
            Running::Open {
                picture, seed: 7, ..
            },
        ] = running.as_slice()
        else {
            panic!("{running:?}");
        };
        assert_ne!(*picture, cover, "a texture of its cabinet's");
        assert_eq!(
            shown(carts.play(7, cart(1), at(3.0), &screen())),
            Some(cover)
        );
        let laid = |carts: &Held| carts.carts.picture_of((7, cart(1)), &carts.live);
        assert_eq!(laid(&carts), Some((cover, (160, 120))), "the cover is laid");
        carts.end_frame();
        carts.opened(*picture, Ok((160, 120))).unwrap();
        assert_eq!(carts.drain_changes().len(), 1);
        assert_eq!(
            shown(carts.play(7, cart(1), at(3.0), &screen())),
            Some(*picture)
        );
        assert_eq!(laid(&carts), Some((*picture, (160, 120))));
    }

    #[test]
    fn a_game_stopped_says_why_while_seen_and_plays_again_once_let_go() {
        let mut carts = Held::default();
        carts.play(1, cart(1), at(3.0), &screen());
        carts.end_frame();
        let running = carts.drain_running();
        let (picture, cover) = (opened(&running)[0], covered(&running)[0]);
        carts.opened(cover, Ok((160, 120))).unwrap();
        carts.opened(picture, Ok((160, 120))).unwrap();
        carts.stopped(picture);
        assert!(!carts.playable((1, cart(1))));
        // Played no more, it closes in the live budget, and its stop screen
        // stays with the shell, shown and read, no cover over it.
        for _ in 0..=UNWANTED_FRAMES * 3 {
            assert_eq!(
                shown(carts.draw(1, cart(1), at(3.0), &screen())),
                Some(picture)
            );
            carts.end_frame();
        }
        assert!(!carts.live.is_open(Source::Game(1, cart(1))));
        assert!(carts.drain_running().is_empty());
        assert_eq!(
            carts.carts.read(Some((1, cart(1))), false, &carts.live),
            Ok(())
        );
        assert_eq!(carts.drain_running(), [Running::Read(picture)]);
        // Out of view a while, it is let go, and may be played again.
        for _ in 0..=SHOWN_UNSEEN_FRAMES + 1 {
            carts.end_frame();
        }
        assert_eq!(
            carts.drain_running(),
            [Running::Close(picture), Running::Close(cover)]
        );
        assert!(carts.playable((1, cart(1))));
    }

    #[test]
    fn a_game_played_runs_from_its_cabinets_name_and_shows_once_answered() {
        let mut carts = Held::default();
        assert!(carts.play(7, cart(1), at(3.0), &screen()).is_none());
        carts.end_frame();
        let running = carts.drain_running();
        let [
            Running::Cover { .. },
            Running::Open {
                picture,
                cart,
                seed,
                fed: false,
            },
        ] = running.as_slice()
        else {
            panic!("{running:?}");
        };
        assert_eq!(*seed, 7);
        assert!(cart.ends_with(".wasm"));
        carts.opened(*picture, Ok((160, 120))).unwrap();
        let changes = carts.drain_changes();
        assert!(matches!(
            changes.as_slice(),
            [PictureChange::Live { id, width: 160, height: 120, sharp: true }] if id == picture
        ));
        let drawn = carts.play(7, ContentId([1; 32]), at(3.0), &screen());
        assert_eq!(shown(drawn), Some(*picture));
    }

    #[test]
    fn a_game_out_of_view_or_past_the_play_distance_does_not_run_or_show() {
        let mut carts = Held::default();
        let behind = DAffine3::from_translation(DVec3::new(0.0, 0.0, 5.0));
        carts.play(1, cart(1), behind, &screen());
        carts.play(2, cart(1), at(60.0), &screen());
        carts.end_frame();
        assert!(carts.drain_running().is_empty());
    }

    #[test]
    fn a_few_run_at_once_the_tallest_first_leaving_moving_pictures_a_place() {
        let mut carts = Held::default();
        for n in 0..6 {
            carts.play(n, cart(1), at(2.0 + n as f64), &screen());
        }
        carts.end_frame();
        let running = carts.drain_running();
        assert_eq!(opened(&running).len(), ONE_KIND_AT_MOST);
        let seeds: Vec<u32> = running
            .iter()
            .filter_map(|run| match run {
                Running::Open { seed, .. } => Some(*seed),
                _ => None,
            })
            .collect();
        assert_eq!(seeds, [0, 1, 2]);
    }

    #[test]
    fn a_game_played_turned_from_or_left_behind_stops_and_keeps_its_last_frame_in_view() {
        let mut carts = Held::default();
        carts.play(1, cart(1), at(3.0), &screen());
        carts.end_frame();
        let running = carts.drain_running();
        let (picture, cover) = (opened(&running)[0], covered(&running)[0]);
        carts.opened(picture, Ok((160, 120))).unwrap();
        carts.drain_changes();
        let behind = DAffine3::from_translation(DVec3::new(0.0, 0.0, 5.0));
        for _ in 0..=UNWANTED_FRAMES {
            carts.play(1, cart(1), behind, &screen());
            carts.end_frame();
        }
        assert_eq!(carts.drain_running(), [Running::Close(picture)]);
        // Past the play distance it is not run again, and its last frame
        // shows for as long as it is in view; the cover, seen nowhere, goes.
        for _ in 0..=SHOWN_UNSEEN_FRAMES {
            let drawn = carts.play(1, cart(1), at(60.0), &screen());
            assert_eq!(shown(drawn), Some(picture));
            carts.end_frame();
        }
        assert_eq!(carts.drain_running(), [Running::Close(cover)]);
        assert!(carts.drain_changes().is_empty());
    }

    #[test]
    fn a_game_of_two_pads_runs_only_what_it_is_fed_and_is_never_pressed_or_held() {
        let mut carts = Held::default();
        let game = (7, cart(2));
        // Its cover read first: it plays two.
        carts.draw(7, cart(2), at(3.0), &screen());
        carts.end_frame();
        let cover = covered(&carts.drain_running())[0];
        carts.plays_read(cover, 2);
        carts.opened(cover, Ok((160, 120))).unwrap();
        assert_eq!(carts.plays(game), Some(2));
        carts.play(7, cart(2), at(3.0), &screen());
        carts.end_frame();
        let running = carts.drain_running();
        let [
            Running::Open {
                picture, fed: true, ..
            },
        ] = running.as_slice()
        else {
            panic!("{running:?}");
        };
        let picture = *picture;
        // Nothing is given before the shell answers.
        let Held {
            carts: fed, live, ..
        } = &mut carts;
        assert_eq!(fed.given(game, live), None);
        fed.feed(game, &[1, 2], false, live);
        assert!(carts.drain_running().is_empty());
        carts.opened(picture, Ok((160, 120))).unwrap();
        let Held {
            carts: fed, live, ..
        } = &mut carts;
        assert_eq!(fed.given(game, live), Some(0));
        fed.feed(game, &[1, 0x100], false, live);
        fed.feed(game, &[0x101; 3], true, live);
        assert_eq!(fed.given(game, live), Some(5));
        // Pressed, it is not: its buttons are what it is fed.
        carts.press(game, Pad::LEFT);
        carts.play(7, cart(2), at(3.0), &screen());
        carts.end_frame();
        assert_eq!(
            carts.drain_running(),
            [
                Running::Feed {
                    picture,
                    buttons: vec![1, 0x100],
                    at_once: false
                },
                Running::Feed {
                    picture,
                    buttons: vec![0x101; 3],
                    at_once: true
                }
            ]
        );
        // Left, it waits and is not held: it runs as it is fed.
        carts.draw(7, cart(2), at(3.0), &screen());
        carts.end_frame();
        assert!(carts.waits(game));
        let Held {
            carts: fed, live, ..
        } = &mut carts;
        fed.feed(game, &[0x100], false, live);
        assert_eq!(fed.given(game, live), Some(6));
        carts.play(7, cart(2), at(3.0), &screen());
        carts.end_frame();
        assert!(!carts.waits(game));
        assert_eq!(
            carts.drain_running(),
            [Running::Feed {
                picture,
                buttons: vec![0x100],
                at_once: false
            }]
        );
        // Run again from its start: it stops, and opens again fed, given
        // nothing.
        carts.restart(game);
        carts.play(7, cart(2), at(3.0), &screen());
        carts.end_frame();
        assert_eq!(carts.drain_running(), [Running::Close(picture)]);
        assert_eq!(carts.given(game, &carts.live), None);
        carts.play(7, cart(2), at(3.0), &screen());
        carts.end_frame();
        let running = carts.drain_running();
        assert!(
            matches!(running.as_slice(), [Running::Open { fed: true, .. }]),
            "{running:?}"
        );
        carts.opened(picture, Ok((160, 120))).unwrap();
        assert_eq!(carts.given(game, &carts.live), Some(0));
    }

    #[test]
    fn a_game_left_holds_at_once_and_goes_on_where_it_was_when_played_again_in_its_window() {
        let mut carts = Held::default();
        let (picture, _) = carts.running(1, cart(1));
        let game = (1, cart(1));
        carts.press(game, Pad::LEFT);
        carts.play(1, cart(1), at(3.0), &screen());
        carts.end_frame();
        assert_eq!(
            carts.drain_running(),
            [Running::Press {
                picture,
                pad: Pad::LEFT
            }]
        );
        // Left: held the frame it is asked no more, its buttons let go
        // while it holds, and its last frame shown, open in the budget.
        assert_eq!(
            shown(carts.draw(1, cart(1), at(3.0), &screen())),
            Some(picture)
        );
        carts.end_frame();
        assert_eq!(
            carts.drain_running(),
            [
                Running::Hold(picture),
                Running::Press {
                    picture,
                    pad: Pad::NONE
                }
            ]
        );
        assert!(carts.waits(game));
        for _ in 1..waits_frames(GAME_WAITS_S) {
            assert_eq!(
                shown(carts.draw(1, cart(1), at(3.0), &screen())),
                Some(picture)
            );
            carts.end_frame();
        }
        assert!(carts.drain_running().is_empty(), "nothing runs or stops");
        assert!(carts.live.is_open(Source::Game(1, cart(1))));
        // Played again in the last frame of its window: it goes on, opened
        // once, its buttons pressed again after.
        carts.press(game, Pad::JUMP);
        carts.play(1, cart(1), at(3.0), &screen());
        carts.end_frame();
        assert_eq!(
            carts.drain_running(),
            [
                Running::Resume(picture),
                Running::Press {
                    picture,
                    pad: Pad::JUMP
                }
            ]
        );
        assert!(!carts.waits(game));
    }

    #[test]
    fn a_game_left_stops_past_its_window_its_cover_shows_again_then_is_let_go_unseen() {
        let mut carts = Held::default();
        let (picture, cover) = carts.running(1, cart(1));
        carts.end_frame();
        assert_eq!(carts.drain_running(), [Running::Hold(picture)]);
        // In view all along, as one who steps back from it.
        for _ in 0..waits_frames(GAME_WAITS_S) {
            carts.draw(1, cart(1), at(3.0), &screen());
            carts.end_frame();
        }
        assert!(carts.drain_running().is_empty(), "it waits to the end");
        carts.draw(1, cart(1), at(3.0), &screen());
        carts.end_frame();
        assert_eq!(carts.drain_running(), [Running::Close(picture)]);
        assert!(!carts.waits((1, cart(1))));
        assert!(!carts.live.is_open(Source::Game(1, cart(1))));
        // Seen again, its cover shows, and its own texture goes.
        assert_eq!(
            shown(carts.draw(1, cart(1), at(3.0), &screen())),
            Some(cover)
        );
        carts.end_frame();
        assert!(carts.drain_running().is_empty(), "the cover is asked once");
        assert!(matches!(
            carts.drain_changes().as_slice(),
            [PictureChange::Remove(id)] if *id == picture
        ));
        for _ in 0..=SHOWN_UNSEEN_FRAMES {
            carts.end_frame();
        }
        assert_eq!(carts.drain_running(), [Running::Close(cover)]);
        let changes = carts.drain_changes();
        assert!(matches!(changes.as_slice(), [PictureChange::Remove(id)] if *id == cover));
        assert!(carts.games.is_empty() && carts.covers.is_empty() && carts.covered.is_empty());
    }

    #[test]
    fn a_game_left_before_the_shell_opened_it_stops_at_once() {
        let mut carts = Held::default();
        carts.play(1, cart(1), at(3.0), &screen());
        carts.end_frame();
        let picture = opened(&carts.drain_running())[0];
        carts.end_frame();
        assert_eq!(carts.drain_running(), [Running::Close(picture)]);
        assert!(!carts.waits((1, cart(1))));
        assert!(!carts.live.is_open(Source::Game(1, cart(1))));
    }

    #[test]
    fn a_game_waiting_gives_its_place_to_a_game_wanted_and_waits_no_more() {
        let mut carts = Held::default();
        let (left, _) = carts.running(1, cart(1));
        carts.end_frame();
        assert_eq!(carts.drain_running(), [Running::Hold(left)]);
        // Three more played: the games' places are full, and the one
        // waiting gives its own to the one wanted that lacks it.
        for n in 2..=4 {
            carts.play(n, cart(1), at(2.0 + n as f64), &screen());
        }
        carts.end_frame();
        let running = carts.drain_running();
        assert!(running.contains(&Running::Close(left)), "{running:?}");
        assert_eq!(opened(&running).len(), ONE_KIND_AT_MOST);
        assert!(!carts.waits((1, cart(1))));
        // Played again, it runs from its start.
        carts.play(1, cart(1), at(3.0), &screen());
        carts.end_frame();
        assert!(!carts.drain_running().contains(&Running::Resume(left)));
    }

    #[test]
    fn a_player_gone_away_leaves_every_game_at_once_for_the_away_window() {
        let mut carts = Held::default();
        let (picture, _) = carts.running(1, cart(1));
        carts.play(1, cart(1), at(3.0), &screen());
        carts.end_frame();
        carts.drain_running();
        // Gone between frames: held then, before any frame is drawn.
        carts.carts.away(&mut carts.live, |_| true);
        assert_eq!(carts.drain_running(), [Running::Hold(picture)]);
        for _ in 0..waits_frames(GAME_WAITS_AWAY_S) {
            carts.draw(1, cart(1), at(3.0), &screen());
            carts.end_frame();
        }
        assert!(carts.drain_running().is_empty());
        // A hidden page's time comes at once: past the window, it stops.
        carts.end_frame_after(GAME_WAITS_AWAY_S);
        assert_eq!(carts.drain_running(), [Running::Close(picture)]);
    }

    #[test]
    fn a_game_waiting_says_where_its_screen_is_and_how_long_it_still_waits() {
        let mut carts = Held::default();
        carts.running(1, cart(1));
        carts.end_frame();
        let quarter = waits_frames(GAME_WAITS_S) / 4;
        for _ in 0..quarter {
            carts.draw(1, cart(1), at(3.0), &screen());
            carts.end_frame();
        }
        let waiting: Vec<_> = carts.waiting().collect();
        let [(1, at, remains_s, for_s)] = waiting.as_slice() else {
            panic!("one waits");
        };
        assert_eq!(*at, DVec3::new(0.0, 0.0, -3.0), "the middle of its screen");
        assert_eq!(*for_s, GAME_WAITS_S);
        assert_eq!(*remains_s, GAME_WAITS_S * 0.75);
        // Its screen out of view, it still waits, and no mark stands.
        carts.end_frame();
        assert!(carts.waits((1, cart(1))));
        assert_eq!(carts.waiting().count(), 0);
    }

    #[test]
    fn a_game_whose_cabinet_went_or_was_given_another_cartridge_stops_at_once() {
        let mut carts = Held::default();
        // Waiting as its cabinet goes: it stops the next frame.
        let (waiting, _) = carts.running(1, cart(1));
        carts.end_frame();
        assert_eq!(carts.drain_running(), [Running::Hold(waiting)]);
        carts.gone.push((1, cart(1)));
        carts.end_frame();
        assert_eq!(carts.drain_running(), [Running::Close(waiting)]);
        assert!(!carts.waits((1, cart(1))));
        assert!(!carts.live.is_open(Source::Game(1, cart(1))));
        // Played as its cabinet is given another: it stops, never held.
        let (played, _) = carts.running(2, cart(1));
        carts.play(2, cart(1), at(3.0), &screen());
        carts.end_frame();
        carts.drain_running();
        carts.gone.push((2, cart(1)));
        carts.play(2, cart(2), at(3.0), &screen());
        carts.end_frame();
        let running = carts.drain_running();
        assert!(running.contains(&Running::Close(played)), "{running:?}");
        assert!(!running.contains(&Running::Hold(played)), "{running:?}");
        assert!(!carts.waits((2, cart(1))));
    }

    #[test]
    fn a_game_asked_again_in_the_frame_its_window_ends_stops() {
        let mut carts = Held::default();
        let (picture, _) = carts.running(1, cart(1));
        carts.end_frame();
        assert_eq!(carts.drain_running(), [Running::Hold(picture)]);
        // The first frame back from a page hidden past the window, asked to
        // run by what waited for it.
        carts.play(1, cart(1), at(3.0), &screen());
        carts.end_frame_after(GAME_WAITS_S + FRAME_S);
        assert_eq!(carts.drain_running(), [Running::Close(picture)]);
        assert!(!carts.waits((1, cart(1))));
    }

    #[test]
    fn a_game_played_again_keeps_its_place_while_the_eye_eases_to_it() {
        let mut carts = Held::default();
        let (picture, _) = carts.running(1, cart(1));
        let game = (1, cart(1));
        carts.end_frame();
        assert_eq!(carts.drain_running(), [Running::Hold(picture)]);
        // Sat at again, its screen not yet drawn near: wanted all the same.
        carts.carts.play(game);
        carts.end_frame();
        assert_eq!(carts.drain_running(), [Running::Resume(picture)]);
        // Three more played meanwhile: none takes its place.
        carts.carts.play(game);
        for n in 2..=4 {
            carts.play(n, cart(1), at(2.0 + n as f64), &screen());
        }
        carts.end_frame();
        let running = carts.drain_running();
        assert!(!running.contains(&Running::Close(picture)), "{running:?}");
        assert_eq!(opened(&running).len(), ONE_KIND_AT_MOST - 1);
        assert!(carts.live.is_open(Source::Game(1, cart(1))));
    }

    #[test]
    fn another_player_at_the_cabinet_meanwhile_plays_their_own_game_from_its_start() {
        // Each client runs its own copy: the one who left holds theirs, and
        // one sitting at the same cabinet on another runs from the seed.
        let mut left = Held::default();
        let (picture, _) = left.running(1, cart(1));
        left.end_frame();
        assert_eq!(left.drain_running(), [Running::Hold(picture)]);
        let mut other = Held::default();
        other.play(1, cart(1), at(3.0), &screen());
        other.end_frame();
        let running = other.drain_running();
        assert!(
            running
                .iter()
                .any(|run| matches!(run, Running::Open { seed: 1, .. })),
            "{running:?}"
        );
        assert!(left.waits((1, cart(1))));
    }

    #[test]
    fn a_frame_walks_only_the_games_drawn_pressed_or_held() {
        let mut carts = Held::default();
        for cabinet in 1..=3 {
            carts.play(cabinet, cart(1), at(3.0), &screen());
        }
        carts.end_frame();
        let pictures = opened(&carts.drain_running());
        for &picture in &pictures {
            carts.opened(picture, Ok((160, 120))).unwrap();
        }
        let game = (1, cart(1));
        carts.press(game, Pad::LEFT);
        for cabinet in 1..=3 {
            carts.play(cabinet, cart(1), at(3.0), &screen());
        }
        carts.end_frame();
        assert_eq!(carts.holding, [game]);
        // Nobody presses it: it is let go once, and nothing is held after.
        carts.drain_running();
        carts.end_frame();
        let let_go = carts.drain_running();
        assert!(
            let_go.contains(&Running::Press {
                picture: pictures[0],
                pad: Pad::NONE
            }),
            "{let_go:?}"
        );
        assert!(carts.holding.is_empty());
        // Out of view, every game waits in the queue, and an idle frame
        // walks none of them. Left, they wait for their player, and stop
        // past their window.
        assert!(carts.shown.is_empty() && carts.drawn.is_empty());
        assert_eq!(carts.unseen.len(), 3);
        carts.end_frame_after(GAME_WAITS_S * 2.0);
        assert_eq!(carts.drain_running().len(), 3);
        // The second comes back and leaves again: it waits from then.
        for _ in 0..SHOWN_UNSEEN_FRAMES / 2 {
            carts.end_frame();
        }
        carts.play(2, cart(1), at(3.0), &screen());
        carts.end_frame();
        for _ in 0..SHOWN_UNSEEN_FRAMES / 2 + 2 {
            carts.end_frame();
        }
        let removed = |carts: &mut Held| {
            carts
                .drain_changes()
                .into_iter()
                .filter_map(|change| match change {
                    PictureChange::Remove(id) => Some(id),
                    _ => None,
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(removed(&mut carts), [pictures[0], pictures[2]]);
        assert_eq!(carts.games.len(), 1);
        for _ in 0..SHOWN_UNSEEN_FRAMES / 2 {
            carts.end_frame();
        }
        assert_eq!(removed(&mut carts), [pictures[1]]);
        assert!(carts.games.is_empty() && carts.pictures.is_empty() && carts.unseen.is_empty());
    }

    #[test]
    fn a_game_that_fails_waits_before_it_is_asked_again() {
        let mut carts = Held::default();
        carts.play(1, cart(1), at(3.0), &screen());
        carts.end_frame();
        let picture = opened(&carts.drain_running())[0];
        assert!(carts.opened(picture, Err("gone".into())).is_err());
        for _ in 0..10 {
            carts.play(1, cart(1), at(3.0), &screen());
            carts.end_frame();
        }
        assert!(opened(&carts.drain_running()).is_empty());
        assert!(carts.play(1, cart(1), at(3.0), &screen()).is_none());
    }

    #[test]
    fn a_game_whose_runner_died_runs_again_after_a_while() {
        let mut carts = Held::default();
        carts.play(1, cart(1), at(3.0), &screen());
        carts.end_frame();
        let picture = opened(&carts.drain_running())[0];
        carts.opened(picture, Ok((160, 120))).unwrap();
        assert!(carts.opened(picture, Err("died".into())).is_err());
        for _ in 0..RETRY_FRAMES {
            carts.play(1, cart(1), at(3.0), &screen());
            carts.end_frame();
        }
        assert!(opened(&carts.drain_running()).is_empty());
        carts.play(1, cart(1), at(3.0), &screen());
        carts.end_frame();
        assert_eq!(opened(&carts.drain_running()), [picture]);
    }

    #[test]
    fn a_game_whose_file_never_comes_is_given_up_and_its_screen_says_so() {
        let mut carts = Held::default();
        let mut picture = None;
        let mut cover = None;
        let mut fails = 0;
        while fails < GIVE_UP_AFTER {
            carts.play(1, cart(1), at(3.0), &screen());
            carts.end_frame();
            let running = carts.drain_running();
            cover = cover.or(covered(&running).first().copied());
            if let Some(&open) = opened(&running).first() {
                picture = Some(open);
                assert!(carts.opened(open, Err("404".into())).is_err());
                fails += 1;
            }
        }
        let (picture, cover) = (picture.unwrap(), cover.unwrap());
        // Seen again, it is not asked for: its screen is told to say why.
        carts.play(1, cart(1), at(3.0), &screen());
        carts.end_frame();
        assert_eq!(carts.drain_running(), [Running::Fail(picture)]);
        carts.play(1, cart(1), at(3.0), &screen());
        carts.end_frame();
        assert!(carts.drain_running().is_empty(), "told once");
        // The shell answers as for an open: the screen is made and shown,
        // and nobody plays it.
        carts.opened(picture, Ok((160, 120))).unwrap();
        assert_eq!(carts.drain_changes().len(), 1);
        let drawn = carts.play(1, cart(1), at(3.0), &screen());
        assert_eq!(shown(drawn), Some(picture));
        assert!(!carts.playable((1, cart(1))));
        // Out of view a while, the shell lets it go.
        for _ in 0..=SHOWN_UNSEEN_FRAMES + 1 {
            carts.end_frame();
        }
        assert_eq!(
            carts.drain_running(),
            [Running::Close(picture), Running::Close(cover)]
        );
        assert!(carts.games.is_empty());
    }

    #[test]
    fn every_game_running_within_reach_is_heard_at_its_screen_and_no_other() {
        let mut carts = Held::default();
        // Two played near, one far, one only in view: its cover runs nothing.
        for (cabinet, away_m) in [(1, 3.0), (2, 8.0), (3, HEARD_M + 5.0)] {
            carts.play(cabinet, cart(cabinet as u8), at(away_m), &screen());
        }
        carts.draw(4, cart(4), at(2.0), &screen());
        carts.end_frame();
        let heard = |carts: &Held| {
            let mut heard: Vec<(PictureId, f64)> = carts
                .carts
                .heard(&carts.live, DVec3::ZERO)
                .map(|sound| {
                    let SoundSource::Game(picture) = sound.source else {
                        panic!("{sound:?}")
                    };
                    assert_eq!(sound.reach_m, HEARD_M);
                    (picture, -sound.position.z)
                })
                .collect();
            heard.sort_by(|a, b| a.1.total_cmp(&b.1));
            heard
        };
        let opened = opened(&carts.drain_running());
        assert_eq!(opened.len(), 3);
        let near: Vec<f64> = heard(&carts).iter().map(|&(_, away)| away).collect();
        assert_eq!(near, [3.0, 8.0], "within reach, each at its screen");
        // One that stops for good is silent at once.
        let nearest = heard(&carts)[0].0;
        carts.stopped(nearest);
        assert_eq!(heard(&carts).len(), 1);
    }
}
