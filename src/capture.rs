use std::{fs::File, io::Write, os::fd::AsFd, println, write};

use wayland_client::{
    Connection, Dispatch, QueueHandle, WEnum, delegate_noop,
    protocol::{
        wl_buffer::WlBuffer,
        wl_output::WlOutput,
        wl_registry::{self, WlRegistry},
        wl_shm::{self, WlShm},
        wl_shm_pool::WlShmPool,
    },
};
use wayland_protocols_wlr::screencopy::v1::client::{
    zwlr_screencopy_frame_v1::{self, ZwlrScreencopyFrameV1},
    zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1,
};

struct BufferInfo {
    format: wl_shm::Format,
    width: u32,
    height: u32,
    stride: u32,
}

#[derive(Default)]
struct App {
    shm: Option<WlShm>,
    outputs: Vec<WlOutput>,
    screencopy: Option<ZwlrScreencopyManagerV1>,
    buffer_info: Option<BufferInfo>,
    buffer_done: bool,
    y_invert: bool,
    ready: bool,
    failed: bool,
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
            "wl_output" => {
                state.outputs.push(registry.bind(name, version.min(4), qh, ()));
            }
            "zwlr_screencopy_manager_v1" => {
                state.screencopy = Some(registry.bind(name, 3, qh, ()));
            }
            _ => {}
        }
    }
}

impl Dispatch<ZwlrScreencopyFrameV1, ()> for App {
    fn event(
        state: &mut Self,
        _frame: &ZwlrScreencopyFrameV1,
        event: zwlr_screencopy_frame_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        match event {
            zwlr_screencopy_frame_v1::Event::Buffer {
                format: WEnum::Value(format),
                width,
                height,
                stride,
            } => {
                state.buffer_info = Some(BufferInfo { format, width, height, stride });
            }
            zwlr_screencopy_frame_v1::Event::BufferDone => {
                state.buffer_done = true;
            }
            zwlr_screencopy_frame_v1::Event::Flags { flags: WEnum::Value(flags) } => {
                state.y_invert = flags.contains(zwlr_screencopy_frame_v1::Flags::YInvert);
            }
            zwlr_screencopy_frame_v1::Event::Ready { .. } => {
                state.ready = true;
            }
            zwlr_screencopy_frame_v1::Event::Failed => {
                state.failed = true;
            }
            _ => {}
        }
    }
}

delegate_noop!(App: ignore WlShm);
delegate_noop!(App: ignore WlOutput);
delegate_noop!(App: ignore WlShmPool);
delegate_noop!(App: ignore WlBuffer);
delegate_noop!(App: ignore ZwlrScreencopyManagerV1);

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let conn = Connection::connect_to_env()?;
    let mut event_queue = conn.new_event_queue();
    let qh = event_queue.handle();

    let _registry = conn.display().get_registry(&qh, ());

    let mut app = App::default();
    event_queue.roundtrip(&mut app)?;

    let screencopy = app.screencopy.clone().ok_or("compositor lacks zwlr_screencopy_manager_v1")?;
    let shm = app.shm.clone().ok_or("compositor lacks wl_shm")?;

    println!("found {} output(s), capturing the first", app.outputs.len());
    let frame = screencopy.capture_output(0, &app.outputs[0], &qh, ());

    while !app.buffer_done {
        event_queue.blocking_dispatch(&mut app)?;
    }

    let info = app.buffer_info.as_ref().ok_or("no usable shm buffer format offered")?;
    if !matches!(info.format, wl_shm::Format::Xrgb8888 | wl_shm::Format::Argb8888) {
        return Err(format!("unhandled pixel format {:?}", info.format).into());
    }
    let size = info.stride as usize * info.height as usize;

    let memfd = rustix::fs::memfd_create("wayzoomy-shm", rustix::fs::MemfdFlags::CLOEXEC)?;
    let file = File::from(memfd);
    file.set_len(size as u64)?;

    let pool = shm.create_pool(file.as_fd(), size as i32, &qh, ());
    let buffer = pool.create_buffer(
        0,
        info.width as i32,
        info.height as i32,
        info.stride as i32,
        info.format,
        &qh,
        (),
    );

    frame.copy(&buffer);

    while !app.ready && !app.failed {
        event_queue.blocking_dispatch(&mut app)?;
    }
    if app.failed {
        return Err("compositor reported capture failure".into());
    }

    let mmap = unsafe { memmap2::Mmap::map(&file)? };
    let info = app.buffer_info.as_ref().unwrap();
    write_ppm("screenshot.ppm", &mmap, info, app.y_invert)?;
    println!("wrote screenshot.ppm ({}x{})", info.width, info.height);

    frame.destroy();
    buffer.destroy();
    pool.destroy();

    Ok(())
}

fn write_ppm(
    path: &str,
    pixels: &[u8],
    info: &BufferInfo,
    y_invert: bool,
) -> std::io::Result<()> {
    let mut out = std::io::BufWriter::new(File::create(path)?);
    write!(out, "P6\n{} {}\n255\n", info.width, info.height)?;

    let rows: Box<dyn Iterator<Item = u32>> = if y_invert {
        Box::new((0..info.height).rev())
    } else {
        Box::new(0..info.height)
    };

    for y in rows {
        let row = &pixels[(y * info.stride) as usize..];
        for x in 0..info.width as usize {
            let px = &row[x * 4..];
            // XRGB8888 is a little-endian u32: bytes are [B, G, R, X]
            out.write_all(&[px[2], px[1], px[0]])?;
        }
    }

    out.flush()
}
