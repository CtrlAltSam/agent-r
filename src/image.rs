use std::fs::File;
use std::path::Path;
use std::time::Duration;

use ::image::codecs::gif::GifDecoder;
use ::image::{AnimationDecoder, Frame, RgbaImage};
use windows::Win32::Foundation::{COLORREF, HANDLE, HWND, POINT, SIZE};
use windows::Win32::Graphics::Gdi::{
    AC_SRC_ALPHA, AC_SRC_OVER, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION,
    CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC,
    HGDIOBJ, ReleaseDC, SelectObject,
};
use windows::Win32::UI::WindowsAndMessaging::{UpdateLayeredWindow, ULW_ALPHA};

fn scale_dimensions(width: u32, height: u32, max_size: u32) -> (u32, u32) {
    if width <= max_size && height <= max_size {
        return (width, height);
    }

    let ratio = (width as f32).max(height as f32) / max_size as f32;
    ((width as f32 / ratio) as u32, (height as f32 / ratio) as u32)
}

fn scale_frame(frame: &RgbaImage, width: u32, height: u32) -> RgbaImage {
    if frame.width() == width && frame.height() == height {
        return frame.clone();
    }

    ::image::imageops::resize(frame, width, height, ::image::imageops::FilterType::Lanczos3)
}

pub fn load_image_frames(path: &Path, max_size: u32) -> (Vec<RgbaImage>, u32, u32, Vec<Duration>) {
    if path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("gif")) {
        let decoder = GifDecoder::new(File::open(path).expect("Failed to open GIF"))
            .expect("Failed to decode GIF");
        let frames = decoder
            .into_frames()
            .collect_frames()
            .expect("Failed to collect GIF frames");

        let first = frames.first().expect("GIF contained no frames");
        let (width, height) = scale_dimensions(first.buffer().width(), first.buffer().height(), max_size);

        let images = frames
            .iter()
            .map(Frame::buffer)
            .map(|frame| scale_frame(frame, width, height))
            .collect::<Vec<_>>();

        let durations = frames
            .iter()
            .map(|frame| {
                let duration = Duration::from(frame.delay());
                if duration.is_zero() {
                    Duration::from_millis(100)
                } else {
                    duration
                }
            })
            .collect::<Vec<_>>();

        return (images, width, height, durations);
    }

    let image = ::image::open(path)
        .expect("Failed to read image")
        .to_rgba8();
    let (width, height) = scale_dimensions(image.width(), image.height(), max_size);
    let scaled = scale_frame(&image, width, height);
    (vec![scaled], width, height, vec![Duration::from_millis(100)])
}

pub fn render_layered_window(hwnd: HWND, image: &RgbaImage, position: POINT) {
    let size = SIZE {
        cx: image.width() as i32,
        cy: image.height() as i32,
    };
    let src_point = POINT { x: 0, y: 0 };
    let blend = BLENDFUNCTION {
        BlendOp: AC_SRC_OVER as u8,
        BlendFlags: 0,
        SourceConstantAlpha: 255,
        AlphaFormat: AC_SRC_ALPHA as u8,
    };

    let mut bitmap_info = BITMAPINFO::default();
    bitmap_info.bmiHeader = BITMAPINFOHEADER {
        biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
        biWidth: image.width() as i32,
        biHeight: -(image.height() as i32),
        biPlanes: 1,
        biBitCount: 32,
        biCompression: BI_RGB.0,
        ..Default::default()
    };

    unsafe {
        let screen_dc = GetDC(HWND(0));
        let memory_dc = CreateCompatibleDC(screen_dc);
        let mut bits = std::ptr::null_mut();
        let bitmap = CreateDIBSection(
            screen_dc,
            &bitmap_info,
            DIB_RGB_COLORS,
            &mut bits,
            HANDLE(0),
            0,
        )
        .expect("Failed to create DIB section");

        let old_bitmap = SelectObject(memory_dc, HGDIOBJ(bitmap.0));

        let output = std::slice::from_raw_parts_mut(
            bits as *mut u8,
            (image.width() * image.height() * 4) as usize,
        );

        for (src, dst) in image.pixels().zip(output.chunks_exact_mut(4)) {
            let alpha = src[3] as u16;
            dst[0] = ((src[2] as u16 * alpha) / 255) as u8;
            dst[1] = ((src[1] as u16 * alpha) / 255) as u8;
            dst[2] = ((src[0] as u16 * alpha) / 255) as u8;
            dst[3] = src[3];
        }

        UpdateLayeredWindow(
            hwnd,
            screen_dc,
            Some(&position),
            Some(&size),
            memory_dc,
            Some(&src_point),
            COLORREF(0),
            Some(&blend),
            ULW_ALPHA,
        )
        .expect("Failed to update layered window");

        SelectObject(memory_dc, old_bitmap);
        DeleteObject(bitmap);
        DeleteDC(memory_dc);
        ReleaseDC(HWND(0), screen_dc);
    }
}
