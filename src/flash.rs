use slint::{Model, StandardListViewItem, VecModel};
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::{exit, Command};
use std::sync::Arc;
use std::sync::Mutex;
use tokio::time::Duration;

use crate::merge_filesystem::prepare_filesystem;
use crate::FlashInfo;

pub async fn rk_flash_start(flash: FlashInfo) -> tokio::io::Result<()> {
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
    let rootfs = prepare_filesystem(&flash.version_selected, &flash.board_type).unwrap();

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
