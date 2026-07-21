use wayland_client::{
    Connection, Dispatch, QueueHandle,
    protocol::wl_callback::{self, WlCallback},
};

use crate::app::App;

const ANIM_SPEED: f64 = 10.0;

impl App {
    pub fn kick(&mut self, qh: &QueueHandle<Self>) {
        if self.animating {
            return;
        }
        self.animating = true;
        self.last_tick = None;
        self.overlays[0].surface.frame(qh, 0);
        self.apply_view();
    }

    fn tick(&mut self, time: u32, qh: &QueueHandle<Self>) {
        let dt = match self.last_tick {
            Some(last) => (time.wrapping_sub(last) as f64 / 1000.0).clamp(0.0, 0.1),
            None => 1.0 / 60.0,
        };
        self.last_tick = Some(time);

        let alpha = 1.0 - (-dt * ANIM_SPEED).exp();
        let target = self.target;
        let view = &mut self.view;
        view.x += (target.x - view.x) * alpha;
        view.y += (target.y - view.y) * alpha;
        view.w += (target.w - view.w) * alpha;
        view.h += (target.h - view.h) * alpha;

        let done = (target.x - view.x).abs() < 0.05
            && (target.y - view.y).abs() < 0.05
            && (target.w - view.w).abs() < 0.05
            && (target.h - view.h).abs() < 0.05;

        if done {
            self.view = target;
            self.animating = false;
            self.last_tick = None;
        } else {
            self.overlays[0].surface.frame(qh, 0);
        }

        self.apply_view();
    }

    pub fn apply_view(&self) {
        let v = self.view;
        for (index, overlay) in self.overlays.iter().enumerate() {
            let output = &self.outputs[index];
            let fx = (output.x as f64 - self.bbox.x) / self.bbox.w;
            let fy = (output.y as f64 - self.bbox.y) / self.bbox.h;
            let fw = overlay.width as f64 / self.bbox.w;
            let fh = overlay.height as f64 / self.bbox.h;

            let sx = v.x + fx * v.w;
            let sy = v.y + fy * v.h;
            let sw = (fw * v.w).min(self.canvas.w - sx);
            let sh = (fh * v.h).min(self.canvas.h - sy);

            overlay.viewport.set_source(sx, sy, sw, sh);
            overlay.surface.damage(0, 0, i32::MAX, i32::MAX);
            overlay.surface.commit();
        }
    }
}

impl Dispatch<WlCallback, usize> for App {
    fn event(
        state: &mut Self,
        _callback: &WlCallback,
        event: wl_callback::Event,
        _data: &usize,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_callback::Event::Done { callback_data } = event {
            state.tick(callback_data, qh);
        }
    }
}
