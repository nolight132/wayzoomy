use wayland_client::{
    Connection, Dispatch, QueueHandle, WEnum, delegate_noop,
    protocol::{
        wl_buffer::WlBuffer,
        wl_compositor::WlCompositor,
        wl_keyboard::WlKeyboard,
        wl_output::{self, WlOutput},
        wl_pointer::WlPointer,
        wl_registry::{self, WlRegistry},
        wl_seat::WlSeat,
        wl_shm::WlShm,
        wl_shm_pool::WlShmPool,
        wl_surface::WlSurface,
    },
};
use wayland_protocols::wp::cursor_shape::v1::client::{
    wp_cursor_shape_device_v1::WpCursorShapeDeviceV1,
    wp_cursor_shape_manager_v1::WpCursorShapeManagerV1,
};
use wayland_protocols::wp::viewporter::client::{
    wp_viewport::WpViewport,
    wp_viewporter::WpViewporter,
};
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1::ZwlrLayerShellV1;
use wayland_protocols_wlr::screencopy::v1::client::zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1;

use crate::{
    capture::FrameState,
    overlay::{Overlay, Rect},
};

#[allow(dead_code)]
pub struct OutputInfo {
    pub output: WlOutput,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub scale: i32,
    pub name: String,
}

#[derive(Default)]
pub struct App {
    pub shm: Option<WlShm>,
    pub compositor: Option<WlCompositor>,
    pub screencopy: Option<ZwlrScreencopyManagerV1>,
    pub layer_shell: Option<ZwlrLayerShellV1>,
    pub viewporter: Option<WpViewporter>,
    pub seat: Option<WlSeat>,
    pub pointer: Option<WlPointer>,
    pub keyboard: Option<WlKeyboard>,
    pub cursor_shape_manager: Option<WpCursorShapeManagerV1>,
    pub cursor_shape: Option<WpCursorShapeDeviceV1>,
    pub focus: Option<usize>,
    pub pointer_pos: (f64, f64),
    pub dragging: bool,
    pub running: bool,
    pub bbox: Rect,
    pub canvas: Rect,
    pub view: Rect,
    pub target: Rect,
    pub animating: bool,
    pub last_tick: Option<u32>,
    pub outputs: Vec<OutputInfo>,
    pub frames: Vec<FrameState>,
    pub overlays: Vec<Overlay>,
}

impl Dispatch<WlRegistry, ()> for App {
    fn event(
        state: &mut Self,
        registry: &WlRegistry,
        event: wl_registry::Event,
        _data: &(),
        _conn: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        let wl_registry::Event::Global { name, interface, version } = event else {
            return;
        };

        match interface.as_str() {
            "wl_shm" => {
                state.shm = Some(registry.bind(name, 1, qh, ()));
            }
            "wl_compositor" => {
                state.compositor = Some(registry.bind(name, version.min(4), qh, ()));
            }
            "wl_output" => {
                let index = state.outputs.len();
                let output = registry.bind(name, version.min(4), qh, index);
                state.outputs.push(OutputInfo {
                    output,
                    x: 0,
                    y: 0,
                    width: 0,
                    height: 0,
                    scale: 1,
                    name: String::new(),
                });
            }
            "zwlr_screencopy_manager_v1" => {
                state.screencopy = Some(registry.bind(name, 3, qh, ()));
            }
            "zwlr_layer_shell_v1" => {
                state.layer_shell = Some(registry.bind(name, version.min(4), qh, ()));
            }
            "wp_viewporter" => {
                state.viewporter = Some(registry.bind(name, 1, qh, ()));
            }
            "wl_seat" => {
                state.seat = Some(registry.bind(name, version.min(5), qh, ()));
            }
            "wp_cursor_shape_manager_v1" => {
                state.cursor_shape_manager = Some(registry.bind(name, 1, qh, ()));
            }
            _ => {}
        }
    }
}

impl Dispatch<WlOutput, usize> for App {
    fn event(
        state: &mut Self,
        _output: &WlOutput,
        event: wl_output::Event,
        data: &usize,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        let info = &mut state.outputs[*data];
        match event {
            wl_output::Event::Geometry { x, y, .. } => {
                info.x = x;
                info.y = y;
            }
            wl_output::Event::Mode { flags: WEnum::Value(flags), width, height, .. }
                if flags.contains(wl_output::Mode::Current) =>
            {
                info.width = width;
                info.height = height;
            }
            wl_output::Event::Scale { factor } => {
                info.scale = factor;
            }
            wl_output::Event::Name { name } => {
                info.name = name;
            }
            _ => {}
        }
    }
}

delegate_noop!(App: ignore WlShm);
delegate_noop!(App: ignore WlShmPool);
delegate_noop!(App: ignore WlBuffer);
delegate_noop!(App: ignore WlCompositor);
delegate_noop!(App: ignore WlSurface);
delegate_noop!(App: ignore ZwlrScreencopyManagerV1);
delegate_noop!(App: ignore ZwlrLayerShellV1);
delegate_noop!(App: ignore WpViewporter);
delegate_noop!(App: ignore WpViewport);
delegate_noop!(App: ignore WpCursorShapeManagerV1);
delegate_noop!(App: ignore WpCursorShapeDeviceV1);

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let conn = Connection::connect_to_env()?;
    let mut queue = conn.new_event_queue();
    let qh = queue.handle();

    let _registry = conn.display().get_registry(&qh, ());

    let mut app = App { running: true, ..Default::default() };
    queue.roundtrip(&mut app)?;
    queue.roundtrip(&mut app)?;

    let buffers = crate::capture::capture_outputs(&mut app, &mut queue, &qh)?;
    crate::overlay::create_overlays(&mut app, &mut queue, &qh)?;
    let _canvas = crate::canvas::compose(&mut app, &qh, buffers)?;

    while app.running {
        queue.blocking_dispatch(&mut app)?;
    }

    Ok(())
}
