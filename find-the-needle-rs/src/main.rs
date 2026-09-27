// Entry point: winit 0.30 event loop + ApplicationHandler glue.

use find_the_needle::app::App;
use std::sync::Arc;
use winit::application::ApplicationHandler;
use winit::event::DeviceEvent;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

struct AppHandler {
    app: Option<App>,
}

impl ApplicationHandler for AppHandler {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.app.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title("FIND THE NEEDLE - rust+wgpu")
            .with_inner_size(winit::dpi::LogicalSize::new(1280.0f64, 720.0f64));
        let window = Arc::new(
            event_loop
                .create_window(attrs)
                .expect("window creation failed"),
        );
        self.app = Some(App::new(window));
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(a) = self.app.as_mut() else { return };
        match event {
            WindowEvent::CloseRequested => {
                a.quit = true;
            }
            WindowEvent::Resized(size) => {
                a.resize(size.width, size.height);
            }
            WindowEvent::KeyboardInput { event, .. } => {
                a.key(event.physical_key, event.state == winit::event::ElementState::Pressed);
            }
            WindowEvent::MouseInput { state, button, .. } => {
                a.mouse_button(button, state);
            }
            WindowEvent::RedrawRequested => {
                a.redraw();
                if a.quit {
                    event_loop.exit();
                }
            }
            _ => {}
        }
    }

    fn device_event(
        &mut self,
        _loop: &ActiveEventLoop,
        _id: winit::event::DeviceId,
        event: DeviceEvent,
    ) {
        if let DeviceEvent::MouseMotion { delta } = event {
            if let Some(a) = self.app.as_mut() {
                a.mouse_motion(delta.0, delta.1);
            }
        }
    }

    fn about_to_wait(&mut self, _loop: &ActiveEventLoop) {
        if let Some(a) = self.app.as_ref() {
            a.window.request_redraw();
        }
    }
}

fn run_selftest() {
    use find_the_needle::cam::Camera;
    use find_the_needle::hud::Hud;
    use find_the_needle::render::Renderer;
    use find_the_needle::settings::{Quality, Settings};
    use find_the_needle::world::World;
    use glam::Vec3;

    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: None,
        force_fallback_adapter: false,
    }))
    .expect("selftest: no GPU adapter found");
    println!("selftest adapter: {:?}", adapter.get_info());

    let (device, queue) = pollster::block_on(adapter.request_device(
        &wgpu::DeviceDescriptor {
            label: Some("selftest"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            memory_hints: wgpu::MemoryHints::default(),
        },
        None,
    ))
    .expect("selftest: device creation failed");

    let mut settings = Settings::default();
    settings.quality = Quality::Low;
    let world = World::new(0xF1EED, 40_000); // small straw count: validation only
    let mut renderer = Renderer::new(device, queue, None, &adapter, (1280, 720), &world, &settings);

    let mut cam = Camera::new(75.0);
    cam.pos = Vec3::new(0.0, 2.2, 14.0);
    cam.pitch = -0.25;

    let mut hud = Hud::new();
    hud.rect(8.0, 8.0, 140.0, 30.0, [0.0, 0.0, 0.0, 0.6]);
    hud.text(16.0, 15.0, 1.4, [1.0, 1.0, 1.0, 1.0], "SELFTEST");

    renderer.run_selftest(&cam, &hud, &[], &[], &[]);
    println!("SELFTEST OK — every pipeline/bind-group combination validated");
}

fn main() {
    if std::env::args().any(|a| a == "--selftest") {
        run_selftest();
        return;
    }
    let event_loop = EventLoop::builder().build().expect("event loop");
    let mut handler = AppHandler { app: None };
    event_loop.set_control_flow(ControlFlow::Poll);
    event_loop.run_app(&mut handler).expect("event loop failed");
}
