use std::{fs::File, os::fd::AsFd};

use wayland_client::{
    Connection, Dispatch, EventQueue, QueueHandle, WEnum,
    protocol::{wl_buffer::WlBuffer, wl_shm},
};
use wayland_protocols_wlr::screencopy::v1::client::zwlr_screencopy_frame_v1::{
    self, ZwlrScreencopyFrameV1,
};

use crate::app::App;

#[derive(Clone, Copy)]
pub struct BufferInfo {
    pub format: wl_shm::Format,
    pub width: u32,
    pub height: u32,
    pub stride: u32,
}

#[derive(Default)]
pub struct FrameState {
    pub buffer_info: Option<BufferInfo>,
    pub buffer_done: bool,
    pub ready: bool,
    pub failed: bool,
}

impl Dispatch<ZwlrScreencopyFrameV1, usize> for App {
    fn event(
        state: &mut Self,
        _frame: &ZwlrScreencopyFrameV1,
        event: zwlr_screencopy_frame_v1::Event,
        data: &usize,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        let frame = &mut state.frames[*data];
        match event {
            zwlr_screencopy_frame_v1::Event::Buffer {
                format: WEnum::Value(format),
                width,
                height,
                stride,
            } => {
                frame.buffer_info = Some(BufferInfo { format, width, height, stride });
            }
            zwlr_screencopy_frame_v1::Event::BufferDone => {
                frame.buffer_done = true;
            }
            zwlr_screencopy_frame_v1::Event::Ready { .. } => {
                frame.ready = true;
            }
            zwlr_screencopy_frame_v1::Event::Failed => {
                frame.failed = true;
            }
            _ => {}
        }
    }
}

pub fn capture_outputs(
    app: &mut App,
    queue: &mut EventQueue<App>,
    qh: &QueueHandle<App>,
) -> Result<Vec<(File, WlBuffer)>, Box<dyn std::error::Error>> {
    let shm = app.shm.clone().ok_or("missing wl_shm")?;
    let screencopy = app.screencopy.clone().ok_or("missing zwlr_screencopy_manager_v1")?;

    let mut frames = Vec::new();
    for (index, info) in app.outputs.iter().enumerate() {
        frames.push(screencopy.capture_output(0, &info.output, qh, index));
        app.frames.push(FrameState::default());
    }

    while !app.frames.iter().all(|frame| frame.buffer_done) {
        queue.blocking_dispatch(app)?;
    }

    let mut buffers = Vec::new();
    for (index, frame) in frames.iter().enumerate() {
        let info = app.frames[index].buffer_info.ok_or("no shm format offered")?;
        if !matches!(info.format, wl_shm::Format::Xrgb8888 | wl_shm::Format::Argb8888) {
            return Err(format!("unhandled pixel format {:?}", info.format).into());
        }
        let size = info.stride as usize * info.height as usize;

        let memfd = rustix::fs::memfd_create("wayzoomy-shm", rustix::fs::MemfdFlags::CLOEXEC)?;
        let file = File::from(memfd);
        file.set_len(size as u64)?;

        let pool = shm.create_pool(file.as_fd(), size as i32, qh, ());
        let buffer = pool.create_buffer(
            0,
            info.width as i32,
            info.height as i32,
            info.stride as i32,
            info.format,
            qh,
            (),
        );
        pool.destroy();

        frame.copy(&buffer);
        buffers.push((file, buffer));
    }

    while !app.frames.iter().all(|frame| frame.ready || frame.failed) {
        queue.blocking_dispatch(app)?;
    }

    for (index, frame) in frames.iter().enumerate() {
        if app.frames[index].failed {
            return Err(format!("capture failed on {}", app.outputs[index].name).into());
        }
        frame.destroy();
    }

    Ok(buffers)
}
