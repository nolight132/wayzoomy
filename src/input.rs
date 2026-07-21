use wayland_client::{
    Connection, Dispatch, QueueHandle, WEnum,
    protocol::{
        wl_keyboard::{self, WlKeyboard},
        wl_pointer::{self, WlPointer},
        wl_seat::{self, WlSeat},
    },
};
use wayland_protocols::wp::cursor_shape::v1::client::wp_cursor_shape_device_v1::Shape;

use crate::{app::App, overlay::Rect};

const BTN_LEFT: u32 = 0x110;
const KEY_ESC: u32 = 1;
const ZOOM_STEP: f64 = 1.25;
const MAX_ZOOM: f64 = 20.0;
const DRAG_EASE: f64 = 0.4;

impl Dispatch<WlSeat, ()> for App {
    fn event(
        state: &mut Self,
        seat: &WlSeat,
        event: wl_seat::Event,
        _data: &(),
        _conn: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        let wl_seat::Event::Capabilities { capabilities: WEnum::Value(caps) } = event else {
            return;
        };
        if caps.contains(wl_seat::Capability::Pointer) && state.pointer.is_none() {
            let pointer = seat.get_pointer(qh, ());
            if let Some(manager) = &state.cursor_shape_manager {
                state.cursor_shape = Some(manager.get_pointer(&pointer, qh, ()));
            }
            state.pointer = Some(pointer);
        }
        if caps.contains(wl_seat::Capability::Keyboard) && state.keyboard.is_none() {
            state.keyboard = Some(seat.get_keyboard(qh, ()));
        }
    }
}

impl Dispatch<WlPointer, ()> for App {
    fn event(
        state: &mut Self,
        _pointer: &WlPointer,
        event: wl_pointer::Event,
        _data: &(),
        _conn: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        match event {
            wl_pointer::Event::Enter { serial, surface, surface_x, surface_y } => {
                state.focus = state.overlays.iter().position(|o| o.surface == surface);
                state.pointer_pos = (surface_x, surface_y);
                if let Some(device) = &state.cursor_shape {
                    device.set_shape(serial, Shape::Crosshair);
                }
            }
            wl_pointer::Event::Leave { .. } => {
                state.focus = None;
                state.dragging = false;
            }
            wl_pointer::Event::Motion { surface_x, surface_y, .. } => {
                if state.dragging {
                    if let Some(index) = state.focus {
                        let (last_x, last_y) = state.pointer_pos;
                        let (dx, dy) = (surface_x - last_x, surface_y - last_y);
                        let info = state.frames[index].buffer_info.unwrap();
                        let (buf_w, buf_h) = (info.width as f64, info.height as f64);
                        let overlay = &mut state.overlays[index];
                        let width = overlay.width as f64;
                        let pairs = [
                            (&mut overlay.view, 1.0 - DRAG_EASE),
                            (&mut overlay.target, 1.0),
                        ];
                        for (rect, share) in pairs {
                            let per_px = rect.w / width;
                            rect.x = (rect.x - dx * per_px * share).clamp(0.0, buf_w - rect.w);
                            rect.y = (rect.y - dy * per_px * share).clamp(0.0, buf_h - rect.h);
                        }
                        state.kick(index, qh);
                    }
                }
                state.pointer_pos = (surface_x, surface_y);
            }
            wl_pointer::Event::Button { button, state: WEnum::Value(button_state), .. } => {
                if button == BTN_LEFT {
                    state.dragging = button_state == wl_pointer::ButtonState::Pressed;
                }
            }
            wl_pointer::Event::Axis {
                axis: WEnum::Value(wl_pointer::Axis::VerticalScroll),
                value,
                ..
            } => {
                let Some(index) = state.focus else { return };
                let info = state.frames[index].buffer_info.unwrap();
                let overlay = &mut state.overlays[index];
                let (buf_w, buf_h) = (info.width as f64, info.height as f64);

                let old = overlay.target;
                let w = (old.w * ZOOM_STEP.powf(value / 15.0)).clamp(buf_w / MAX_ZOOM, buf_w);
                let h = w * buf_h / buf_w;
                let fx = (state.pointer_pos.0 / overlay.width as f64).clamp(0.0, 1.0);
                let fy = (state.pointer_pos.1 / overlay.height as f64).clamp(0.0, 1.0);

                overlay.target = Rect {
                    x: (old.x + fx * (old.w - w)).clamp(0.0, buf_w - w),
                    y: (old.y + fy * (old.h - h)).clamp(0.0, buf_h - h),
                    w,
                    h,
                };
                state.kick(index, qh);
            }
            _ => {}
        }
    }
}

impl Dispatch<WlKeyboard, ()> for App {
    fn event(
        state: &mut Self,
        _keyboard: &WlKeyboard,
        event: wl_keyboard::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        if let wl_keyboard::Event::Key {
            key,
            state: WEnum::Value(wl_keyboard::KeyState::Pressed),
            ..
        } = event
        {
            if key == KEY_ESC {
                state.running = false;
            }
        }
    }
}
