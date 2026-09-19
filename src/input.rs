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
                if state.dragging && state.focus.is_some() {
                    let (last_x, last_y) = state.pointer_pos;
                    let (dx, dy) = (surface_x - last_x, surface_y - last_y);
                    let (cw, ch) = (state.canvas.w, state.canvas.h);
                    let (bw, bh) = (state.bbox.w, state.bbox.h);
                    let pairs = [
                        (&mut state.view, 1.0 - DRAG_EASE),
                        (&mut state.target, 1.0),
                    ];
                    for (rect, share) in pairs {
                        rect.x = (rect.x - dx * (rect.w / bw) * share).clamp(0.0, cw - rect.w);
                        rect.y = (rect.y - dy * (rect.h / bh) * share).clamp(0.0, ch - rect.h);
                    }
                    state.kick(qh);
                }
                state.pointer_pos = (surface_x, surface_y);
            }
            wl_pointer::Event::Button { button, state: WEnum::Value(button_state), .. } => {
                let pressed = button_state == wl_pointer::ButtonState::Pressed;
                if button == BTN_LEFT {
                    state.dragging = pressed;
                } else if pressed {
                    state.exit_armed = true;
                } else if state.exit_armed {
                    state.running = false;
                }
            }
            wl_pointer::Event::Axis {
                axis: WEnum::Value(wl_pointer::Axis::VerticalScroll),
                value,
                ..
            } => {
                let Some(index) = state.focus else { return };
                let output = &state.outputs[index];
                let gx = ((output.x as f64 - state.bbox.x + state.pointer_pos.0) / state.bbox.w)
                    .clamp(0.0, 1.0);
                let gy = ((output.y as f64 - state.bbox.y + state.pointer_pos.1) / state.bbox.h)
                    .clamp(0.0, 1.0);
                let (cw, ch) = (state.canvas.w, state.canvas.h);

                let old = state.target;
                let w = (old.w * ZOOM_STEP.powf(value / 15.0)).clamp(cw / MAX_ZOOM, cw);
                let h = w * ch / cw;

                state.target = Rect {
                    x: (old.x + gx * (old.w - w)).clamp(0.0, cw - w),
                    y: (old.y + gy * (old.h - h)).clamp(0.0, ch - h),
                    w,
                    h,
                };
                state.kick(qh);
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
            state: WEnum::Value(wl_keyboard::KeyState::Pressed),
            ..
        } = event
        {
            state.running = false;
        }
    }
}
