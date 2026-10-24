use std::error::Error;
use std::num::NonZeroU32;

use glutin::config::{Config, ConfigTemplateBuilder, GlConfig};
use glutin::context::{ContextApi, ContextAttributesBuilder, GlProfile, NotCurrentGlContext, PossiblyCurrentContext, PossiblyCurrentGlContext, Version};
use glutin::display::{GetGlDisplay, GlDisplay};
use glutin::surface::{GlSurface, Surface, SwapInterval, WindowSurface};
use glow::HasContext;
use glutin_winit::{DisplayBuilder, GlWindow};
use raw_window_handle::HasWindowHandle;
use twinbit_game::Frame;
use twinbit_render::{Profile, Renderer};
use winit::dpi::{LogicalPosition, LogicalSize, PhysicalSize};
use winit::event_loop::ActiveEventLoop;
use winit::window::{Fullscreen, Window, WindowAttributes, WindowId};

const MAX_SAMPLES: u8 = 4;
const PROFILE: Profile = if cfg!(target_os = "linux") { Profile::Gles } else { Profile::Desktop };

pub struct Screen {
    pub window: Window,
    surface: Surface<WindowSurface>,
}

impl Screen {
    fn new(window: Window, config: &Config) -> Result<Self, Box<dyn Error>> {
        let attrs = window.build_surface_attributes(Default::default())?;
        let surface = unsafe { config.display().create_window_surface(config, &attrs)? };
        Ok(Self { window, surface })
    }
}

/// One GL context shared by two windows: one per physical screen.
pub struct Gfx {
    pub top: Screen,
    pub bottom: Screen,
    context: PossiblyCurrentContext,
    gl: glow::Context,
    renderer: Renderer,
}

impl Gfx {
    pub fn new(el: &ActiveEventLoop) -> Result<Self, Box<dyn Error>> {
        let (top_attrs, bottom_attrs) = window_attrs(el);
        let mut template = ConfigTemplateBuilder::new().with_depth_size(24);
        if PROFILE == Profile::Gles {
            template = template.with_api(glutin::config::Api::GLES3);
        }
        let (top_window, config) = DisplayBuilder::new()
            .with_window_attributes(Some(top_attrs))
            .build(el, template, |configs| {
                configs
                    .max_by_key(|c| (c.depth_size().min(24), c.num_samples().min(MAX_SAMPLES), std::cmp::Reverse(c.num_samples())))
                    .expect("no GL config")
            })?;
        eprintln!("gl config: depth {} bits, {} samples", config.depth_size(), config.num_samples());
        let top_window = top_window.ok_or("display builder made no window")?;
        let bottom_window = glutin_winit::finalize_window(el, bottom_attrs, &config)?;

        let api = match PROFILE {
            Profile::Gles => ContextApi::Gles(Some(Version::new(3, 0))),
            Profile::Desktop => ContextApi::OpenGl(Some(Version::new(3, 3))),
        };
        let context_attrs = ContextAttributesBuilder::new()
            .with_profile(GlProfile::Core)
            .with_context_api(api)
            .build(Some(top_window.window_handle()?.as_raw()));
        let display = config.display();
        let not_current = unsafe { display.create_context(&config, &context_attrs)? };

        let top = Screen::new(top_window, &config)?;
        let bottom = Screen::new(bottom_window, &config)?;
        let context = not_current.make_current(&bottom.surface)?;
        // Vsync on one screen paces the loop; waiting on both would halve the frame rate.
        bottom.surface.set_swap_interval(&context, SwapInterval::DontWait).ok();
        context.make_current(&top.surface)?;
        top.surface.set_swap_interval(&context, SwapInterval::Wait(NonZeroU32::MIN)).ok();

        let gl = unsafe { glow::Context::from_loader_function_cstr(|s| display.get_proc_address(s)) };
        let renderer = Renderer::new(&gl, PROFILE)?;
        Ok(Self { top, bottom, context, gl, renderer })
    }

    pub fn resize(&self, id: WindowId, size: PhysicalSize<u32>) {
        let (Some(w), Some(h)) = (NonZeroU32::new(size.width), NonZeroU32::new(size.height)) else { return };
        for screen in [&self.top, &self.bottom] {
            if screen.window.id() == id {
                screen.surface.resize(&self.context, w, h);
            }
        }
    }

    /// Draws both screens. With `capture`, also returns them stacked top above bottom as RGBA rows.
    pub fn render(&mut self, frame: &Frame, capture: bool) -> Result<Option<Shot>, Box<dyn Error>> {
        let Self { top, bottom, context, gl, renderer } = self;
        let mut shots = Vec::new();
        for (screen, canvas) in [(&*top, &frame.top), (&*bottom, &frame.bottom)] {
            context.make_current(&screen.surface)?;
            let size = screen.window.inner_size();
            renderer.draw(gl, canvas, [0, 0, size.width as i32, size.height as i32]);
            if capture {
                shots.push(read_back(gl, size.width, size.height));
            }
            screen.surface.swap_buffers(context)?;
        }
        Ok(capture.then(|| stack(shots)))
    }
}

pub struct Shot {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

const SHOT_GAP: u32 = 16;

fn read_back(gl: &glow::Context, width: u32, height: u32) -> Shot {
    let mut rgba = vec![0u8; (width * height * 4) as usize];
    unsafe {
        gl.read_pixels(0, 0, width as i32, height as i32, glow::RGBA, glow::UNSIGNED_BYTE, glow::PixelPackData::Slice(Some(&mut rgba)));
    }
    // GL rows run bottom to top, and the window's alpha channel is not meaningful in a screenshot.
    let row = (width * 4) as usize;
    let mut flipped: Vec<u8> = rgba.chunks_exact(row).rev().flatten().copied().collect();
    flipped.chunks_exact_mut(4).for_each(|px| px[3] = 255);
    Shot { width, height, rgba: flipped }
}

fn stack(shots: Vec<Shot>) -> Shot {
    let width = shots.iter().map(|s| s.width).max().unwrap_or(0);
    let height = shots.iter().map(|s| s.height).sum::<u32>() + SHOT_GAP;
    let mut rgba = Vec::with_capacity((width * height * 4) as usize);
    for (i, s) in shots.iter().enumerate() {
        if i > 0 {
            rgba.extend(std::iter::repeat_n([40, 40, 48, 255], (width * SHOT_GAP) as usize).flatten());
        }
        for row in s.rgba.chunks_exact((s.width * 4) as usize) {
            rgba.extend_from_slice(row);
            rgba.extend(std::iter::repeat_n(0, ((width - s.width) * 4) as usize));
        }
    }
    Shot { width, height, rgba }
}

fn window_attrs(el: &ActiveEventLoop) -> (WindowAttributes, WindowAttributes) {
    let base = |title: &str| Window::default_attributes().with_title(title).with_resizable(false);
    let monitors: Vec<_> = el.available_monitors().collect();
    for m in &monitors {
        eprintln!("monitor {:?} {:?} at {:?}", m.name(), m.size(), m.position());
    }
    if cfg!(target_os = "linux") && monitors.len() >= 2 {
        let swapped = std::fs::read_to_string("/sys/class/anbernic_misc/lcdswap").is_ok_and(|s| s.trim() == "1");
        let top_name = std::env::var("TWINBIT_TOP").unwrap_or_else(|_| if swapped { "DSI-2" } else { "DSI-1" }.into());
        let top = monitors.iter().find(|m| m.name().as_deref() == Some(&top_name)).unwrap_or(&monitors[0]);
        let bottom = monitors.iter().find(|m| m.name() != top.name()).unwrap_or(&monitors[1]);
        eprintln!("top={:?} bottom={:?}", top.name(), bottom.name());
        let full = |m: &winit::monitor::MonitorHandle| Some(Fullscreen::Borderless(Some(m.clone())));
        (base("twinbit top").with_fullscreen(full(top)), base("twinbit bottom").with_fullscreen(full(bottom)))
    } else {
        let size = LogicalSize::new(512.0, 384.0);
        (
            base("twinbit top").with_inner_size(size).with_position(LogicalPosition::new(200.0, 60.0)),
            base("twinbit bottom").with_inner_size(size).with_position(LogicalPosition::new(200.0, 60.0 + 384.0 + 36.0)),
        )
    }
}
