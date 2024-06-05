#![deny(unsafe_code)]

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

slint::include_modules!();

use std::rc::Rc;

use slint::{Model, StandardListViewItem, VecModel};

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(start))]
pub fn main() {
    // This provides better error messages in debug mode.
    // It's disabled in release mode so it doesn't bloat up the file size.
    #[cfg(all(debug_assertions, target_arch = "wasm32"))]
    console_error_panic_hook::set_once();

    // Disable gettext on macOS due to https://github.com/Koka/gettext-rs/issues/114
    #[cfg(not(target_os = "macos"))]
    //slint::init_translations!(concat!(env!("CARGO_MANIFEST_DIR"), "/lang/"));
    let app = App::new().unwrap();

    app.run().unwrap();
}
