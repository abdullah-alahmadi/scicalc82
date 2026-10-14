//! Embeds the app icon in the Windows executable, so Explorer and shortcuts
//! show it. The .ico is generated from the same code as the window icon.

use std::path::PathBuf;
use std::{env, fs};

#[path = "src/icon_pixels.rs"]
mod icon_pixels;

/// Icon sizes Windows picks from for different views and DPI settings.
const SIZES: [u32; 6] = [16, 24, 32, 48, 64, 256];

/// One icon image in .ico form: a 32-bit bitmap (rows bottom to top, BGRA)
/// followed by an all-zero AND mask, since the alpha channel does the work.
fn ico_image(size: u32) -> Vec<u8> {
    let rgba = icon_pixels::rgba(size);
    let mask_row = size.div_ceil(32) * 4;
    let pixel_bytes = size * size * 4;
    let mut out = Vec::new();
    // BITMAPINFOHEADER; the height counts the bitmap and the mask together.
    for value in [40, size, size * 2] {
        out.extend(value.to_le_bytes());
    }
    out.extend(1u16.to_le_bytes()); // planes
    out.extend(32u16.to_le_bytes()); // bits per pixel
    for value in [0, pixel_bytes + mask_row * size, 0, 0, 0, 0] {
        out.extend(u32::to_le_bytes(value));
    }
    for y in (0..size).rev() {
        for x in 0..size {
            let p = ((y * size + x) * 4) as usize;
            out.extend([rgba[p + 2], rgba[p + 1], rgba[p], rgba[p + 3]]);
        }
    }
    out.resize(out.len() + (mask_row * size) as usize, 0);
    out
}

fn ico() -> Vec<u8> {
    let images: Vec<Vec<u8>> = SIZES.iter().map(|&s| ico_image(s)).collect();
    let mut out = Vec::new();
    for value in [0u16, 1, SIZES.len() as u16] {
        out.extend(value.to_le_bytes()); // reserved, type (icon), count
    }
    let mut offset = 6 + 16 * SIZES.len() as u32;
    for (&size, image) in SIZES.iter().zip(&images) {
        let dim = if size >= 256 { 0 } else { size as u8 }; // 0 means 256
        out.extend([dim, dim, 0, 0]);
        out.extend(1u16.to_le_bytes()); // planes
        out.extend(32u16.to_le_bytes()); // bits per pixel
        out.extend((image.len() as u32).to_le_bytes());
        out.extend(offset.to_le_bytes());
        offset += image.len() as u32;
    }
    images.into_iter().for_each(|image| out.extend(image));
    out
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/icon_pixels.rs");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let path = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR is set")).join("icon.ico");
    fs::write(&path, ico()).expect("write icon.ico");

    let mut resource = winresource::WindowsResource::new();
    resource.set_icon(path.to_str().expect("UTF-8 path"));
    resource.set("FileDescription", "SciCalc 82");
    resource.set("ProductName", "SciCalc 82");
    // Without the Windows SDK the app still builds, just without an .exe icon.
    if let Err(e) = resource.compile() {
        println!("cargo:warning=could not embed the .exe icon: {e}");
    }
}
