//! Встроенные ресурсы: иконки окна и трея, сохранение снимков.

use std::io::Cursor;

use eframe::egui::{Color32, ColorImage};

const APP_ICON_PNG: &[u8] = include_bytes!("../../assets/app-icon.png");
const LOGO_WIDE_PNG: &[u8] = include_bytes!("../../assets/logo-wide.png");
const TRAY_ICON_ICO: &[u8] = include_bytes!("../../assets/tray.ico");

/// Декодировать PNG в RGBA.
pub fn decode_png(bytes: &[u8]) -> Option<ColorImage> {
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().ok()?;
    let mut buf = vec![0u8; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).ok()?;
    let width = info.width as usize;
    let height = info.height as usize;
    let mut rgba = Vec::with_capacity(width * height * 4);
    match info.color_type {
        png::ColorType::Rgba => rgba.extend_from_slice(&buf[..info.buffer_size()]),
        png::ColorType::Rgb => {
            for chunk in buf[..info.buffer_size()].chunks_exact(3) {
                rgba.extend_from_slice(&[chunk[0], chunk[1], chunk[2], 255]);
            }
        }
        png::ColorType::Grayscale => {
            for &g in &buf[..info.buffer_size()] {
                rgba.extend_from_slice(&[g, g, g, 255]);
            }
        }
        png::ColorType::GrayscaleAlpha => {
            for chunk in buf[..info.buffer_size()].chunks_exact(2) {
                rgba.extend_from_slice(&[chunk[0], chunk[0], chunk[0], chunk[1]]);
            }
        }
        png::ColorType::Indexed => return None,
    }
    let pixels = rgba
        .chunks_exact(4)
        .map(|c| Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3]))
        .collect();
    Some(ColorImage {
        size: [width, height],
        pixels,
    })
}

pub fn window_icon() -> Option<egui::IconData> {
    let image = decode_png(APP_ICON_PNG)?;
    Some(egui::IconData {
        rgba: image.pixels.iter().flat_map(|c| c.to_array()).collect(),
        width: image.size[0] as u32,
        height: image.size[1] as u32,
    })
}

/// Широкий логотип «Zapret Manager» для страницы «О программе».
pub fn logo_wide() -> Option<ColorImage> {
    decode_png(LOGO_WIDE_PNG)
}

pub fn tray_icon_bytes() -> Option<Vec<u8>> {
    Some(TRAY_ICON_ICO.to_vec())
}

/// Сохранить снимок экрана в PNG.
pub fn save_png(path: &str, image: &ColorImage) -> bool {
    let file = match std::fs::File::create(path) {
        Ok(f) => f,
        Err(_) => return false,
    };
    let mut encoder = png::Encoder::new(file, image.size[0] as u32, image.size[1] as u32);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = match encoder.write_header() {
        Ok(w) => w,
        Err(_) => return false,
    };
    let mut data = Vec::with_capacity(image.pixels.len() * 4);
    for pixel in &image.pixels {
        data.extend_from_slice(&pixel.to_array());
    }
    writer.write_image_data(&data).is_ok()
}

use eframe::egui;
