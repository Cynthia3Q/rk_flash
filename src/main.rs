//#![deny(unsafe_code)]

use slint::ModelRc;
use tokio::sync::watch::error;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;
slint::include_modules!();

use slint::{Model, StandardListViewItem, VecModel};
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::{exit, Command};
use std::sync::Arc;
use std::sync::Mutex;
use tokio::time::Duration;
mod flash;
mod merge_filesystem;
use regex::Regex;

use flash::rk_flash_start;

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
    dev_no: String,
    mode: String,
    serial_no: String,
}

impl From<device_info> for DeviceInfo {
    fn from(device_info: device_info) -> Self {
        Self {
            checked: device_info.checked,
            dev_no: device_info.dev_no.to_string(),
            mode: device_info.mode.to_string(),
            serial_no: device_info.serial_no.to_string(),
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
        // println!("Command output:\n{}", output_str); // 打印命令输出
        let mut description: String;
        let devices: Vec<DeviceInfo> = output_str
            .lines()
            .filter(|line| line.starts_with("DevNo="))
            .enumerate()
            .filter_map(|(_, line)| parse_device_description(line))
            .collect();

        //打印解析后的设备列表
        for device in &devices {
            println!("Parsed device: {:?}", device);
        }

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
                dev_no: d.dev_no.clone().into(),
                mode: d.mode.clone().into(),
                serial_no: d.serial_no.clone().into(),
            })
            .collect();
        ModelRc::new(VecModel::from(device_infos))
    }
}

// 添加一个解析函数从字符串中提取字段
fn parse_device_description(description: &str) -> Option<DeviceInfo> {
    let re = Regex::new(r"DevNo=(\d+)\s+.*?Mode=(\w+)\s+.*?SerialNo=(\w+)").unwrap();
    re.captures(description).map(|caps| {
        DeviceInfo {
            checked: true, // 默认值，根据需要设置
            dev_no: caps
                .get(1)
                .map_or_else(|| "".to_string(), |m| m.as_str().to_string()),
            mode: caps
                .get(2)
                .map_or_else(|| "".to_string(), |m| m.as_str().to_string()),
            serial_no: caps
                .get(3)
                .map_or_else(|| "".to_string(), |m| m.as_str().to_string()),
        }
    })
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(start))]
#[tokio::main]
pub async fn main() -> Result<(), slint::PlatformError> {
    #[cfg(target_os = "linux")]
    check_root();
    // This provides better error messages in debug mode.
    // It's disabled in release mode so it doesn't bloat up the file size.
    #[cfg(all(debug_assertions, target_arch = "wasm32"))]
    console_error_panic_hook::set_once();

    // Disable gettext on macOS due to https://github.com/Koka/gettext-rs/issues/114
    #[cfg(not(target_os = "macos"))]
    //slint::init_translations!(concat!(env!("CARGO_MANIFEST_DIR"), "/lang/"));
    let app = App::new().unwrap();
    /*
    let (tx, rx) = std::sync::mpsc::channel();

    app.global::<ControlsPageAdapter>().on_flash_apply({
        move |flash| {
            println!("{} {}", flash.board_type, flash.version_selected);
            tx.send(flash.clone()).unwrap();
        }
    });
    */

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
    let flash_info_struct: FlashInfo = flash_info.into();

    // Set the version list model for the ComboBox in Slint
    //let model_rc = flash_info_struct.to_model_rc();
    //let devices_rc: ModelRc<device_info> = flash_info_struct.devices_to_model_rc();

    let flash_info_arc = Arc::new(Mutex::new(flash_info_struct));

    let flash_info_arc_clone = Arc::clone(&flash_info_arc);

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

    app.global::<ControlsPageAdapter>().on_flash_apply({
        let app_weak = app.as_weak();
        move |a| {
            let app = app_weak.upgrade().unwrap();
            let mut flash_info_lock = flash_info_arc_clone.lock().unwrap();
            flash_info_lock.update_device_list();
            print_flash_info(&flash_info_lock);

            app.global::<ControlsPageAdapter>().set_flash(flash_info {
                board_type: flash_info_lock.board_type.clone().into(),
                version_list: flash_info_lock.to_model_rc(),
                version_selected: flash_info_lock.version_selected.clone().into(),
                devices: flash_info_lock.devices_to_model_rc(),
            });
        }
    });

    let app_weak = app.as_weak();
    // Periodically update the device list
    std::thread::spawn(move || loop {
        {
            //let mut flash_info_lock = flash_info_arc_clone.lock().unwrap();
            //flash_info_lock.update_device_list();
            //print_flash_info(&flash_info_lock);
        }

        std::thread::sleep(Duration::from_millis(500));
    });
    // 启动事件循环
    app.run()
}

fn print_flash_info(flash_info: &FlashInfo) {
    dbg!("\n");
    println!("Board Type: {:?}", flash_info.board_type);
    println!("Version List: {:?}", flash_info.version_list);
    println!("Version Selected: {:?}", flash_info.version_selected);
    println!("Devices: {:?}", flash_info.devices);
}

#[cfg(target_os = "linux")]
fn check_root() {
    if unsafe { libc::getuid() } != 0 {
        eprintln!("please run this script with root.");
        eprintln!("etc: sudo ./rk_flash");
        exit(1);
    }
}
