use wayland_client::{
    Connection, Dispatch, QueueHandle, WEnum,
    protocol::{
        wl_pointer::{self, WlPointer},
        wl_seat::{self, WlSeat},
    },
};
use wayland_protocols::wp::cursor_shape::v1::client::wp_cursor_shape_device_v1::Shape;

use crate::app::App;

const BTN_LEFT: u32 = 0x110;
const ZOOM_STEP: f64 = 1.1;
const MAX_ZOOM: f64 = 20.0;

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
                        let info = state.frames[index].buffer_info.unwrap();
                        let overlay = &mut state.overlays[index];
                        let sw = info.width as f64 / overlay.target_zoom;
                        let sh = info.height as f64 / overlay.target_zoom;
                        let per_px = sw / overlay.width as f64;
                        overlay.target_src_x = (overlay.target_src_x
                            - (surface_x - last_x) * per_px)
                            .clamp(0.0, info.width as f64 - sw);
                        overlay.target_src_y = (overlay.target_src_y
                            - (surface_y - last_y) * per_px)
                            .clamp(0.0, info.height as f64 - sh);
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

                let old_sw = buf_w / overlay.target_zoom;
                let old_sh = buf_h / overlay.target_zoom;
                let fx = (state.pointer_pos.0 / overlay.width as f64).clamp(0.0, 1.0);
                let fy = (state.pointer_pos.1 / overlay.height as f64).clamp(0.0, 1.0);

                overlay.target_zoom = (overlay.target_zoom * ZOOM_STEP.powf(-value / 15.0))
                    .clamp(1.0, MAX_ZOOM);

                let new_sw = buf_w / overlay.target_zoom;
                let new_sh = buf_h / overlay.target_zoom;
                overlay.target_src_x =
                    (overlay.target_src_x + fx * (old_sw - new_sw)).clamp(0.0, buf_w - new_sw);
                overlay.target_src_y =
                    (overlay.target_src_y + fy * (old_sh - new_sh)).clamp(0.0, buf_h - new_sh);

                state.kick(index, qh);
            }
            _ => {}
        }
    }
}
