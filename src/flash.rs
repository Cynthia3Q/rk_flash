use crate::merge_filesystem::prepare_filesystem;

use crate::DeviceInfo;
use crate::FlashInfo;
use log::{debug, error, info, warn};
use slint::Model;
use slint::ModelRc;
use slint::SharedString;
use slint::Weak;
//use slint::*;
use slint::ComponentHandle;
use slint::Window;
use std::env;
use std::error::Error;
use std::fs;
use std::path::PathBuf;
use std::process::{exit, Stdio};
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;
use tokio::io::AsyncReadExt;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::time::Duration;
//use ui::*;
use crate::ui::*;

pub fn flash_setup(window: &MainWindow, flash: FlashInfo) -> std::thread::JoinHandle<()> {
    let window_weak = window.as_weak();
    thread::spawn(move || {
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(rk_flash_start(window_weak, flash))
            .unwrap()
    })
}
fn update_flash_progress(window_weak: Weak<MainWindow>, progress: &str) {
    let progress = progress.to_string();
    window_weak.upgrade_in_event_loop(move |window| {
        let mut flash = window.global::<ControlsPageAdapter>().get_flash();
        let mut flash_info: FlashInfo = flash.clone().into();
        let devices_info = &mut flash_info.devices;

        let index = 0;
        if let Some(device) = devices_info.get_mut(index) {
            device.progress = progress;
        }
        flash.devices = flash_info.devices_to_model_rc();

        window.global::<ControlsPageAdapter>().set_flash(flash);
    });
}

//RUST_LOG=debug
pub async fn rk_flash_start(
    window_weak: Weak<MainWindow>,
    flash: FlashInfo,
) -> tokio::io::Result<()> {
    // Define the paths
    let common_dir = fs::canonicalize(env::current_dir().unwrap().to_str().unwrap())
        .expect("Failed to get common directory");

    //let sdk_dir = fs::canonicalize(common_dir.join("..")).expect("Failed to get SDK directory");
    let upgrade_tool = common_dir.join("tools/rk_flash_tools/upgrade_tool");
    let rockdev_dir = common_dir.join("rockdev");

    let loader = rockdev_dir.join("loader.bin");
    let parameter = rockdev_dir.join("parameter.txt");
    let uboot = rockdev_dir.join("uboot.img");
    let boot = rockdev_dir.join("boot.img");

    let rootfs = prepare_filesystem(&flash.version_selected, &flash.board_type).unwrap();
    // Check the flash type argument
    let flash_type = env::args().nth(1).unwrap_or_else(|| "all".to_string());

    // Ensure upgrade_tool exists and is executable
    if !upgrade_tool.exists() {
        eprintln!("{} not found.", upgrade_tool.display());
        exit(1);
    }

    // Filter out devices with checked == true
    let selected_devices: Vec<&DeviceInfo> = flash
        .devices
        .iter()
        .filter(|device| device.checked)
        .collect();
    debug!("Selected devices for flashing: {:?}", selected_devices);

    for d in selected_devices {
        if flash_type == "all" {
            println!("Flashing d with LocationID: {}", d.loc_id);
            // Run the upgrade_tool commands

            update_flash_progress(window_weak.clone(), "upgrade loader");
            run_command_with_progress(
                &upgrade_tool,
                &["-s", &d.loc_id, "ul", loader.to_str().unwrap(), "-noreset"],
            );

            update_flash_progress(window_weak.clone(), "writing parameter");
            run_command(
                &upgrade_tool,
                &["-s", &d.loc_id, "di", "-p", parameter.to_str().unwrap()],
            );

            update_flash_progress(window_weak.clone(), "Writing uboot");
            run_command(
                &upgrade_tool,
                &["-s", &d.loc_id, "di", "-uboot", uboot.to_str().unwrap()],
            );

            update_flash_progress(window_weak.clone(), "Writing boot");
            run_command(
                &upgrade_tool,
                &["-s", &d.loc_id, "di", "-b", boot.to_str().unwrap()],
            );
            update_flash_progress(window_weak.clone(), "Writing rootfs");
            run_command(
                &upgrade_tool,
                &["-s", &d.loc_id, "di", "-rootfs", rootfs.to_str().unwrap()],
            );

            update_flash_progress(window_weak.clone(), "Reset Device");
            run_command(&upgrade_tool, &["-s", &d.loc_id, "rd"]);
            update_flash_progress(window_weak.clone(), "SUCCESS");
        }
    }

    Ok(())
}

// Function to run a command and handle errors
fn run_command(command: &PathBuf, args: &[&str]) {
    let status = std::process::Command::new(command)
        .args(args)
        .status()
        .expect("Failed to execute command");

    if !status.success() {
        error!("Command {:?} failed with status: {:?}", args, status);
        exit(1);
    }
}

fn swicth_to_maskrom(flash: FlashInfo) {
    let common_dir = fs::canonicalize(env::current_dir().unwrap().to_str().unwrap())
        .expect("Failed to get common directory");

    //let sdk_dir = fs::canonicalize(common_dir.join("..")).expect("Failed to get SDK directory");
    let upgrade_tool = common_dir.join("tools/rk_flash_tools/upgrade_tool");

    let selected_devices: Vec<&DeviceInfo> = flash
        .devices
        .iter()
        .filter(|device| device.checked)
        .collect();
    debug!("Selected devices for flashing: {:?}", selected_devices);
    for d in selected_devices {
        run_command(&upgrade_tool, &["-s", &d.loc_id, "rd", "3"]);
    }
}

async fn update_progress(
    progress_sender: &Arc<slint::ModelRc<slint::SharedString>>,
    progress: &str,
) -> Result<(), Box<dyn Error>> {
    // Clone the current SharedString content and append the new progress line
    let mut new_progress = progress_sender
        .row_data(0)
        .as_ref()
        .map(|s| s.to_string())
        .unwrap_or_else(|| String::from(""));

    new_progress.push_str(progress);
    new_progress.push('\n');

    // Update the SharedString in the model
    progress_sender.set_row_data(0, slint::SharedString::from(new_progress));
    Ok(())
}

async fn run_command_with_progress(
    command: &PathBuf,
    args: &[&str],
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let mut child = tokio::process::Command::new(command)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let stdout = child.stdout.take().ok_or("Failed to capture stdout")?;
    let stderr = child.stderr.take().ok_or("Failed to capture stderr")?;

    let mut stdout_reader = BufReader::new(stdout).lines();
    let mut stderr_reader = BufReader::new(stderr).lines();

    // 使用 tokio::spawn 让标准输出和错误输出并行处理
    let stdout_handle = tokio::spawn(async move {
        while let Some(line) = stdout_reader.next_line().await? {
            log::info!("STDOUT: {}", line);
        }
        Ok::<_, Box<dyn Error + Send + Sync>>(())
    });

    let stderr_handle = tokio::spawn(async move {
        while let Some(line) = stderr_reader.next_line().await? {
            log::error!("STDERR: {}", line);
        }
        Ok::<_, Box<dyn Error + Send + Sync>>(())
    });

    // 等待命令完成
    let status = child.wait().await?;
    stdout_handle.await??;
    stderr_handle.await??;

    if !status.success() {
        return Err("Command failed".into());
    }

    Ok(())
}
async fn __update_flash_progress(progress: String) -> Result<(), Box<dyn Error>> {
    Ok(())
}

async fn flash_worker_loop(window_weak: Weak<MainWindow>) {
    let common_dir = fs::canonicalize(env::current_dir().unwrap().to_str().unwrap()).unwrap();
    let upgrade_tool = common_dir.join("tools/rk_flash_tools/upgrade_tool");
    let rockdev_dir = common_dir.join("rockdev");
    let loader = rockdev_dir.join("loader.bin");
    let parameter = rockdev_dir.join("parameter.txt");
    let uboot = rockdev_dir.join("uboot.img");
    let boot = rockdev_dir.join("boot.img");
    let rootfs = rockdev_dir.join("rootfs.img");

    update_flash_progress(window_weak.clone(), "upgrade loader...");
    run_command_with_progress(&upgrade_tool, &["ul", loader.to_str().unwrap(), "-noreset"]).await;

    update_flash_progress(window_weak.clone(), "writing parameter");
    run_command_with_progress(&upgrade_tool, &["di", "-p", parameter.to_str().unwrap()]).await;

    update_flash_progress(window_weak.clone(), "Writing uboot image...");
    run_command_with_progress(&upgrade_tool, &["di", "-uboot", uboot.to_str().unwrap()]).await;

    update_flash_progress(window_weak.clone(), "Writing boot image...");
    run_command_with_progress(&upgrade_tool, &["di", "-b", boot.to_str().unwrap()]).await;

    update_flash_progress(window_weak.clone(), "Writing rootfs image...");
    run_command_with_progress(&upgrade_tool, &["di", "-rootfs", rootfs.to_str().unwrap()]).await;

    update_flash_progress(window_weak.clone(), "Reset Device...");
    run_command_with_progress(&upgrade_tool, &["rd"]).await;

    update_flash_progress(window_weak, "Success.");
}

fn callback_func_register() -> Result<(), ()> {
    Ok(())
}
