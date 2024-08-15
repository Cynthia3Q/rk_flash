// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT
use std::env;
use std::fs;
use std::path::Path;

fn main() {
    slint_build::compile("ui/test.slint").unwrap();
    log_compile_info();

    println!("cargo:rerun-if-changed=build.rs");
}

fn log_compile_info() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let build_date = format!("{}", chrono::Local::now().format("%Y-%m-%d %H:%M"));
    let dest_path = Path::new(&out_dir).join("build_info.rs");
    fs::write(
        &dest_path,
        format!("pub const BUILD_DATE: &str = \"{}\";", build_date),
    )
    .unwrap();
}
