use std::fs::File;

use wayland_client::{
    Connection, Dispatch, EventQueue, QueueHandle,
    protocol::{wl_buffer::WlBuffer, wl_surface::WlSurface},
};
use wayland_protocols::wp::viewporter::client::wp_viewport::WpViewport;
use wayland_protocols_wlr::layer_shell::v1::client::{
    zwlr_layer_shell_v1::Layer,
    zwlr_layer_surface_v1::{self, Anchor, KeyboardInteractivity, ZwlrLayerSurfaceV1},
};

use crate::app::App;

#[derive(Clone, Copy, Default)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

pub struct Overlay {
    pub surface: WlSurface,
    pub viewport: WpViewport,
    pub width: u32,
    pub height: u32,
    pub configured: bool,
    pub view: Rect,
    pub target: Rect,
    pub animating: bool,
    pub last_tick: Option<u32>,
}

impl Dispatch<ZwlrLayerSurfaceV1, usize> for App {
    fn event(
        state: &mut Self,
        layer_surface: &ZwlrLayerSurfaceV1,
        event: zwlr_layer_surface_v1::Event,
        data: &usize,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        match event {
            zwlr_layer_surface_v1::Event::Configure { serial, width, height } => {
                layer_surface.ack_configure(serial);
                let overlay = &mut state.overlays[*data];
                overlay.width = width;
                overlay.height = height;
                overlay.configured = true;
            }
            zwlr_layer_surface_v1::Event::Closed => {
                state.running = false;
            }
            _ => {}
        }
    }
}

pub fn create_overlays(
    app: &mut App,
    queue: &mut EventQueue<App>,
    qh: &QueueHandle<App>,
    buffers: &[(File, WlBuffer)],
) -> Result<(), Box<dyn std::error::Error>> {
    let compositor = app.compositor.clone().ok_or("missing wl_compositor")?;
    let layer_shell = app.layer_shell.clone().ok_or("missing zwlr_layer_shell_v1")?;
    let viewporter = app.viewporter.clone().ok_or("missing wp_viewporter")?;

    for (index, info) in app.outputs.iter().enumerate() {
        let surface = compositor.create_surface(qh, ());
        let layer_surface = layer_shell.get_layer_surface(
            &surface,
            Some(&info.output),
            Layer::Overlay,
            "wayzoomy".to_string(),
            qh,
            index,
        );
        layer_surface.set_anchor(Anchor::Top | Anchor::Bottom | Anchor::Left | Anchor::Right);
        layer_surface.set_exclusive_zone(-1);
        layer_surface.set_size(0, 0);
        layer_surface.set_keyboard_interactivity(KeyboardInteractivity::Exclusive);

        let viewport = viewporter.get_viewport(&surface, qh, ());
        surface.commit();

        let buffer_info = app.frames[index].buffer_info.unwrap();
        let full = Rect {
            x: 0.0,
            y: 0.0,
            w: buffer_info.width as f64,
            h: buffer_info.height as f64,
        };
        app.overlays.push(Overlay {
            surface,
            viewport,
            width: 0,
            height: 0,
            configured: false,
            view: full,
            target: full,
            animating: false,
            last_tick: None,
        });
    }

    while !app.overlays.iter().all(|overlay| overlay.configured) {
        queue.blocking_dispatch(app)?;
    }

    for (index, overlay) in app.overlays.iter().enumerate() {
        let (_file, buffer) = &buffers[index];
        overlay.viewport.set_destination(overlay.width as i32, overlay.height as i32);
        overlay.surface.attach(Some(buffer), 0, 0);
        overlay.surface.damage(0, 0, i32::MAX, i32::MAX);
        overlay.surface.commit();
    }

    Ok(())
}
