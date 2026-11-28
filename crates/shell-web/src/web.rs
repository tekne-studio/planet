use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use client::{Chord, Client, Form, Input, Key, Outbound};
use render::{Gpu, Renderer, View, surface_configuration, wgpu};

use crate::keys;
use crate::start::{Backend, Power, Start, Step};
use crate::{audio, carts, moving, touch};
use wasm_bindgen::prelude::*;
use web_sys::{HtmlCanvasElement, KeyboardEvent, MessageEvent, PointerEvent, WheelEvent};

#[wasm_bindgen(start)]
fn start() {
    console_error_panic_hook::set_once();
    let _ = console_log::init_with_level(log::Level::Info);
}

/// Weighs a `.glb` against what a world draws of one 3D model, before its
/// bytes leave for the world: the code of why it is refused (`broken`,
/// `triangles`, `texels`, `materials`, `vertices`), or empty within the
/// budget. The client checks the same when a file arrives.
#[wasm_bindgen]
pub fn weigh_glb(bytes: &[u8]) -> String {
    client::model3d::weigh(bytes)
        .err()
        .map_or_else(String::new, |why| why.code().to_owned())
}

/// The pictures a UI offers whole: `light`, what a phone or a Pi pays for,
/// `medium` between, and `high`, what the engine starts with. A knob set
/// off them all is the UI's custom picture. Written as `effects_changed`
/// writes its effects, so a UI can tell one in force from a preset field by
/// field.
#[derive(serde::Serialize)]
struct Presets {
    light: client::Effects,
    medium: client::Effects,
    high: client::Effects,
}

#[wasm_bindgen]
pub fn presets() -> String {
    serde_json::to_string(&Presets {
        light: client::Effects::light(),
        medium: client::Effects::medium(),
        high: client::Effects::default(),
    })
    .expect("effects are plain data")
}

/// The layout a cabinet's skin is painted on, as the engine lays a skin
/// (`client::cabinet::template`): its picture, and where each place is
/// named, for the page to write in its own words.
#[wasm_bindgen]
pub struct CabinetTemplate(client::cabinet::Template);

#[wasm_bindgen]
impl CabinetTemplate {
    #[wasm_bindgen(getter)]
    pub fn width(&self) -> u32 {
        self.0.image.width
    }

    #[wasm_bindgen(getter)]
    pub fn height(&self) -> u32 {
        self.0.image.height
    }

    /// RGBA, sRGB, top row first.
    pub fn rgba(&self) -> Vec<u8> {
        self.0.image.rgba.clone()
    }

    /// `[{ name, rect: [x0, y0, x1, y1], ink: [r, g, b] }]`, texels.
    pub fn labels(&self) -> String {
        serde_json::to_string(&self.0.labels).expect("labels are plain data")
    }
}

#[wasm_bindgen]
pub fn cabinet_template() -> CabinetTemplate {
    CabinetTemplate(client::cabinet::template())
}

/// What the voice board lists: every voice, the core's and the trials, how
/// long each sounds, and how many seeds a strike turns through.
#[derive(serde::Serialize)]
struct Board {
    variants: u32,
    voices: Vec<BoardVoice>,
}

#[derive(serde::Serialize)]
struct BoardVoice {
    name: &'static str,
    /// A recipe for a plugin to come: nothing in the world strikes it.
    trial: bool,
    seconds: f32,
}

#[wasm_bindgen]
pub fn voices() -> String {
    const RATE: u32 = 48_000;
    let listed = |voice: &'static synth::Voice, trial| BoardVoice {
        name: voice.name,
        trial,
        seconds: voice.length(0, RATE) as f32 / RATE as f32,
    };
    let core = crate::board::core().map(|voice| listed(voice, false));
    let trials = crate::board::TRIALS
        .iter()
        .map(|&voice| listed(voice, true));
    serde_json::to_string(&Board {
        variants: client::VARIANTS,
        voices: core.chain(trials).collect(),
    })
    .expect("the board is plain data")
}

/// The voice the board calls `name`, rendered for `seed` at `rate` samples
/// a second: what a strike of it plays. Empty when no voice is called so.
#[wasm_bindgen]
pub fn render_voice(name: &str, seed: u32, rate: u32) -> Vec<f32> {
    let Some(voice) = crate::board::called(name) else {
        return Vec::new();
    };
    let mut samples = vec![0.0; voice.length(seed, rate)];
    voice.render(seed, rate, &mut samples);
    samples
}

/// The engine on one canvas. It owns its animation loop and its input
/// listeners; dropping it (`free()` in JS) stops both.
#[wasm_bindgen]
pub struct Engine {
    state: Rc<RefCell<State>>,
    /// Cleared on drop so the loop ends and listeners go quiet.
    alive: Rc<RefCell<bool>>,
    listeners: Vec<Listener>,
}

pub(crate) struct State {
    canvas: HtmlCanvasElement,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    gpu: Gpu,
    pub(crate) renderer: Renderer,
    pub(crate) client: Client,
    input: Input,
    on_event: js_sys::Function,
    last_frame_ms: f64,
    /// When the frame that last handed a picture to the screen began, on
    /// the page's clock; 0 before the first. What the page reads of the
    /// engine's own frames.
    presented_ms: f64,
    /// Set once the device, or the canvas's WebGL context, is lost: no
    /// picture is handed after, whatever an acquire on it answers.
    lost: Arc<AtomicBool>,
    /// The socket to a world server, while the page wants one.
    link: Option<Link>,
    /// Building, the right button is held: the pointer turns the camera.
    looking: bool,
    /// Where the world's files are read from: what `asset://` stands for.
    pub(crate) files: String,
    /// Where the secondary button went down, in client pixels, while it is
    /// down: let go within a few pixels of it, it was a tap.
    secondary_down: Option<(f64, f64)>,
    /// The fingers on the canvas: the stick, the one that looks, the pinch.
    fingers: touch::Fingers,
    /// The part of the canvas the page leaves clear of what it lays over
    /// it, CSS pixels `[left, top, right, bottom]`, as it last said.
    clear: Option<[f64; 4]>,
    /// The pictures that move, playing into the renderer.
    pub(crate) playing: moving::Playing,
    /// The cabinets' games, run by a worker into the renderer.
    pub(crate) carts: carts::Carts,
    /// What is heard, mixed at the eye.
    pub(crate) audio: audio::Audio,
    /// Drawing through WebGL2, where a frame takes another road to the texture.
    pub(crate) webgl: bool,
    /// The steps of the start still to say; none once the first frame drawn is said.
    steps: Option<Steps>,
}

/// The steps of the start said once the loop runs: the first frame handed
/// to the GPU, then the GPU done with it.
struct Steps {
    framed: bool,
    /// The first frame drawn: [`NOT_DRAWN`], then when the GPU said it was
    /// done with it, on the page's clock, as `f64` bits, then
    /// [`DRAWN_SAID`] once the page was told. The GPU says so from a callback
    /// that must be `Send`, so it cannot hold the state.
    drawn: Arc<AtomicU64>,
}

/// The GPU has not said it drew the first frame.
const NOT_DRAWN: u64 = 0;
/// The first frame drawn was said to the page: nothing is left to say.
const DRAWN_SAID: u64 = u64::MAX;

/// Pixels the pointer may move between the secondary button going down and
/// up for it to be a tap and not a drag.
const TAP_PX: f64 = 4.0;

/// Where the web app serves the asset root.
const ASSET_ROOT: &str = "/assets/";
/// Where a world's files are until the page says: the world serves them
/// itself when no bucket is set.
const FILES: &str = "/files";

#[wasm_bindgen]
impl Engine {
    /// Rejects when the browser offers neither WebGPU nor WebGL2, or the
    /// start is not one the engine reads. `start` is JSON, empty for every
    /// default (`start::Start` says what it holds).
    pub async fn create(
        canvas: HtmlCanvasElement,
        on_event: js_sys::Function,
        start: &str,
    ) -> Result<Engine, JsError> {
        let start = Start::read(start).map_err(|e| JsError::new(&e))?;
        let tell = |step: Step| say(start.paint, &on_event, step);
        let instance = match start.backend {
            Backend::WebGpu => wgpu::Instance::default(),
            Backend::WebGl2 => wgpu::Instance::new(wgpu::InstanceDescriptor {
                backends: wgpu::Backends::GL,
                ..wgpu::InstanceDescriptor::new_without_display_handle()
            }),
        };
        let surface = instance
            .create_surface(wgpu::SurfaceTarget::Canvas(canvas.clone()))
            .map_err(|e| JsError::new(&e.to_string()))?;
        let power = match start.power {
            Power::High => wgpu::PowerPreference::HighPerformance,
            Power::Low => wgpu::PowerPreference::LowPower,
        };
        let adapter = Gpu::adapter(&instance, Some(&surface), power)
            .await
            .map_err(|e| JsError::new(&e))?;
        tell(adapter_step(&adapter)).await;
        let gpu = Gpu::open(adapter).await.map_err(|e| JsError::new(&e))?;
        let lost = Arc::new(AtomicBool::new(false));
        let told = lost.clone();
        gpu.device
            .set_device_lost_callback(move |_, _| told.store(true, Ordering::Relaxed));
        tell(Step::Device {
            about: about(&gpu.device),
        })
        .await;
        let config = surface_configuration(&surface, &gpu.adapter, canvas.width(), canvas.height());
        surface.configure(&gpu.device, &config);
        let renderer = Renderer::new(&gpu, config.format);
        tell(Step::Pipelines).await;
        let webgl = gpu.adapter.get_info().backend == wgpu::Backend::Gl;

        // A placeholder world until the UI sends the recipe it wants.
        let mut client =
            Client::new(client::Recipe::new(1)).map_err(|e| JsError::new(&e.to_string()))?;
        if let Some(effects) = start.effects {
            client.command(client::Command::SetEffects { effects });
        }
        // What this version carries, off until a world says which are on.
        for plugin in plugins_client::all() {
            client.plug(plugin);
        }
        let state = Rc::new(RefCell::new(State {
            canvas: canvas.clone(),
            surface,
            config,
            gpu,
            renderer,
            client,
            input: Input::default(),
            on_event,
            last_frame_ms: now_ms(),
            presented_ms: 0.0,
            lost: lost.clone(),
            link: None,
            looking: false,
            files: FILES.to_owned(),
            secondary_down: None,
            fingers: touch::Fingers::default(),
            clear: None,
            playing: moving::Playing::default(),
            carts: carts::Carts::default(),
            audio: audio::Audio::default(),
            webgl,
            steps: Some(Steps {
                framed: false,
                drawn: Arc::new(AtomicU64::new(NOT_DRAWN)),
            }),
        }));
        let alive = Rc::new(RefCell::new(true));
        let mut listeners = listen(&canvas, &state);
        listeners.push(Listener::add(&canvas, "webglcontextlost", move |_| {
            lost.store(true, Ordering::Relaxed);
        }));
        run(state.clone(), alive.clone());
        Ok(Engine {
            state,
            alive,
            listeners,
        })
    }

    /// What makes the worker that runs the cabinets' games, called the
    /// first time one is in view: `() => new Worker(...)` over the page's
    /// `cartridges.worker.ts`.
    pub fn set_carts(&self, make: js_sys::Function) {
        self.state.borrow_mut().carts.set_maker(make);
    }

    /// What makes the node a running game sounds through, the arcade's
    /// chip in an AudioWorklet: `(context) => Promise<AudioWorkletNode>`
    /// (`audio::Audio::set_games` says what it takes).
    pub fn set_games(&self, make: js_sys::Function) {
        self.state.borrow_mut().audio.set_games(make);
    }

    /// One `client::Command` as JSON.
    pub fn command(&self, json: &str) {
        self.state.borrow_mut().client.command_json(json);
    }

    /// Stands in for a world's word where none is linked, as a headless
    /// picture does: every plugin this version carries on, and life with
    /// them, so a measure sees the world whole.
    pub fn rehearse(&self) {
        self.state
            .borrow_mut()
            .client
            .rehearse(client::Level::Anonymous);
    }

    /// Sets the world's clock, seconds: the hour a measure holds still.
    pub fn set_clock(&self, seconds: f64) {
        self.state.borrow_mut().client.set_clock(seconds);
    }

    /// Starts measuring what each frame costs, the CPU's steps by the
    /// page's clock (`render::Renderer::measure`); [`Engine::measured`]
    /// says what it read.
    pub fn measure(&self) {
        self.state.borrow_mut().renderer.measure(Some(now_ms));
    }

    /// Leaves the foliage out of the view and of each cascade a measure
    /// times, where `cascades` holds that cascade's index, and each form of
    /// the view's `forms` holds, near to far.
    pub fn measure_without(&self, view: bool, cascades: &[u8], forms: &[u8]) {
        let without = render::Without {
            view,
            cascades: core::array::from_fn(|k| cascades.contains(&(k as u8))),
            forms: core::array::from_fn(|k| forms.contains(&(k as u8))),
        };
        self.state.borrow_mut().renderer.measure_without(without);
    }

    /// Counts the foliage's fragments in the next frame drawn.
    pub fn count_foliage(&self) {
        self.state.borrow_mut().renderer.count_foliage();
    }

    /// What the measure read so far, as JSON: `null` without one.
    pub fn measured(&self) -> String {
        let measured = self.state.borrow().renderer.measured();
        measured.map_or_else(|| "null".to_owned(), |measured| measured.json())
    }

    /// Presses a key or lets it go, by the name the web gives it (`Space`,
    /// `KeyF`), as the keyboard does: what a button on the screen does in
    /// place of a key.
    pub fn key(&self, code: &str, down: bool) {
        let mut state = self.state.borrow_mut();
        if down {
            state.audio.unlock();
        }
        press(&mut state, code, Chord::default(), down, false);
    }

    /// What the engine holds down right now, as JSON: the keys of the
    /// keyboard by the names the web gives them (`codes`), the core's keys
    /// (`keys`), the stick (`[right, forward]`), and whether the person is
    /// `away`. What a readout shows of a key that stays held.
    pub fn held(&self) -> String {
        let state = self.state.borrow();
        let keys: Vec<String> = state
            .input
            .keys_held()
            .map(|key| format!("{key:?}"))
            .collect();
        serde_json::json!({
            "codes": state.input.codes_held(),
            "keys": keys,
            "stick": state.input.stick,
            "away": state.client.is_away(),
        })
        .to_string()
    }

    /// When the engine last handed a picture to the screen, the start of
    /// that frame on the page's clock; 0 before the first. A frame that
    /// finds no surface, or whose device or WebGL context is lost, hands
    /// none: the page measures the engine's frames by it, never its own.
    pub fn presented_ms(&self) -> f64 {
        self.state.borrow().presented_ms
    }

    /// The ground a field world is shaped by. Sent before the recipe that
    /// names it: a world cannot be generated without the field it was made
    /// from, so the page fetches it first.
    pub fn set_field(&self, bytes: &[u8]) -> Result<(), JsError> {
        self.state
            .borrow_mut()
            .client
            .set_field(bytes)
            .map_err(|e| JsError::new(&e.to_string()))
    }

    /// Opens the link to a world server at a socket URL, the ticket already
    /// in it. The client speaks the protocol; this only carries frames. The
    /// page hears `session` events and decides when to connect again.
    pub fn connect(&self, url: &str) -> Result<(), JsError> {
        self.disconnect();
        let link = Link::open(url, &self.state).map_err(|e| JsError::new(&e))?;
        self.state.borrow_mut().link = Some(link);
        Ok(())
    }

    /// Where the world's files are: the base an `asset://` reference is read
    /// under, as `GET /api/world` says it.
    pub fn set_files(&self, base: &str) {
        self.state.borrow_mut().files = base.trim_end_matches('/').to_owned();
    }

    /// How loud each group of what is heard is, as JSON (`audio::Loudness`):
    /// the shell's mix, heard at once. A level left out is full.
    pub fn set_loudness(&self, json: &str) {
        match serde_json::from_str(json) {
            Ok(loudness) => self.state.borrow_mut().audio.set_loudness(loudness),
            Err(error) => log::warn!("loudness not read: {error}"),
        }
    }

    /// The part of the canvas the page leaves clear of what it lays over
    /// it, CSS pixels from its top left, `[left, top, right, bottom]`: a
    /// game played fills it. None, or any other length, leaves the room
    /// round the middle a page with keys wants.
    pub fn set_clear(&self, clear: Option<Box<[f64]>>) {
        self.state.borrow_mut().clear = clear.and_then(|part| <[f64; 4]>::try_from(&*part).ok());
    }

    /// Closes the link, if one is open.
    pub fn disconnect(&self) {
        let link = self.state.borrow_mut().link.take();
        if let Some(link) = link {
            link.close();
            self.state.borrow_mut().client.link_closed();
        }
    }
}

/// One WebSocket, and its listeners, for as long as the page wants it.
struct Link {
    socket: web_sys::WebSocket,
    listeners: Vec<Listener>,
}

impl Link {
    fn open(url: &str, state: &Rc<RefCell<State>>) -> Result<Link, String> {
        let socket = web_sys::WebSocket::new(url).map_err(|e| format!("{url}: {e:?}"))?;
        socket.set_binary_type(web_sys::BinaryType::Arraybuffer);
        let target: &web_sys::EventTarget = socket.as_ref();

        let opened = {
            let state = state.clone();
            move |_: web_sys::Event| state.borrow_mut().client.link_opened()
        };
        let message = {
            let state = state.clone();
            move |event: web_sys::Event| {
                let event: MessageEvent = event.unchecked_into();
                if let Ok(buffer) = event.data().dyn_into::<js_sys::ArrayBuffer>() {
                    let frame = js_sys::Uint8Array::new(&buffer).to_vec();
                    state.borrow_mut().client.receive(&frame);
                }
            }
        };
        // A close after an error, or an error after a close: the client
        // hears one closing either way, and a second is harmless.
        let closed = {
            let state = state.clone();
            move |_: web_sys::Event| {
                let mut state = state.borrow_mut();
                if state.link.is_some() {
                    state.link = None;
                    state.client.link_closed();
                }
            }
        };
        let listeners = vec![
            Listener::add(target, "open", opened),
            Listener::add(target, "message", message),
            Listener::add(target, "close", closed.clone()),
            Listener::add(target, "error", closed),
        ];
        Ok(Link { socket, listeners })
    }

    fn send(&self, outbound: Outbound) {
        match outbound {
            Outbound::Frame(frame) => {
                if let Err(error) = self.socket.send_with_u8_array(&frame) {
                    log::warn!("send: {error:?}");
                }
            }
            Outbound::Close => self.socket.close().unwrap_or(()),
        }
    }

    fn close(self) {
        for listener in self.listeners {
            listener.remove();
        }
        let _ = self.socket.close();
    }
}

/// Sends what the client queued for the link.
fn pump_link(state: &Rc<RefCell<State>>) {
    let mut held = state.borrow_mut();
    let outbound = held.client.drain_outbound();
    if let Some(link) = &held.link {
        for out in outbound {
            link.send(out);
        }
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        self.disconnect();
        *self.alive.borrow_mut() = false;
        for listener in self.listeners.drain(..) {
            listener.remove();
        }
        if let Some(document) = web_sys::window().and_then(|w| w.document()) {
            document.exit_pointer_lock();
        }
    }
}

/// The browser side of the asset seam: one `fetch` per request, answered
/// whenever it lands, decoded by the browser when pixels were asked for; a
/// picture inside a file the client holds is decoded with nothing fetched.
/// A dropped engine simply never hears the answer.
fn fetch_assets(state: &Rc<RefCell<State>>) {
    let requests = state.borrow_mut().client.drain_asset_requests();
    for request in requests {
        let url = resolve(&request.path, &state.borrow().files);
        let state = Rc::downgrade(state);
        wasm_bindgen_futures::spawn_local(async move {
            match request.form {
                Form::Bytes => {
                    let bytes = fetch(&url).await;
                    if let Some(state) = state.upgrade() {
                        state.borrow_mut().client.asset_loaded(request.id, bytes);
                    }
                }
                Form::Picture { max_side } => {
                    let image = decode(&url, max_side).await;
                    if let Some(state) = state.upgrade() {
                        state.borrow_mut().client.picture_loaded(request.id, image);
                    }
                }
                Form::Embedded { bytes, max_side } => {
                    let image = decode_bytes(&url, &bytes, max_side).await;
                    if let Some(state) = state.upgrade() {
                        state.borrow_mut().client.picture_loaded(request.id, image);
                    }
                }
            }
        });
    }
}

/// An asset reference as a URL: a world's file (`asset://`) is under the
/// files base, an absolute one (a user's own avatar) passes through, a
/// relative one lives under the asset root.
pub(crate) fn resolve(reference: &str, files: &str) -> String {
    if let Some(name) = reference.strip_prefix("asset://") {
        format!("{files}/{name}")
    } else if reference.starts_with('/') || reference.contains("://") {
        reference.to_owned()
    } else {
        format!("{ASSET_ROOT}{reference}")
    }
}

pub(crate) async fn respond(url: &str) -> Result<web_sys::Response, String> {
    let window = web_sys::window().ok_or("no window")?;
    let response = wasm_bindgen_futures::JsFuture::from(window.fetch_with_str(url))
        .await
        .map_err(|error| format!("{url}: {error:?}"))?;
    let response: web_sys::Response = response.unchecked_into();
    if !response.ok() {
        return Err(format!("{url}: HTTP {}", response.status()));
    }
    Ok(response)
}

async fn fetch(url: &str) -> Result<Vec<u8>, String> {
    let describe = |error: JsValue| format!("{url}: {error:?}");
    let response = respond(url).await?;
    let buffer = wasm_bindgen_futures::JsFuture::from(response.array_buffer().map_err(describe)?)
        .await
        .map_err(describe)?;
    Ok(js_sys::Uint8Array::new(&buffer).to_vec())
}

/// A picture fetched and decoded by the browser.
async fn decode(url: &str, max_side: u32) -> Result<scene::Image, String> {
    let describe = |error: JsValue| format!("{url}: {error:?}");
    let response = respond(url).await?;
    let blob = wasm_bindgen_futures::JsFuture::from(response.blob().map_err(describe)?)
        .await
        .map_err(describe)?
        .unchecked_into();
    decode_blob(url, &blob, max_side).await
}

/// A picture the client holds the bytes of, from the file at `url`,
/// decoded by the browser.
async fn decode_bytes(url: &str, bytes: &[u8], max_side: u32) -> Result<scene::Image, String> {
    let describe = |error: JsValue| format!("{url}: {error:?}");
    let parts = js_sys::Array::of1(&js_sys::Uint8Array::from(bytes));
    let blob = web_sys::Blob::new_with_u8_array_sequence(&parts).map_err(describe)?;
    decode_blob(url, &blob, max_side).await
}

/// A picture as the browser decodes it, off the frame's thread, no wider
/// than `max_side` on a side: RGBA, sRGB, straight alpha, top row first.
async fn decode_blob(
    url: &str,
    blob: &web_sys::Blob,
    max_side: u32,
) -> Result<scene::Image, String> {
    use wasm_bindgen_futures::JsFuture;
    use web_sys::{
        ImageBitmap, ImageBitmapOptions, OffscreenCanvas, OffscreenCanvasRenderingContext2d,
        PremultiplyAlpha, ResizeQuality,
    };
    let describe = |error: JsValue| format!("{url}: {error:?}");
    let window = web_sys::window().ok_or("no window")?;
    let options = ImageBitmapOptions::new();
    options.set_premultiply_alpha(PremultiplyAlpha::None);
    let bitmap: ImageBitmap = JsFuture::from(
        window
            .create_image_bitmap_with_blob_and_image_bitmap_options(blob, &options)
            .map_err(describe)?,
    )
    .await
    .map_err(describe)?
    .unchecked_into();
    let (mut width, mut height) = (bitmap.width(), bitmap.height());
    let longest = width.max(height);
    let bitmap = if longest > max_side {
        let scale = f64::from(max_side) / f64::from(longest);
        width = ((f64::from(width) * scale).round() as u32).max(1);
        height = ((f64::from(height) * scale).round() as u32).max(1);
        options.set_resize_width(width);
        options.set_resize_height(height);
        options.set_resize_quality(ResizeQuality::High);
        JsFuture::from(
            window
                .create_image_bitmap_with_image_bitmap_and_image_bitmap_options(&bitmap, &options)
                .map_err(describe)?,
        )
        .await
        .map_err(describe)?
        .unchecked_into()
    } else {
        bitmap
    };
    let canvas = OffscreenCanvas::new(width, height).map_err(describe)?;
    let context: OffscreenCanvasRenderingContext2d = canvas
        .get_context("2d")
        .map_err(describe)?
        .ok_or_else(|| format!("{url}: no 2d context to read pixels with"))?
        .unchecked_into();
    context
        .draw_image_with_image_bitmap(&bitmap, 0.0, 0.0)
        .map_err(describe)?;
    let data = context
        .get_image_data(0.0, 0.0, f64::from(width), f64::from(height))
        .map_err(describe)?;
    Ok(scene::Image {
        width,
        height,
        rgba: data.data().0,
    })
}

impl State {
    /// One frame. Returns the events it produced rather than handing them to
    /// JS here: the whole of this runs inside a `RefCell` borrow of the state,
    /// and a handler that answers an event with a command, which is the most
    /// natural thing a front end does, would re-enter that borrow and panic.
    /// The caller dispatches them once the borrow is gone.
    #[must_use]
    fn frame(&mut self) -> Vec<JsValue> {
        let now = now_ms();
        let dt = (now - self.last_frame_ms) / 1000.0;
        self.last_frame_ms = now;

        // The canvas follows its CSS box; the backing store follows the canvas.
        let ratio = web_sys::window().map_or(1.0, |w| w.device_pixel_ratio());
        let width = ((f64::from(self.canvas.client_width()) * ratio) as u32).max(1);
        let height = ((f64::from(self.canvas.client_height()) * ratio) as u32).max(1);
        if (width, height) != (self.config.width, self.config.height) {
            self.canvas.set_width(width);
            self.canvas.set_height(height);
            self.config.width = width;
            self.config.height = height;
            self.surface.configure(&self.gpu.device, &self.config);
        }

        self.client.set_view([width, height]);
        self.client
            .set_clear(self.clear.map(|part| part.map(|css| css * ratio)));
        let frame = self.client.update(dt, &mut self.input);
        self.audio.tick(&frame.camera, self.client.sounds());
        let struck = self.client.drain_strikes();
        self.audio.strike(&frame.camera, struck);
        self.audio.sustain(&frame.camera, self.client.sustains());
        // A tool aims with a free pointer, and the controls lent take the
        // eye: either lets a captured pointer go.
        if (self.client.pointing() || self.client.controls_lent()) && locked(&self.canvas) {
            if let Some(document) = web_sys::window().and_then(|w| w.document()) {
                document.exit_pointer_lock();
            }
        }
        self.renderer.apply(self.client.drain_terrain_changes());
        self.renderer
            .apply_volumes(self.client.drain_volume_changes());
        self.renderer
            .apply_skinned(self.client.drain_skinned_changes());
        self.renderer
            .apply_pictures(self.client.drain_picture_changes());
        self.renderer
            .apply_models(self.client.drain_model_changes());
        let mut events: Vec<JsValue> = self
            .client
            .drain_events()
            .iter()
            .map(|event| JsValue::from_str(&event.to_json()))
            .collect();
        // The stick is the shell's, drawn by the page: said once a frame at
        // most, while it changes.
        if let Some(stick) = self.fingers.told() {
            events.push(JsValue::from_str(&stick));
        }

        if self.lost.load(Ordering::Relaxed) {
            return events;
        }
        use wgpu::CurrentSurfaceTexture::{Lost, Outdated, Suboptimal, Success};
        let texture = match self.surface.get_current_texture() {
            Success(texture) | Suboptimal(texture) => texture,
            // No picture this frame, but the world still moved and still has
            // things to say, so the events go out either way.
            Outdated | Lost => {
                self.surface.configure(&self.gpu.device, &self.config);
                return events;
            }
            _ => return events,
        };
        let target = texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let view = View {
            camera: frame.camera,
            target: &target,
            size: [width, height],
        };
        self.renderer.render(&frame, &[view]);
        self.gpu.queue.present(texture);
        self.presented_ms = now;
        if let Some(steps) = &mut self.steps {
            steps.said(&self.gpu.queue, [width, height], ratio, &mut events);
            if steps.framed && steps.drawn.load(Ordering::Relaxed) == DRAWN_SAID {
                self.steps = None;
            }
        }
        events
    }
}

impl Steps {
    /// Once a frame while there is a step left to say: the first frame
    /// handed over, then, a frame after the GPU said so, the first drawn.
    fn said(
        &mut self,
        queue: &wgpu::Queue,
        [width, height]: [u32; 2],
        ratio: f64,
        events: &mut Vec<JsValue>,
    ) {
        if !self.framed {
            self.framed = true;
            let step = Step::Frame {
                width,
                height,
                ratio,
            };
            events.push(JsValue::from_str(&step.said(now_ms())));
            let drawn = self.drawn.clone();
            queue.on_submitted_work_done(move || {
                // Never NOT_DRAWN's bits: a time of zero would read as no word.
                drawn.store(now_ms().max(f64::MIN_POSITIVE).to_bits(), Ordering::Relaxed);
            });
            return;
        }
        let bits = self.drawn.load(Ordering::Relaxed);
        if bits != NOT_DRAWN && bits != DRAWN_SAID {
            events.push(JsValue::from_str(&Step::Drawn.said(f64::from_bits(bits))));
            self.drawn.store(DRAWN_SAID, Ordering::Relaxed);
        }
    }
}

/// What the adapter says of itself, as the start's first step.
fn adapter_step(adapter: &wgpu::Adapter) -> Step {
    let info = adapter.get_info();
    let limits = adapter.limits();
    Step::Adapter {
        name: info.name,
        backend: match info.backend {
            wgpu::Backend::Gl => Backend::WebGl2,
            _ => Backend::WebGpu,
        },
        texture_px: limits.max_texture_dimension_2d,
        buffer_mib: limits.max_buffer_size >> 20,
    }
}

/// What WebGPU says of the adapter (`GPUDevice.adapterInfo`), read off the
/// device since wgpu's own adapter info is blank on the web: vendor,
/// architecture, device and description, those it does not hide. Nothing
/// through WebGL2, whose adapter said its name already.
fn about(device: &wgpu::Device) -> Vec<String> {
    let Some(info) = device
        .as_webgpu()
        .and_then(|device| js_sys::Reflect::get(device, &"adapterInfo".into()).ok())
    else {
        return Vec::new();
    };
    ["vendor", "architecture", "device", "description"]
        .into_iter()
        .filter_map(|key| js_sys::Reflect::get(&info, &key.into()).ok()?.as_string())
        .filter(|said| !said.is_empty())
        .collect()
}

/// Says a step of the start and, when the page asked, lets it paint
/// before the next: a step that never ends then shows as the last one said.
async fn say(paint: bool, on_event: &js_sys::Function, step: Step) {
    let json = JsValue::from_str(&step.said(now_ms()));
    if let Err(error) = on_event.call1(&JsValue::NULL, &json) {
        log::error!("event handler threw: {error:?}");
    }
    if paint {
        painted().await;
    }
}

/// Resolves once the page has painted: a timeout queued from an animation
/// frame runs after that frame's paint.
async fn painted() {
    let Some(window) = web_sys::window() else {
        return;
    };
    let promise = js_sys::Promise::new(&mut |resolve, _| {
        let later = window.clone();
        let after = Closure::once_into_js(move || {
            let _ = later.set_timeout_with_callback(resolve.unchecked_ref());
        });
        let _ = window.request_animation_frame(after.unchecked_ref());
    });
    let _ = wasm_bindgen_futures::JsFuture::from(promise).await;
}

/// The loop closure holds a handle to itself so it can reschedule.
type Tick = Rc<RefCell<Option<Closure<dyn FnMut()>>>>;

/// The animation loop: one closure that reschedules itself while `alive`.
fn run(state: Rc<RefCell<State>>, alive: Rc<RefCell<bool>>) {
    let tick: Tick = Rc::new(RefCell::new(None));
    let again = tick.clone();
    *tick.borrow_mut() = Some(Closure::new(move || {
        if !*alive.borrow() {
            // Dropping the closure here breaks the cycle that kept it alive.
            again.borrow_mut().take();
            return;
        }
        fetch_assets(&state);
        moving::tick(&state);
        carts::tick(&state);
        pump_link(&state);
        let (events, on_event) = {
            let mut held = state.borrow_mut();
            let events = held.frame();
            (events, held.on_event.clone())
        };
        // Outside the borrow: a handler is free to answer with a command.
        for json in events {
            if let Err(error) = on_event.call1(&JsValue::NULL, &json) {
                log::error!("event handler threw: {error:?}");
            }
        }
        request_frame(again.borrow().as_ref().expect("the loop closure"));
    }));
    request_frame(tick.borrow().as_ref().expect("the loop closure"));
}

fn request_frame(tick: &Closure<dyn FnMut()>) {
    let window = web_sys::window().expect("a window");
    window
        .request_animation_frame(tick.as_ref().unchecked_ref())
        .expect("requestAnimationFrame");
}

pub(crate) fn now_ms() -> f64 {
    web_sys::window()
        .and_then(|w| w.performance())
        .map_or(0.0, |p| p.now())
}

/// A DOM listener that can be removed again.
struct Listener {
    target: web_sys::EventTarget,
    kind: &'static str,
    closure: Closure<dyn FnMut(web_sys::Event)>,
}

impl Listener {
    fn add(
        target: &web_sys::EventTarget,
        kind: &'static str,
        handler: impl FnMut(web_sys::Event) + 'static,
    ) -> Listener {
        let closure = Closure::new(handler);
        // Not passive: the wheel handler must be able to stop the page scroll.
        let options = web_sys::AddEventListenerOptions::new();
        options.set_passive(false);
        target
            .add_event_listener_with_callback_and_add_event_listener_options(
                kind,
                closure.as_ref().unchecked_ref(),
                &options,
            )
            .expect("addEventListener");
        Listener {
            target: target.clone(),
            kind,
            closure,
        }
    }

    fn remove(self) {
        let _ = self
            .target
            .remove_event_listener_with_callback(self.kind, self.closure.as_ref().unchecked_ref());
    }
}

/// Whether the pointer is captured by this canvas.
fn locked(canvas: &HtmlCanvasElement) -> bool {
    let locked = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.pointer_lock_element());
    let own: &web_sys::Element = canvas.as_ref();
    locked.is_some_and(|element| &element == own)
}

/// Where a pointer event is over the canvas, as fractions from the top left.
fn fraction(canvas: &HtmlCanvasElement, event: &PointerEvent) -> [f32; 2] {
    [
        event.offset_x() as f32 / canvas.client_width().max(1) as f32,
        event.offset_y() as f32 / canvas.client_height().max(1) as f32,
    ]
}

/// Pointer buttons as the DOM numbers them.
const PRIMARY: i16 = 0;
const SECONDARY: i16 = 2;

/// Whether the page has the keys, which the canvas leaves them to: for a
/// field of the panels typed into or chosen from, a select or a combobox
/// alike, or for a modal dialog open over the world, whose Escape cancels
/// it.
fn page_has_keys() -> bool {
    let focused = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.active_element());
    focused.is_some_and(|element| {
        let editable = element.has_attribute("contenteditable");
        let modal = element.closest("dialog[open]").ok().flatten().is_some();
        let combobox = element.get_attribute("role").as_deref() == Some("combobox");
        editable
            || modal
            || combobox
            || matches!(element.tag_name().as_str(), "INPUT" | "TEXTAREA" | "SELECT")
    })
}

fn listen(canvas: &HtmlCanvasElement, state: &Rc<RefCell<State>>) -> Vec<Listener> {
    let target: &web_sys::EventTarget = canvas.as_ref();
    let window = web_sys::window().expect("a window");
    let page: &web_sys::EventTarget = window.as_ref();

    // The keys are the page's wherever its focus is, a button of a panel
    // clicked last included, and a field typed into alone keeps them, as
    // does a key the page already took (an Escape that closed a card). A
    // key the world takes brings the focus back to the canvas, so the
    // button does not act on it too.
    let key = |down: bool| {
        let (state, canvas) = (state.clone(), canvas.clone());
        move |event: web_sys::Event| {
            if page_has_keys() || event.default_prevented() {
                return;
            }
            if down {
                state.borrow_mut().audio.unlock();
            }
            let event: KeyboardEvent = event.unchecked_into();
            // Command on a Mac, Control elsewhere.
            let chord = Chord {
                command: event.meta_key() || event.ctrl_key(),
                shift: event.shift_key(),
            };
            let taken = press(
                &mut state.borrow_mut(),
                &event.code(),
                chord,
                down,
                event.repeat(),
            );
            if taken {
                event.prevent_default();
                let _ = canvas.focus();
            }
        }
    };
    // While a plugin has the pointer, the primary button is its tool's and
    // the secondary one looks while it is held, so the pointer stays free to
    // aim. Otherwise a press captures the pointer, and the camera has it
    // until Escape. A finger on a world no tool aims at walks and looks
    // instead, and captures nothing.
    let down = {
        let (state, canvas) = (state.clone(), canvas.clone());
        move |event: web_sys::Event| {
            let event: PointerEvent = event.unchecked_into();
            let _ = canvas.focus();
            let mut state = state.borrow_mut();
            state.audio.unlock();
            if finger(&event) && !state.client.pointing() {
                event.prevent_default();
                let width = canvas.client_width() as f32;
                state
                    .fingers
                    .down(event.pointer_id(), offset(&event), width);
                return;
            }
            if event.button() == SECONDARY {
                state.secondary_down = Some((event.client_x().into(), event.client_y().into()));
            }
            if state.client.pointing() {
                event.prevent_default();
                let _ = canvas.set_pointer_capture(event.pointer_id());
                state.input.pointer = Some(fraction(&canvas, &event));
                // The pointer says what is held with it, whoever had the
                // keys when it went down.
                hold_alt(&mut state.input, event.alt_key());
                match event.button() {
                    PRIMARY => state.input.key(Key::Use, true),
                    SECONDARY => state.looking = true,
                    _ => {}
                }
            } else if event.button() == PRIMARY && !state.client.controls_lent() {
                canvas.request_pointer_lock();
            }
        }
    };
    // Let go, or taken back by the browser: a finger lifts either way.
    let up = || {
        let state = state.clone();
        move |event: web_sys::Event| {
            let event: PointerEvent = event.unchecked_into();
            let mut state = state.borrow_mut();
            let state = &mut *state;
            if finger(&event) && state.fingers.up(event.pointer_id(), &mut state.input) {
                return;
            }
            match event.button() {
                PRIMARY => state.input.key(Key::Use, false),
                SECONDARY => {
                    state.looking = false;
                    // Let go where it went down, it was a tap: what is under
                    // the pointer is picked to be looked at again.
                    let (x, y) = (f64::from(event.client_x()), f64::from(event.client_y()));
                    if let Some((x0, y0)) = state.secondary_down.take()
                        && (x - x0).abs() <= TAP_PX
                        && (y - y0).abs() <= TAP_PX
                    {
                        state.input.inspect();
                    }
                }
                _ => {}
            }
        }
    };
    let motion = {
        let (state, canvas) = (state.clone(), canvas.clone());
        move |event: web_sys::Event| {
            let event: PointerEvent = event.unchecked_into();
            let mut state = state.borrow_mut();
            let state = &mut *state;
            if finger(&event)
                && state
                    .fingers
                    .moved(event.pointer_id(), offset(&event), &mut state.input)
            {
                return;
            }
            let turning = locked(&canvas) || state.looking;
            if !locked(&canvas) {
                state.input.pointer = Some(fraction(&canvas, &event));
            }
            // With a tool in hand, the keys are the tool's while the pointer
            // is over the world: a button of the panel that was clicked last
            // keeps them otherwise, and a key meant for the stroke is lost.
            if state.client.pointing() {
                hold_alt(&mut state.input, event.alt_key());
                if !page_has_keys() {
                    let _ = canvas.focus();
                }
            }
            if turning {
                state.input.look[0] += event.movement_x() as f32;
                state.input.look[1] += event.movement_y() as f32;
            }
        }
    };
    // The secondary button is the world's over the canvas: it looks while
    // a tool is in hand and picks with a tap. The page's menu stays off it.
    let menu = move |event: web_sys::Event| {
        event.prevent_default();
    };
    let wheel = {
        let state = state.clone();
        move |event: web_sys::Event| {
            event.prevent_default();
            let event: WheelEvent = event.unchecked_into();
            state.borrow_mut().input.zoom -= (event.delta_y() / 100.0) as f32;
        }
    };
    // The canvas or the page losing focus lets every key go: what was held
    // cannot be heard up again. A finger lifting is always heard, so a
    // button of the page touched beside the stick leaves the stick held;
    // the page itself going lets the fingers go too, and the person is
    // away from the world until it comes back.
    let blur = |page: bool| {
        let state = state.clone();
        move |_: web_sys::Event| {
            let mut state = state.borrow_mut();
            let state = &mut *state;
            state.input.release_all();
            state.looking = false;
            state.secondary_down = None;
            if page {
                state.fingers.release(&mut state.input);
                state.client.away(true);
            }
        }
    };
    let focus = {
        let state = state.clone();
        move |_: web_sys::Event| {
            if !away() {
                state.borrow_mut().client.away(false);
            }
        }
    };

    // A hidden page draws no frame, so nothing names a sustained voice,
    // and no frame holds a game: the page going and coming says so
    // instead, a game held as the page hides, by the frame's own handing
    // of what the games run. Shown again, it is back only with the keys.
    let shown = {
        let state = state.clone();
        move |_: web_sys::Event| {
            let hidden = web_sys::window()
                .and_then(|w| w.document())
                .is_some_and(|d| d.hidden());
            {
                let mut state = state.borrow_mut();
                state.audio.hidden(hidden);
                state.client.away(away());
            }
            if hidden {
                carts::tick(&state);
            }
        }
    };
    let document: web_sys::EventTarget = window.document().expect("a document").into();

    vec![
        Listener::add(&document, "visibilitychange", shown),
        Listener::add(page, "keydown", key(true)),
        Listener::add(page, "keyup", key(false)),
        Listener::add(page, "blur", blur(true)),
        Listener::add(page, "focus", focus),
        Listener::add(target, "pointerdown", down),
        Listener::add(target, "pointerup", up()),
        Listener::add(target, "pointercancel", up()),
        Listener::add(target, "pointermove", motion),
        Listener::add(target, "contextmenu", menu),
        Listener::add(target, "wheel", wheel),
        Listener::add(target, "blur", blur(false)),
    ]
}

/// A key going down or up, by the name the web gives it, with what is held
/// with it: the keyboard's and a button's on the screen alike. Says whether
/// the world took it, so the page leaves it be.
fn press(state: &mut State, code: &str, chord: Chord, down: bool, repeat: bool) -> bool {
    let asked = state.client.asks(code, chord);
    let lent = state.client.controls_lent();
    keys::press(&mut state.input, code, chord, (down, repeat), asked, lent)
}

/// Whether the person is away from the world: the page hidden, or its
/// window without the keys, the address bar or another app holding them.
fn away() -> bool {
    web_sys::window()
        .and_then(|w| w.document())
        .is_none_or(|d| d.hidden() || !d.has_focus().unwrap_or(true))
}

/// Whether a pointer is a finger on the glass.
fn finger(event: &PointerEvent) -> bool {
    event.pointer_type() == "touch"
}

/// Where a pointer is over the canvas, in CSS pixels from its top left.
fn offset(event: &PointerEvent) -> [f32; 2] {
    [event.offset_x() as f32, event.offset_y() as f32]
}

/// Keeps Alt in step with what a pointer event says of it. A plugin asks for
/// either Alt by the name the web gives it.
fn hold_alt(input: &mut Input, down: bool) {
    input.hold("AltLeft", down);
    if !down {
        input.hold("AltRight", false);
    }
}
