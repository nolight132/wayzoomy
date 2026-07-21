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
        let overlay = &mut self.overlays[index];

        let dt = match overlay.last_tick {
            Some(last) => (time.wrapping_sub(last) as f64 / 1000.0).clamp(0.0, 0.1),
            None => 1.0 / 60.0,
        };
        overlay.last_tick = Some(time);

        let alpha = 1.0 - (-dt * ANIM_SPEED).exp();
        let target = overlay.target;
        let view = &mut overlay.view;
        view.x += (target.x - view.x) * alpha;
        view.y += (target.y - view.y) * alpha;
        view.w += (target.w - view.w) * alpha;
        view.h += (target.h - view.h) * alpha;

        let done = (target.x - view.x).abs() < 0.05
            && (target.y - view.y).abs() < 0.05
            && (target.w - view.w).abs() < 0.05
            && (target.h - view.h).abs() < 0.05;

        if done {
            overlay.view = target;
            overlay.animating = false;
            overlay.last_tick = None;
        } else {
            overlay.surface.frame(qh, index);
        }

        self.apply_view(index);
    }

    fn apply_view(&self, index: usize) {
        let overlay = &self.overlays[index];
        let v = overlay.view;
        overlay.viewport.set_source(v.x, v.y, v.w, v.h);
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
