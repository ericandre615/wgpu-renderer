#[cfg(target_arch="wasm32")]
use wasm_bindgen::prelude::*;

use std::rc::Rc;

use winit::{
    event::*,
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

pub mod app;
pub mod render;
mod resources;
pub mod texture;
pub mod camera;
pub mod instance;
pub mod model;
pub mod light;
pub mod primitives;

use crate::app::App;

fn initialize_logger() {
    cfg_if::cfg_if! {
        if #[cfg(target_arch = "wasm32")] {
            std::panic::set_hook(Box::new(console_error_panic_hook::hook));
            console_log::init_with_level(log::Level::Warn).expect("Couldn't initialize Logger")
        } else {
            env_logger::init();
        }
    }
}

#[cfg_attr(target_arch="wasm32", wasm_bindgen(start))]
pub async fn run() {
    initialize_logger();
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new().build(&event_loop).unwrap();

    #[cfg(target_arch = "wasm32")]
    let get_window_size = || {
        // TODO: not sure how to get scrollbar dimensions
        let scrollbar_offset = 0.0; // 30.0;
        let browser_window = web_sys::window().unwrap();
        // `inner_width` corresponds to the browser's `self.innerWidth` function, which are in;
        // logical, not physical pixels
        winit::dpi::LogicalSize::new(
            browser_window.inner_width().unwrap().as_f64().unwrap() - scrollbar_offset,
            browser_window.inner_height().unwrap().as_f64().unwrap() - scrollbar_offset,
        )
    };

    let winit_window = Rc::new(window);

    #[cfg(target_arch = "wasm32")]
    {
        // let winit_window = Rc::new(window);
        let winit_window = winit_window.clone();
        // Winit prevents sizing with CSS. We have to set the size manually on the web
        use winit::dpi::PhysicalSize;
        // window.set_inner_size(PhysicalSize::new(450, 400));
        winit_window.set_inner_size(get_window_size());

        use winit::platform::web::WindowExtWebSys;
        let browser_window = web_sys::window().unwrap();
            // .and_then(|win| win.document())
            browser_window.document().and_then(|doc| {
                let dst = doc.get_element_by_id("wasm-target")?;
                let canvas = web_sys::Element::from(winit_window.canvas());
                canvas.set_id("wgpu-canvas");
                dst.append_child(&canvas).ok()?;

                Some(())
            })
            .expect("Couldn't append canvas to document body");

        // create a browser event listener for resizing on web
        let resize_handler = wasm_bindgen::closure::Closure::wrap(
            Box::new(move |e: web_sys::Event| {
                let size = get_window_size();
                winit_window.set_inner_size(size)
            }
        ) as Box<dyn FnMut(_)>);

        browser_window.add_event_listener_with_callback(
            "resize", resize_handler.as_ref().unchecked_ref()
        ).unwrap();

        resize_handler.forget();
    }

    let winit_window = winit_window.clone();

    let mut app = App::new(winit_window).await;

    let mut last_render_time = instant::Instant::now();

    // Opens window and starts processing events
    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::DeviceEvent {
                event: DeviceEvent::MouseMotion { delta, },
                .. // we're not using device_id currently
            } => if app.mouse_pressed {
                app.camera_controller.process_mouse(delta.0, delta.1)
            },
            Event::WindowEvent {
                ref event,
                window_id,
            } if window_id == app.window().id() => if !app.input(event) {
                match event {
                    #[cfg(not(target_arch="wasm32"))]
                    WindowEvent::CloseRequested | WindowEvent::KeyboardInput {
                        input: KeyboardInput {
                            state: ElementState::Pressed,
                            virtual_keycode: Some(VirtualKeyCode::Escape),
                            ..
                        },
                        ..
                    } => *control_flow = ControlFlow::Exit,
                    WindowEvent::Resized(physical_size) => {
                        app.resize(*physical_size);
                    },
                    WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                        app.resize(**new_inner_size);
                    },
                    // WindowEvent::CursorMoved {
                    //     device_id,
                    //     position,
                    //     ..
                    // } => {
                    //     let scale_factor = app.window().scale_factor();
                    //     let logical_position = position.to_logical::<i32>(scale_factor);
                    //     println!("Scale Factor: {} Physical {:?} Logical {:?}", scale_factor, position, logical_position);
                    // },
                    _ => {}
                }
            },
            Event::RedrawRequested(window_id) if window_id == app.window().id() => {
                let now = instant::Instant::now();
                let dt = now - last_render_time;
                last_render_time = now;

                app.update(dt);
                match app.render() {
                    Ok(_) => {},
                    // recongifure surface if lost
                    Err(wgpu::SurfaceError::Lost) => app.resize(app.size),
                    // system out of memory, we should probably quit
                    Err(wgpu::SurfaceError::OutOfMemory) => *control_flow = ControlFlow::Exit,
                    Err(e) => eprintln!("{:?}", e),
                }
            },
            Event::MainEventsCleared => {
                // RedrawRequested will only trigger once, unless we manually request it
                app.window().request_redraw();
            },
            _ => {}
        }
    });
}
