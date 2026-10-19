//! Embeds the app icon in the Windows executable, so Explorer and shortcuts
//! show it. The .ico is generated from the same code as the window icon.

use std::path::PathBuf;
use std::{env, fs};

#[path = "src/icon_pixels.rs"]
mod icon_pixels;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/icon_pixels.rs");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let path = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR is set")).join("icon.ico");
    fs::write(&path, icon_pixels::ico()).expect("write icon.ico");

    let mut resource = winresource::WindowsResource::new();
    resource.set_icon(path.to_str().expect("UTF-8 path"));
    resource.set("FileDescription", "SciCalc 82");
    resource.set("ProductName", "SciCalc 82");
    // Without the Windows SDK the app still builds, just without an .exe icon.
    if let Err(e) = resource.compile() {
        println!("cargo:warning=could not embed the .exe icon: {e}");
    }
}
