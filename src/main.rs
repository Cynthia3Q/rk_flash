//#![deny(unsafe_code)]

use slint::ModelRc;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;
slint::include_modules!();

#[derive(Default, Debug, Clone)]
struct FlashInfo {
    board_type: String,
    version_list: Vec<String>,
    version_selected: String,
}

impl From<flash_info> for FlashInfo {
    fn from(flash_info: flash_info) -> Self {
        let mut flash_info_struct = Self {
            board_type: flash_info.board_type.to_string(),
            version_list: vec![],
            version_selected: flash_info.version_selected.to_string(),
        };

        let versions = FlashInfo::load_versions();
        flash_info_struct.version_list = versions;

        flash_info_struct
    }
}

impl FlashInfo {
    // Function to load versions from the directory
    fn load_versions() -> Vec<String> {
        let mut versions = vec!["Select release version".to_string()];

        // Get the current directory and append "upgrade"
        let mut upgrade_dir = std::env::current_dir().unwrap();
        upgrade_dir.push("upgrade");

        if let Ok(entries) = fs::read_dir(upgrade_dir) {
            for entry in entries {
                if let Ok(entry) = entry {
                    let path = entry.path();
                    if path.is_file() && path.extension().map_or(false, |ext| ext == "zip") {
                        if let Some(file_stem) = path.file_stem().and_then(|stem| stem.to_str()) {
                            versions.push(file_stem.to_string());
                        }
                    }
                }
            }
        }
        versions
    }

    // Convert Vec<String> to ModelRc<SharedString>
    // Convert Vec<String> to ModelRc<SharedString>
    fn to_model_rc(&self) -> ModelRc<slint::SharedString> {
        let shared_strings: Vec<slint::SharedString> = self
            .version_list
            .iter()
            .map(|s| slint::SharedString::from(s.as_str()))
            .collect();
        ModelRc::new(VecModel::from(shared_strings))
    }
}

use slint::{Model, StandardListViewItem, VecModel};
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::{exit, Command};
use std::{ops::ControlFlow, rc::Rc};

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(start))]
#[tokio::main]
pub async fn main() -> Result<(), slint::PlatformError> {
    //check_root();
    // This provides better error messages in debug mode.
    // It's disabled in release mode so it doesn't bloat up the file size.
    #[cfg(all(debug_assertions, target_arch = "wasm32"))]
    console_error_panic_hook::set_once();

    // Disable gettext on macOS due to https://github.com/Koka/gettext-rs/issues/114
    #[cfg(not(target_os = "macos"))]
    //slint::init_translations!(concat!(env!("CARGO_MANIFEST_DIR"), "/lang/"));
    let app = App::new().unwrap();

    let common_dir = fs::canonicalize(format!("{}", env::current_dir().unwrap().display()))
        .expect("Failed to get common directory");

    let upgrade_dir = common_dir.join("upgrade");

    let (tx, rx) = std::sync::mpsc::channel();
    app.global::<ControlsPageAdapter>().on_flash_apply({
        move |flash| {
            println!("{} {}", flash.board_type, flash.version_selected);
            tx.send(flash.clone()).unwrap();
        }
    });

    app.global::<ControlsPageAdapter>().on_flash_start({
        let app_weak = app.as_weak();
        move || {
            let app = app_weak.upgrade().unwrap();
            let flash: FlashInfo = app.global::<ControlsPageAdapter>().get_flash().into();
            tokio::spawn(rk_flash_start(flash));
        }
    });

    let flash_info = flash_info::default();
    let flash_info_struct: FlashInfo = flash_info.into();
    let model_rc = flash_info_struct.to_model_rc();
    app.global::<ControlsPageAdapter>().set_flash(flash_info {
        board_type: flash_info_struct.board_type.into(),
        version_list: model_rc,
        version_selected: flash_info_struct.version_selected.into(),
    });

    //let flash: FlashInfo = app.global::<ControlsPageAdapter>().get_flash().into();
    //let flash_info = rx.try_recv();
    //session_initialize(flash_info);
    app.run()
}

async fn rk_flash_start(flash: FlashInfo) -> tokio::io::Result<()> {
    // Define the paths
    let common_dir = fs::canonicalize(format!("{}", env::current_dir().unwrap().display()))
        .expect("Failed to get common directory");

    //let sdk_dir = fs::canonicalize(common_dir.join("..")).expect("Failed to get SDK directory");
    let upgrade_tool = common_dir.join("tools/rk_flash_tools/upgrade_tool");
    let rockdev_dir = common_dir.join("rockdev");

    let loader = rockdev_dir.join("loader.bin");
    let parameter = rockdev_dir.join("parameter.txt");
    let uboot = rockdev_dir.join("uboot.img");
    let boot = rockdev_dir.join("boot.img");
    let rootfs = rockdev_dir.join("rootfs.img");

    // Check the flash type argument
    let flash_type = env::args().nth(1).unwrap_or_else(|| "all".to_string());

    if flash_type == "all" {
        // Ensure upgrade_tool exists and is executable
        if !upgrade_tool.exists() {
            eprintln!("{} not found.", upgrade_tool.display());
            exit(1);
        }

        // Run the upgrade_tool commands
        run_command(&upgrade_tool, &["ul", loader.to_str().unwrap(), "-noreset"]);
        run_command(&upgrade_tool, &["di", "-p", parameter.to_str().unwrap()]);
        run_command(&upgrade_tool, &["di", "-uboot", uboot.to_str().unwrap()]);
        run_command(&upgrade_tool, &["di", "-b", boot.to_str().unwrap()]);
        run_command(&upgrade_tool, &["di", "-rootfs", rootfs.to_str().unwrap()]);
        run_command(&upgrade_tool, &["rd"]);
    }

    Ok(())
}

fn check_root() {
    if unsafe { libc::getuid() } != 0 {
        eprintln!("please run this script with root.");
        eprintln!("etc: sudo ./your_program");
        exit(1);
    }
}

// Function to run a command and handle errors
fn run_command(command: &PathBuf, args: &[&str]) {
    let status = Command::new(command)
        .args(args)
        .status()
        .expect("Failed to execute command");

    if !status.success() {
        eprintln!("Command {:?} failed with status: {:?}", args, status);
        exit(1);
    }
}
