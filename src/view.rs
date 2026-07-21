use wayland_client::{
    Connection, Dispatch, QueueHandle,
    protocol::wl_callback::{self, WlCallback},
};

use crate::app::App;

const ANIM_SPEED: f64 = 10.0;

impl App {
    pub fn kick(&mut self, index: usize, qh: &QueueHandle<Self>) {
        let overlay = &mut self.overlays[index];
        if overlay.animating {
            return;
        }
        overlay.animating = true;
        overlay.last_tick = None;
        overlay.surface.frame(qh, index);
        self.apply_view(index);
    }

    fn tick(&mut self, index: usize, time: u32, qh: &QueueHandle<Self>) {
        let info = self.frames[index].buffer_info.unwrap();
        let overlay = &mut self.overlays[index];

        let dt = match overlay.last_tick {
            Some(last) => (time.wrapping_sub(last) as f64 / 1000.0).clamp(0.0, 0.1),
            None => 1.0 / 60.0,
        };
        overlay.last_tick = Some(time);

        let alpha = 1.0 - (-dt * ANIM_SPEED).exp();
        overlay.zoom += (overlay.target_zoom - overlay.zoom) * alpha;
        overlay.src_x += (overlay.target_src_x - overlay.src_x) * alpha;
        overlay.src_y += (overlay.target_src_y - overlay.src_y) * alpha;

        let sw = info.width as f64 / overlay.zoom;
        let sh = info.height as f64 / overlay.zoom;
        overlay.src_x = overlay.src_x.clamp(0.0, info.width as f64 - sw);
        overlay.src_y = overlay.src_y.clamp(0.0, info.height as f64 - sh);

        let done = (overlay.target_zoom - overlay.zoom).abs() < 5e-4
            && (overlay.target_src_x - overlay.src_x).abs() < 0.05
            && (overlay.target_src_y - overlay.src_y).abs() < 0.05;

        if done {
            overlay.zoom = overlay.target_zoom;
            overlay.src_x = overlay.target_src_x;
            overlay.src_y = overlay.target_src_y;
            overlay.animating = false;
            overlay.last_tick = None;
        } else {
            overlay.surface.frame(qh, index);
        }

        self.apply_view(index);
    }

    fn apply_view(&self, index: usize) {
        let overlay = &self.overlays[index];
        let info = self.frames[index].buffer_info.unwrap();
        overlay.viewport.set_source(
            overlay.src_x,
            overlay.src_y,
            info.width as f64 / overlay.zoom,
            info.height as f64 / overlay.zoom,
        );
        overlay.surface.damage(0, 0, i32::MAX, i32::MAX);
        overlay.surface.commit();
    }
}

impl Dispatch<WlCallback, usize> for App {
    fn event(
        state: &mut Self,
        _callback: &WlCallback,
        event: wl_callback::Event,
        data: &usize,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_callback::Event::Done { callback_data } = event {
            state.tick(*data, callback_data, qh);
        }
    }
}
