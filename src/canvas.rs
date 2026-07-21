use std::{fs::File, os::fd::AsFd};

use wayland_client::{
    QueueHandle,
    protocol::{wl_buffer::WlBuffer, wl_shm},
};

use crate::{app::App, overlay::Rect};

pub fn compose(
    app: &mut App,
    qh: &QueueHandle<App>,
    buffers: Vec<(File, WlBuffer)>,
) -> Result<(File, WlBuffer), Box<dyn std::error::Error>> {
    let shm = app.shm.clone().ok_or("missing wl_shm")?;

    let bx = app.outputs.iter().map(|o| o.x as f64).fold(f64::INFINITY, f64::min);
    let by = app.outputs.iter().map(|o| o.y as f64).fold(f64::INFINITY, f64::min);
    let mut bw = 0.0f64;
    let mut bh = 0.0f64;
    for (index, output) in app.outputs.iter().enumerate() {
        bw = bw.max(output.x as f64 - bx + app.overlays[index].width as f64);
        bh = bh.max(output.y as f64 - by + app.overlays[index].height as f64);
    }
    app.bbox = Rect { x: bx, y: by, w: bw, h: bh };

    let first = app.frames[0].buffer_info.unwrap();
    let scale = first.width as f64 / app.overlays[0].width as f64;

    let mut placements = Vec::new();
    let mut canvas_w = 0u32;
    let mut canvas_h = 0u32;
    for (index, output) in app.outputs.iter().enumerate() {
        let info = app.frames[index].buffer_info.unwrap();
        let px = ((output.x as f64 - bx) * scale).round() as u32;
        let py = ((output.y as f64 - by) * scale).round() as u32;
        canvas_w = canvas_w.max(px + info.width);
        canvas_h = canvas_h.max(py + info.height);
        placements.push((px, py, info));
    }

    let stride = canvas_w * 4;
    let size = stride as usize * canvas_h as usize;
    let memfd = rustix::fs::memfd_create("wayzoomy-canvas", rustix::fs::MemfdFlags::CLOEXEC)?;
    let file = File::from(memfd);
    file.set_len(size as u64)?;
    let mut canvas = unsafe { memmap2::MmapMut::map_mut(&file)? };

    for (index, (src_file, src_buffer)) in buffers.iter().enumerate() {
        let (px, py, info) = placements[index];
        let src = unsafe { memmap2::Mmap::map(src_file)? };
        for row in 0..info.height {
            let soff = (row * info.stride) as usize;
            let doff = ((py + row) * stride + px * 4) as usize;
            let len = (info.width * 4) as usize;
            canvas[doff..doff + len].copy_from_slice(&src[soff..soff + len]);
        }
        src_buffer.destroy();
    }

    let pool = shm.create_pool(file.as_fd(), size as i32, qh, ());
    let buffer = pool.create_buffer(
        0,
        canvas_w as i32,
        canvas_h as i32,
        stride as i32,
        wl_shm::Format::Xrgb8888,
        qh,
        (),
    );
    pool.destroy();

    let full = Rect { x: 0.0, y: 0.0, w: canvas_w as f64, h: canvas_h as f64 };
    app.canvas = full;
    app.view = full;
    app.target = full;

    for overlay in &app.overlays {
        overlay.viewport.set_destination(overlay.width as i32, overlay.height as i32);
        overlay.surface.attach(Some(&buffer), 0, 0);
    }
    app.apply_view();

    Ok((file, buffer))
}
