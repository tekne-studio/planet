//! A cartridge in a window, over softbuffer: the keyboard is its buttons,
//! the updates by the emulator's clock, the screen scaled by whole pixels.
//! No sound yet: the runner lets the oldest tones go.
//!
//! The keys are a cabinet's, by the core's bindings and the pad's table
//! (`client::Pad::keyed`): WASD or the arrows the stick, Space or Z A,
//! Shift or X B, Enter start; Escape closes the window.

use std::num::NonZeroU32;
use std::rc::Rc;
use std::time::{Duration, Instant};

use arcade::{HEIGHT, TICKS, WIDTH};
use arcade_host::Emulator;
use client::{Key, Pad};
use softbuffer::{Context, Surface};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, KeyEvent, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::PhysicalKey;
use winit::window::{Window, WindowId};

const BORDER: u32 = 0x0010_1014;
/// The cart's name on the emulator's shelf: the window's only one.
const CART: u32 = 1;

pub fn run(
    mut emulator: Emulator,
    file: &[u8],
    seed: u32,
    title: String,
    scale: u32,
) -> Result<(), String> {
    let began = Instant::now();
    emulator
        .open(CART, file, seed, 0.0, false)
        .map_err(|e| e.to_string())?;
    let rgba = emulator
        .rgba(CART)
        .map_err(|e| e.to_string())?
        .ok_or("the cart is not on the shelf")?;
    let event_loop = EventLoop::new().map_err(|e| e.to_string())?;
    let mut app = App {
        emulator,
        began,
        title,
        scale,
        stage: None,
        held: Vec::new(),
        tapped: Pad::NONE,
        next: Instant::now(),
        rgba,
        failure: None,
    };
    event_loop.run_app(&mut app).map_err(|e| e.to_string())?;
    app.failure.map_or(Ok(()), Err)
}

struct App {
    emulator: Emulator,
    /// The emulator's clock starts here.
    began: Instant,
    title: String,
    scale: u32,
    stage: Option<Stage>,
    /// The keys held.
    held: Vec<Key>,
    /// What the keys that went down since the last update press: a tap
    /// between two updates still reaches the game.
    tapped: Pad,
    /// When the next frame is due.
    next: Instant,
    rgba: Vec<u8>,
    failure: Option<String>,
}

struct Stage {
    window: Rc<Window>,
    surface: Surface<Rc<Window>, Rc<Window>>,
    size: [u32; 2],
}

impl App {
    /// The updates due by the emulator's clock, then the screen drawn.
    fn frame(&mut self) -> Result<(), String> {
        let now = Instant::now();
        self.next = now + Duration::from_secs(1) / TICKS;
        let held = self
            .held
            .iter()
            .fold(Pad::NONE, |pad, &key| pad | Pad::keyed(key));
        let emulator = &mut self.emulator;
        let broken = |e: arcade_host::Broken| e.to_string();
        emulator
            .press(CART, (held | self.tapped).buttons())
            .map_err(broken)?;
        let ms = now.duration_since(self.began).as_secs_f64() * 1000.0;
        if emulator.tick(ms).map_err(broken)?.contains(&CART) {
            // An update ran: a tap reached it.
            self.tapped = Pad::NONE;
            if let Some(rgba) = emulator.rgba(CART).map_err(broken)? {
                self.rgba = rgba;
            }
        }
        let Some(stage) = &mut self.stage else {
            return Ok(());
        };
        let size = stage.window.inner_size();
        let (Some(w), Some(h)) = (NonZeroU32::new(size.width), NonZeroU32::new(size.height)) else {
            return Ok(());
        };
        if stage.size != [size.width, size.height] {
            stage.surface.resize(w, h).map_err(|e| e.to_string())?;
            stage.size = [size.width, size.height];
        }
        let mut buffer = stage.surface.buffer_mut().map_err(|e| e.to_string())?;
        draw(&mut buffer, stage.size, &self.rgba);
        buffer.present().map_err(|e| e.to_string())
    }
}

/// The screen, as large as whole pixels fit, centred on a border.
fn draw(buffer: &mut [u32], [width, height]: [u32; 2], rgba: &[u8]) {
    let (w, h) = (WIDTH, HEIGHT);
    let scale = (width / w).min(height / h).max(1);
    let (x0, y0) = (
        width.saturating_sub(w * scale) / 2,
        height.saturating_sub(h * scale) / 2,
    );
    buffer.fill(BORDER);
    for y in 0..(h * scale).min(height) {
        let row = (y / scale * w) as usize;
        let out = ((y0 + y) * width + x0) as usize;
        for x in 0..(w * scale).min(width - x0) {
            let at = (row + (x / scale) as usize) * 4;
            let [r, g, b] = [rgba[at], rgba[at + 1], rgba[at + 2]].map(u32::from);
            buffer[out + x as usize] = r << 16 | g << 8 | b;
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.stage.is_some() {
            return;
        }
        let side = |scale: u32| LogicalSize::new(WIDTH * scale, HEIGHT * scale);
        let attributes = Window::default_attributes()
            .with_title(&self.title)
            .with_inner_size(side(self.scale))
            .with_min_inner_size(side(1));
        let made = event_loop
            .create_window(attributes)
            .map_err(|e| e.to_string())
            .and_then(|window| {
                let window = Rc::new(window);
                let context = Context::new(window.clone()).map_err(|e| e.to_string())?;
                let surface = Surface::new(&context, window.clone()).map_err(|e| e.to_string())?;
                Ok(Stage {
                    window,
                    surface,
                    size: [0, 0],
                })
            });
        match made {
            Ok(stage) => {
                self.next = Instant::now();
                stage.window.request_redraw();
                self.stage = Some(stage);
            }
            Err(message) => {
                self.failure = Some(message);
                event_loop.exit();
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Focused(false) => {
                self.held.clear();
                self.tapped = Pad::NONE;
            }
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(key),
                        state,
                        repeat: false,
                        ..
                    },
                ..
            } => {
                let down = state == ElementState::Pressed;
                match crate::window::binding(key) {
                    Some(Key::Leave) => event_loop.exit(),
                    Some(key) if down => {
                        if !self.held.contains(&key) {
                            self.held.push(key);
                        }
                        self.tapped = self.tapped | Pad::keyed(key);
                    }
                    Some(key) => self.held.retain(|&held| held != key),
                    None => {}
                }
            }
            WindowEvent::RedrawRequested => {
                if let Err(message) = self.frame() {
                    self.failure = Some(message);
                    event_loop.exit();
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(stage) = &self.stage {
            if Instant::now() >= self.next {
                stage.window.request_redraw();
            }
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(self.next));
    }
}
