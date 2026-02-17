use std::collections::HashSet;
use std::sync::Arc;
use std::time::Instant;

use winit::application::ApplicationHandler;
use winit::event::{ElementState, KeyEvent, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
pub use winit::keyboard::KeyCode;
use winit::keyboard::PhysicalKey;
use winit::window::{Window, WindowId};

use crate::renderer::Renderer;
use crate::renderer::pipeline::TileVertex;

// ── Color ──────────────────────────────────────────────────────────────────

#[derive(Copy, Clone, Debug)]
pub struct Color(pub [f32; 4]);

impl Color {
    pub const WHITE: Self = Self([1.0, 1.0, 1.0, 1.0]);
    pub const BLACK: Self = Self([0.0, 0.0, 0.0, 1.0]);
    pub const GRAY: Self = Self([0.6, 0.6, 0.6, 1.0]);
    pub const DARK_GRAY: Self = Self([0.2, 0.2, 0.2, 1.0]);
    pub const RED: Self = Self([1.0, 0.0, 0.0, 1.0]);
    pub const GREEN: Self = Self([0.0, 1.0, 0.0, 1.0]);
    pub const BLUE: Self = Self([0.0, 0.0, 1.0, 1.0]);
    pub const YELLOW: Self = Self([1.0, 1.0, 0.0, 1.0]);
    pub const CYAN: Self = Self([0.0, 1.0, 1.0, 1.0]);
    pub const MAGENTA: Self = Self([1.0, 0.0, 1.0, 1.0]);
}

// ── Game trait ──────────────────────────────────────────────────────────────

pub trait Game {
    fn update(&mut self, engine: &mut Engine);
    fn render(&mut self, engine: &mut Engine);
}

// ── Tile cell ───────────────────────────────────────────────────────────────

#[derive(Copy, Clone)]
struct TileCell {
    index: u32,
    fg: Color,
    bg: Color,
}

impl Default for TileCell {
    fn default() -> Self {
        Self {
            index: 0,
            fg: Color::WHITE,
            bg: Color::BLACK,
        }
    }
}

// ── Engine ──────────────────────────────────────────────────────────────────

pub struct Engine {
    renderer: Renderer,
    tile_w: u32,
    tile_h: u32,
    grid_w: u32,
    grid_h: u32,
    grid: Vec<TileCell>,
    dt: f32,
    tick: u64,
    keys_held: HashSet<KeyCode>,
    keys_pressed: HashSet<KeyCode>,
    keys_released: HashSet<KeyCode>,
}

impl Engine {
    pub fn builder() -> EngineBuilder {
        EngineBuilder::default()
    }

    fn from_builder(renderer: Renderer, tile_w: u32, tile_h: u32) -> Self {
        let size = renderer.window.inner_size();
        let grid_w = size.width / tile_w;
        let grid_h = size.height / tile_h;
        let grid = vec![TileCell::default(); (grid_w * grid_h) as usize];
        Self {
            renderer,
            tile_w,
            tile_h,
            grid_w,
            grid_h,
            grid,
            dt: 0.0,
            tick: 0,
            keys_held: HashSet::new(),
            keys_pressed: HashSet::new(),
            keys_released: HashSet::new(),
        }
    }

    pub fn dt(&self) -> f32 {
        self.dt
    }

    pub fn tick(&self) -> u64 {
        self.tick
    }

    pub fn grid_width(&self) -> u32 {
        self.grid_w
    }

    pub fn grid_height(&self) -> u32 {
        self.grid_h
    }

    pub fn is_key_held(&self, key: KeyCode) -> bool {
        self.keys_held.contains(&key)
    }

    pub fn is_key_pressed(&self, key: KeyCode) -> bool {
        self.keys_pressed.contains(&key)
    }

    pub fn is_key_released(&self, key: KeyCode) -> bool {
        self.keys_released.contains(&key)
    }

    pub fn clear(&mut self) {
        self.grid.fill(TileCell::default());
    }

    pub fn set_tile(&mut self, x: u32, y: u32, index: u32, fg: Color, bg: Color) {
        if x < self.grid_w && y < self.grid_h {
            let i = (y * self.grid_w + x) as usize;
            self.grid[i] = TileCell { index, fg, bg };
        }
    }

    pub fn set_char(&mut self, x: u32, y: u32, ch: char, fg: Color, bg: Color) {
        self.set_tile(x, y, ch as u32, fg, bg);
    }

    fn build_vertices(&self) -> Vec<TileVertex> {
        let mut verts = Vec::with_capacity((self.grid_w * self.grid_h * 6) as usize);
        for y in 0..self.grid_h {
            for x in 0..self.grid_w {
                let cell = &self.grid[(y * self.grid_w + x) as usize];
                let px = (x * self.tile_w) as f32;
                let py = (y * self.tile_h) as f32;
                let pw = self.tile_w as f32;
                let ph = self.tile_h as f32;

                let (uv_min, uv_max) = self.renderer.atlas.uv_for_index(cell.index);

                let tl = TileVertex {
                    position: [px, py],
                    uv: uv_min,
                    fg_color: cell.fg.0,
                    bg_color: cell.bg.0,
                };
                let tr = TileVertex {
                    position: [px + pw, py],
                    uv: [uv_max[0], uv_min[1]],
                    fg_color: cell.fg.0,
                    bg_color: cell.bg.0,
                };
                let bl = TileVertex {
                    position: [px, py + ph],
                    uv: [uv_min[0], uv_max[1]],
                    fg_color: cell.fg.0,
                    bg_color: cell.bg.0,
                };
                let br = TileVertex {
                    position: [px + pw, py + ph],
                    uv: uv_max,
                    fg_color: cell.fg.0,
                    bg_color: cell.bg.0,
                };

                // Two triangles: TL-BL-TR, TR-BL-BR
                verts.extend_from_slice(&[tl, bl, tr, tr, bl, br]);
            }
        }
        verts
    }

    fn handle_resize(&mut self) {
        let size = self.renderer.window.inner_size();
        let new_gw = size.width / self.tile_w;
        let new_gh = size.height / self.tile_h;
        if new_gw != self.grid_w || new_gh != self.grid_h {
            self.grid_w = new_gw;
            self.grid_h = new_gh;
            self.grid = vec![TileCell::default(); (new_gw * new_gh) as usize];
        }
    }
}

// ── EngineBuilder ───────────────────────────────────────────────────────────

pub struct EngineBuilder {
    title: String,
    width: u32,
    height: u32,
    png_bytes: &'static [u8],
    tile_w: u32,
    tile_h: u32,
    target_ups: u32,
}

impl Default for EngineBuilder {
    fn default() -> Self {
        Self {
            title: "jengine".into(),
            width: 800,
            height: 600,
            png_bytes: &[],
            tile_w: 16,
            tile_h: 16,
            target_ups: 60,
        }
    }
}

impl EngineBuilder {
    pub fn with_title(mut self, title: &str) -> Self {
        self.title = title.into();
        self
    }

    pub fn with_size(mut self, width: u32, height: u32) -> Self {
        self.width = width;
        self.height = height;
        self
    }

    pub fn with_tileset(mut self, png_bytes: &'static [u8], tile_w: u32, tile_h: u32) -> Self {
        self.png_bytes = png_bytes;
        self.tile_w = tile_w;
        self.tile_h = tile_h;
        self
    }

    pub fn with_ups(mut self, ups: u32) -> Self {
        self.target_ups = ups;
        self
    }

    pub fn run(self, game: impl Game + 'static) {
        let event_loop = EventLoop::new().unwrap();
        let fixed_dt = 1.0 / self.target_ups as f32;
        let mut app = App {
            config: self,
            game: Box::new(game),
            engine: None,
            last_instant: None,
            accumulator: 0.0,
            fixed_dt,
        };
        event_loop.run_app(&mut app).unwrap();
    }
}

// ── App (winit ApplicationHandler) ──────────────────────────────────────────

struct App {
    config: EngineBuilder,
    game: Box<dyn Game>,
    engine: Option<Engine>,
    last_instant: Option<Instant>,
    accumulator: f32,
    fixed_dt: f32,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = Arc::new(
            event_loop
                .create_window(
                    Window::default_attributes()
                        .with_title(&self.config.title)
                        .with_inner_size(winit::dpi::LogicalSize::new(
                            self.config.width,
                            self.config.height,
                        )),
                )
                .unwrap(),
        );
        let renderer = pollster::block_on(Renderer::new(
            window,
            self.config.png_bytes,
            self.config.tile_w,
            self.config.tile_h,
        ));
        self.engine = Some(Engine::from_builder(
            renderer,
            self.config.tile_w,
            self.config.tile_h,
        ));
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(engine) = self.engine.as_ref() {
            engine.renderer.window.request_redraw();
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(engine) = self.engine.as_mut() else {
            return;
        };

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),

            WindowEvent::Resized(size) => {
                engine.renderer.resize(size);
                engine.handle_resize();
            }

            WindowEvent::RedrawRequested => {
                let now = Instant::now();
                let elapsed = match self.last_instant {
                    Some(prev) => now.duration_since(prev).as_secs_f32().min(0.25),
                    None => self.fixed_dt,
                };
                self.last_instant = Some(now);
                self.accumulator += elapsed;

                while self.accumulator >= self.fixed_dt {
                    engine.dt = self.fixed_dt;
                    engine.tick += 1;
                    self.game.update(engine);
                    self.accumulator -= self.fixed_dt;
                }

                engine.keys_pressed.clear();
                engine.keys_released.clear();

                self.game.render(engine);

                let vertices = engine.build_vertices();
                match engine.renderer.render(&vertices) {
                    Ok(_) => {}
                    Err(wgpu::SurfaceError::Lost) => {
                        let size = engine.renderer.window.inner_size();
                        engine.renderer.resize(size);
                    }
                    Err(e) => eprintln!("render error: {e}"),
                }
            }

            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(code),
                        state,
                        ..
                    },
                ..
            } => match state {
                ElementState::Pressed => {
                    if engine.keys_held.insert(code) {
                        engine.keys_pressed.insert(code);
                    }
                }
                ElementState::Released => {
                    engine.keys_held.remove(&code);
                    engine.keys_released.insert(code);
                }
            },

            _ => {}
        }
    }
}
