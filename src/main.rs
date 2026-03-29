#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use std::path::Path;
use std::time::{Duration, Instant};

use winit::{
    dpi::{LogicalSize, PhysicalPosition},
    event::{ElementState, Event, MouseButton, WindowEvent},
    event_loop::EventLoop,
    keyboard::{KeyCode, PhysicalKey},
    platform::windows::WindowBuilderExtWindows,
    raw_window_handle::{HasWindowHandle, RawWindowHandle},
    window::WindowBuilder,
};

use windows::Win32::Foundation::{HWND, POINT};
use windows::Win32::UI::WindowsAndMessaging::{
    GetWindowLongW, SetWindowLongW, GWL_EXSTYLE,
    WS_EX_APPWINDOW, WS_EX_LAYERED, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
};

mod speech;
use speech::SpeechBubble;

mod image;
mod sensors;
mod tray;

fn is_push_macro_key(event: &winit::event::KeyEvent) -> bool {
    matches!(event.physical_key, PhysicalKey::Code(KeyCode::F8))
}

fn main() {
// need to make driver for accurate readings
//
//    let cpu_temp_message = match sensors::cpu::get_cpu_temp() {
//        Some(temp_c) => format!("Your CPU temp is {:.1}C", temp_c),
//        None => "CPU temp unavailable".to_string(),
//    };

    let gpu_temp_message = match sensors::gpu::gpu::get_gpu_temp() {
        Some(temp_c) => format!("Your GPU temp is {:.1}C", temp_c),
        None => "GPU temp unavailable".to_string(),
    };

    let mother_temp_message = match sensors::motherboard::get_motherboard_temp() {
        Some(temp_c) => format!("Your Motherboard temp is {:.1}C", temp_c),
        None => "Motherboard temp unavailable".to_string(),
    };

    let event_loop = EventLoop::new().unwrap();

    // Try to load GIF first, then PNG
    let image_file = if Path::new("assets/wooper.gif").exists() {
        Path::new("assets/wooper.gif")
    } else {
        Path::new("assets/wooper.png")
    };

    let (frames, img_width, img_height, frame_durations) = image::load_image_frames(image_file, 2000);
    let mut current_frame = 0;
    let mut next_frame_time = Instant::now() + frame_durations[0];
    let mut last_redraw_time = Instant::now();

    let mut speech = SpeechBubble::new("");
    speech.push_message("Hello, I am agent-r, your annoying ass assistant!!!");
    //speech.push_message(cpu_temp_message);
    speech.push_message(gpu_temp_message);
    speech.push_message(mother_temp_message);
    speech.push_message("I will destroy the world!!!");
    speech.push_message("Anyways, how may I help you?");
    let mut cursor_pos: Option<(f32, f32)> = None;
    let mut macro_key_down = false;
    let mut last_macro_push_at: Option<Instant> = None;
    let macro_push_debounce = Duration::from_millis(220);

    let (canvas_width, canvas_height) = speech.canvas_size(img_width, img_height);
    let mut current_canvas_size = (canvas_width, canvas_height);
    let mut current_image_offset = speech.image_offset(img_width, img_height);

    let window = WindowBuilder::new()
        .with_decorations(false) // no border
        .with_transparent(true)
        .with_drag_and_drop(false)
        .with_window_level(winit::window::WindowLevel::AlwaysOnTop)
        .with_inner_size(LogicalSize::new(canvas_width as f64, canvas_height as f64))
        .build(&event_loop)
        .unwrap();

    let hwnd = match window.window_handle().unwrap().as_raw() {
        RawWindowHandle::Win32(handle) => HWND(handle.hwnd.get()),
        _ => panic!("expected Win32 window handle"),
    };

    unsafe {
        tray::install_tray_wndproc(hwnd);

        let style = GetWindowLongW(hwnd, GWL_EXSTYLE);
        SetWindowLongW(
            hwnd,
            GWL_EXSTYLE,
            (style | WS_EX_LAYERED.0 as i32 | WS_EX_TOPMOST.0 as i32 | WS_EX_TOOLWINDOW.0 as i32)
                & !(WS_EX_APPWINDOW.0 as i32),
        );
    }

    let _tray_icon = tray::TrayIcon::create(hwnd);

    if let Ok(position) = window.outer_position() {
        if let Some(composed) = speech.compose(&frames[0]) {
            image::render_layered_window(
                hwnd,
                &composed,
                POINT {
                    x: position.x,
                    y: position.y,
                },
            );
        } else {
            image::render_layered_window(
                hwnd,
                &frames[0],
                POINT {
                    x: position.x,
                    y: position.y,
                },
            );
        }
    }

    event_loop.run(move |event, elwt| {
        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => elwt.exit(),

                WindowEvent::KeyboardInput { event, .. } => {
                    if is_push_macro_key(&event) {
                        match event.state {
                            ElementState::Pressed => {
                                if !macro_key_down {
                                    macro_key_down = true;
                                    let can_push = last_macro_push_at
                                        .map(|t| t.elapsed() >= macro_push_debounce)
                                        .unwrap_or(true);
                                    if can_push {
                                        speech.push_message(
                                            "You pushed the secret button!!!.",
                                        );
                                        last_macro_push_at = Some(Instant::now());
                                    }
                                }
                            }
                            ElementState::Released => {
                                macro_key_down = false;
                            }
                        }
                    }
                }

                WindowEvent::CursorMoved { position, .. } => {
                    cursor_pos = Some((position.x as f32, position.y as f32));
                }

                WindowEvent::MouseInput { state, button, .. } => {
                    if button == MouseButton::Left {
                        match state {
                            ElementState::Pressed => {
                                let clicked_bubble = cursor_pos
                                    .map(|(x, y)| speech.hit_test(x, y, img_width, img_height))
                                    .unwrap_or(false);
                                if clicked_bubble {
                                    if speech.awaiting_advance() {
                                        speech.advance_message();
                                    } else {
                                        speech.boost_once();
                                    }
                                } else {
                                    let _ = window.drag_window();
                                }
                            }
                            ElementState::Released => {
                                speech.set_speed_up(false);
                            }
                        }
                    }
                }

                WindowEvent::Moved(position) => {
                    if let Some(composed) = speech.compose(&frames[current_frame]) {
                        image::render_layered_window(
                            hwnd,
                            &composed,
                            POINT {
                                x: position.x,
                                y: position.y,
                            },
                        );
                    } else {
                        image::render_layered_window(
                            hwnd,
                            &frames[current_frame],
                            POINT {
                                x: position.x,
                                y: position.y,
                            },
                        );
                    }
                }

                WindowEvent::RedrawRequested => {
                    let now = Instant::now();
                    let delta = now.saturating_duration_since(last_redraw_time);
                    last_redraw_time = now;

                    speech.update(delta);

                    if frames.len() > 1 && now >= next_frame_time {
                        current_frame = (current_frame + 1) % frames.len();
                        next_frame_time = now + frame_durations[current_frame];
                    }

                    let desired_canvas_size = speech.canvas_size(img_width, img_height);
                    let desired_image_offset = speech.image_offset(img_width, img_height);
                    if desired_canvas_size != current_canvas_size {
                        if let Ok(position) = window.outer_position() {
                            let anchored_x = position.x
                                + current_image_offset.0 as i32
                                - desired_image_offset.0 as i32;
                            let anchored_y = position.y
                                + current_image_offset.1 as i32
                                - desired_image_offset.1 as i32;

                            let _ = window.request_inner_size(LogicalSize::new(
                                desired_canvas_size.0 as f64,
                                desired_canvas_size.1 as f64,
                            ));
                            window.set_outer_position(PhysicalPosition::new(anchored_x, anchored_y));
                        } else {
                            let _ = window.request_inner_size(LogicalSize::new(
                                desired_canvas_size.0 as f64,
                                desired_canvas_size.1 as f64,
                            ));
                        }

                        current_canvas_size = desired_canvas_size;
                    }
                    current_image_offset = desired_image_offset;

                    if let Ok(position) = window.outer_position() {
                        if let Some(composed) = speech.compose(&frames[current_frame]) {
                            image::render_layered_window(
                                hwnd,
                                &composed,
                                POINT {
                                    x: position.x,
                                    y: position.y,
                                },
                            );
                        } else {
                            image::render_layered_window(
                                hwnd,
                                &frames[current_frame],
                                POINT {
                                    x: position.x,
                                    y: position.y,
                                },
                            );
                        }
                    }
                }

                _ => {}
            },

            Event::AboutToWait => {
                window.request_redraw();
            }

            _ => {}
        }
    }).unwrap();
}