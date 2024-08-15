//#![deny(unsafe_code)]

use log::debug;

use slint::*;
use tokio::sync::watch::error;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

use slint::{Model, StandardListViewItem, VecModel};
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::{exit, Command};
use std::thread;
use tokio::time::Duration;
mod flash;
mod merge_filesystem;

use flash::flash_setup;
use flash::rk_flash_start;
use regex::Regex;

pub mod ui {
    slint::include_modules!();
}

use ui::*;
//supported board ,add new board here
const SUPPORTED_BOARDS: &[&str] = &["dc11p626", "dc21scu"];

#[derive(Default, Debug, Clone)]
struct FlashInfo {
    supported_board: Vec<String>,
    board_type: String,
    version_list: Vec<String>,
    version_selected: String,
    devices: Vec<DeviceInfo>,
}

#[derive(Default, Debug, Clone)]
struct DeviceInfo {
    checked: bool,
    dev_no: String,
    loc_id: String,
    mode: String,
    serial_no: String,
    progress: String,
}

impl From<device_info> for DeviceInfo {
    fn from(device_info: device_info) -> Self {
        Self {
            checked: device_info.checked,
            dev_no: device_info.dev_no.to_string(),
            loc_id: device_info.loc_id.to_string(),
            mode: device_info.mode.to_string(),
            serial_no: device_info.serial_no.to_string(),
            progress: device_info.progress.to_string(),
        }
    }
}

impl From<flash_info> for FlashInfo {
    fn from(flash_info: flash_info) -> Self {
        Self {
            supported_board: flash_info
                .supported_board
                .iter()
                .map(|board| board.to_string()) // 根据实际类型进行转换
                .collect(),
            board_type: flash_info.board_type.to_string(),
            version_list: FlashInfo::load_versions(),
            version_selected: flash_info.version_selected.to_string(),
            devices: flash_info
                .devices
                .iter()
                .map(|device_model| DeviceInfo::from(device_model.clone()))
                .collect(),
        }
    }
}

impl FlashInfo {
    // Function to load versions from the directory
    fn load_versions() -> Vec<String> {
        let mut versions = vec![];

        // Get the current directory and append "upgrade"
        let upgrade_dir = std::env::current_dir().unwrap().join("upgrade");

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
        let devices: Vec<DeviceInfo> = output_str
            .lines()
            .filter(|line| line.starts_with("DevNo="))
            .enumerate()
            .filter_map(|(_, line)| parse_device_description(line))
            .collect();

        //打印解析后的设备列表
        //for device in &devices {
        //    debug!("Parsed device: {:?}", device);
        //}

        self.devices = devices;
    }

    fn supported_bd_to_model_rc(&self) -> ModelRc<slint::SharedString> {
        let supported_boards: Vec<slint::SharedString> =
            SUPPORTED_BOARDS.iter().map(|&board| board.into()).collect();
        ModelRc::new(VecModel::from(supported_boards))
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
                loc_id: d.loc_id.clone().into(),
                mode: d.mode.clone().into(),
                serial_no: d.serial_no.clone().into(),
                progress: d.progress.clone().into(),
            })
            .collect();
        ModelRc::new(VecModel::from(device_infos))
    }
}

// 添加一个解析函数从字符串中提取字段
fn parse_device_description(description: &str) -> Option<DeviceInfo> {
    let re = Regex::new(r"DevNo=(\d+)\s+.*?LocationID=(\d+)\s+.*?Mode=(\w+)\s+.*?SerialNo=(\w+)")
        .unwrap();
    re.captures(description).map(|caps| {
        DeviceInfo {
            checked: true, // 默认值，根据需要设置
            dev_no: caps
                .get(1)
                .map_or_else(|| "".to_string(), |m| m.as_str().to_string()),
            loc_id: caps
                .get(2)
                .map_or_else(|| "".to_string(), |m| m.as_str().to_string()),
            mode: caps
                .get(3)
                .map_or_else(|| "".to_string(), |m| m.as_str().to_string()),
            serial_no: caps
                .get(4)
                .map_or_else(|| "".to_string(), |m| m.as_str().to_string()),
            progress: "ready".to_string(),
        }
    })
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(start))]
#[tokio::main]
pub async fn main() -> Result<(), slint::PlatformError> {
    env_logger::init();

    cmdline_handle();

    #[cfg(target_os = "linux")]
    check_root();
    // This provides better error messages in debug mode.
    // It's disabled in release mode so it doesn't bloat up the file size.
    #[cfg(all(debug_assertions, target_arch = "wasm32"))]
    console_error_panic_hook::set_once();

    // Disable gettext on macOS due to https://github.com/Koka/gettext-rs/issues/114
    //#[cfg(not(target_os = "macos"))]
    //slint::init_translations!(concat!(env!("CARGO_MANIFEST_DIR"), "/lang/"));
    let window = MainWindow::new().unwrap();

    /*
    window.global::<ControlsPageAdapter>().on_flash_start({
        let app_weak = window.as_weak();

        let window = app_weak.upgrade().unwrap();
        move || {
            let flash_info: FlashInfo = window.global::<ControlsPageAdapter>().get_flash().into();
            tokio::spawn(rk_flash_start(flash_info));
        }
    });*/
    let _devices_timer = devices_scanf_timer(&window);

    ControlsPageAdapter::get(&window).on_flash_start({
        let window = window.as_weak().upgrade().unwrap();
        move || {
            _devices_timer.stop();
            let flash_info: FlashInfo = window.global::<ControlsPageAdapter>().get_flash().into();
            let setup_join = flash_setup(&window, flash_info);
        }
    });

    let flash_info_rust: FlashInfo = Default::default();

    window
        .global::<ControlsPageAdapter>()
        .set_flash(flash_info {
            supported_board: flash_info_rust.supported_bd_to_model_rc(),
            board_type: flash_info_rust.board_type.clone().into(),
            version_list: flash_info_rust.to_model_rc(),
            version_selected: flash_info_rust.version_selected.clone().into(),
            devices: flash_info_rust.devices_to_model_rc(),
        });

    window.global::<ControlsPageAdapter>().on_flash_apply({
        let app_weak = window.as_weak();
        let mut flash_info_rust: FlashInfo = Default::default();
        move |mut flash| {
            flash_info_rust.update_device_list();
            flash_info_rust.version_list = FlashInfo::load_versions();
            flash.devices = flash_info_rust.devices_to_model_rc();
            flash.version_list = flash_info_rust.to_model_rc();
            print_flash_info(&flash);
            ControlsPageAdapter::get(&app_weak.unwrap()).set_flash(flash);
        }
    });

    //_devices_timer.stop();
    // 启动事件循环
    window.run()
}

pub fn devices_scanf_timer(window: &MainWindow) -> Timer {
    let devices_timer = Timer::default();
    devices_timer.start(
        TimerMode::Repeated,
        core::time::Duration::from_millis(500),
        {
            let window_weak = window.as_weak();
            move || {
                let mut flash = ControlsPageAdapter::get(&window_weak.unwrap()).get_flash();
                let mut flash_info: FlashInfo = Default::default();
                flash_info.update_device_list();
                flash_info.version_list = FlashInfo::load_versions();
                flash.devices = flash_info.devices_to_model_rc();
                flash.version_list = flash_info.to_model_rc();

                ControlsPageAdapter::get(&window_weak.unwrap()).set_flash(flash);
            }
        },
    );
    devices_timer
}

fn print_flash_info(flash_info: &flash_info) {
    debug!("Board Type: {:?}", flash_info.board_type);
    debug!("Version List: {:?}", flash_info.version_list);
    debug!("Version Selected: {:?}", flash_info.version_selected);
    debug!("Devices: {:?}", flash_info.devices);
}

#[cfg(target_os = "linux")]
fn check_root() {
    use log::warn;

    if unsafe { libc::getuid() } != 0 {
        warn!("please run this program with root.");
        warn!("etc: sudo ./rk_flash");
        exit(1);
    }
}

fn print_version() {
    mod build_info {
        include!(concat!(env!("OUT_DIR"), "/build_info.rs"));
    }

    const VERSION: &str = env!("CARGO_PKG_VERSION");
    log::info!("Software Version: {}", VERSION);
    log::info!("Build Date: {}", build_info::BUILD_DATE);
}

fn cmdline_handle() {
    let args: Vec<String> = std::env::args().collect();

    if args.contains(&"-v".to_string()) || args.contains(&"--version".to_string()) {
        print_version();
        exit(0);
    }
}
