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
    devices: Vec<DeviceInfo>,
}

#[derive(Default, Debug, Clone)]
struct DeviceInfo {
    checked: bool,
    description: String,
}

impl From<device_info> for DeviceInfo {
    fn from(device_info: device_info) -> Self {
        Self {
            checked: device_info.checked,
            description: device_info.description.to_string(),
        }
    }
}

impl From<flash_info> for FlashInfo {
    fn from(flash_info: flash_info) -> Self {
        let mut flash_info_struct = Self {
            board_type: flash_info.board_type.to_string(),
            version_list: vec![],
            version_selected: flash_info.version_selected.to_string(),
            devices: vec![],
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

    fn update_device_list(&mut self) {
        let output = Command::new("tools/rk_flash_tools/upgrade_tool")
            .arg("LD")
            .output()
            .expect("Failed to execute upgrade_tool");

        let output_str = String::from_utf8_lossy(&output.stdout);

        let devices: Vec<DeviceInfo> = output_str
            .lines()
            .filter(|line| line.starts_with("DevNo="))
            .enumerate()
            .map(|(i, line)| DeviceInfo {
                checked: i == 0,
                description: line.to_string(),
            })
            .collect();

        self.devices = devices;
    }
    // Convert Vec<String> to ModelRc<SharedString>
    fn to_model_rc(&self) -> ModelRc<slint::SharedString> {
        let shared_strings: Vec<slint::SharedString> = self
            .version_list
            .iter()
            .map(|s| slint::SharedString::from(s.as_str()))
            .collect();
        ModelRc::new(VecModel::from(shared_strings))
    }

    fn devices_to_model_rc(&self) -> ModelRc<device_info> {
        let device_infos: Vec<device_info> = self
            .devices
            .iter()
            .map(|d| device_info {
                checked: d.checked,
                description: d.description.clone().into(),
            })
            .collect();
        ModelRc::new(VecModel::from(device_infos))
    }
}

use slint::{Model, StandardListViewItem, VecModel};
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::{exit, Command};
use std::sync::Arc;
use std::sync::Mutex;
use std::{ops::ControlFlow, rc::Rc};

use tokio::time::Duration;

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

    // Initialize FlashInfo
    let flash_info = flash_info::default();
    let mut flash_info_struct: FlashInfo = flash_info.into();

    // Set the version list model for the ComboBox in Slint
    //let model_rc = flash_info_struct.to_model_rc();
    //let devices_rc: ModelRc<device_info> = flash_info_struct.devices_to_model_rc();

    let flash_info_arc = Arc::new(Mutex::new(flash_info_struct));

    let flash_info_arc_clone = Arc::clone(&flash_info_arc);

    let app_weak = app.as_weak();

    {
        let flash_info_lock = flash_info_arc.lock().unwrap();
        let model_rc = flash_info_lock.to_model_rc();
        let devices_rc = flash_info_lock.devices_to_model_rc();

        app.global::<ControlsPageAdapter>().set_flash(flash_info {
            board_type: flash_info_lock.board_type.clone().into(),
            version_list: model_rc,
            version_selected: flash_info_lock.version_selected.clone().into(),
            devices: devices_rc,
        });
    }
    // Periodically update the device list
    std::thread::spawn(move || loop {
        {
            let mut flash_info_lock = flash_info_arc_clone.lock().unwrap();
            flash_info_lock.update_device_list();
        }

        if let Some(app) = app_weak.upgrade() {
            let flash_info_lock = flash_info_arc_clone.lock().unwrap();
            let devices_rc = flash_info_lock.devices_to_model_rc();

            app.global::<ControlsPageAdapter>().set_flash(flash_info {
                board_type: flash_info_lock.board_type.clone().into(),
                version_list: flash_info_lock.to_model_rc(),
                version_selected: flash_info_lock.version_selected.clone().into(),
                devices: devices_rc,
            });
        }

        std::thread::sleep(Duration::from_millis(500));
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

/*
fn check_root() {
    if unsafe { libc::getuid() } != 0 {
        eprintln!("please run this script with root.");
        eprintln!("etc: sudo ./your_program");
        exit(1);
    }
}
*/

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
