use flate2::read::GzDecoder;
use slint::{Model, StandardListViewItem, VecModel};
use std::env;
use std::fs;
use std::fs::File;
use std::fs::Permissions;
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::{exit, Command};
use std::{ops::ControlFlow, rc::Rc};
use tar::Archive;
use walkdir::WalkDir;

pub fn prepare_filesystem(
    version: &str,
    board_type: &str,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    // 创建临时文件夹

    let update_rootfs_img = PathBuf::from(format!("tmp/rootfs-{}.img", version));

    // 如果目标文件已经存在，直接返回
    if update_rootfs_img.exists() {
        println!(
            "File {} already exists, skipping creation.",
            update_rootfs_img.display()
        );
        return Ok(update_rootfs_img);
    }

    let tmp_dir = PathBuf::from("tmp");
    if !tmp_dir.exists() {
        fs::create_dir(&tmp_dir)?;
    }

    let temp_mount_dir = tmp_dir.join(format!("rootfs-{}", version));
    // Create temporary mount directory
    if !temp_mount_dir.exists() {
        fs::create_dir(&temp_mount_dir)?;
    }

    // 拷贝 rootfs.img 到临时文件夹
    let rootfs_img = env::current_dir()?.join("rockdev/rootfs.img");
    let temp_rootfs_img = tmp_dir.join("rootfs.img");

    // Copy rootfs.img to temporary location
    fs::copy(&rootfs_img, &temp_rootfs_img)?;

    // 挂载 rootfs.img 到临时目录
    let mount_output = Command::new("mount")
        .args(&[
            "-o",
            "loop",
            temp_rootfs_img.to_str().unwrap(),
            temp_mount_dir.to_str().unwrap(),
        ])
        .output()?;

    if !mount_output.status.success() {
        return Err(Box::new(io::Error::new(
            io::ErrorKind::Other,
            "Failed to mount image",
        )));
    }

    let update_rootfs_path = env::current_dir()?.join("rockdev/update-rootfs.tar.gz");
    let update_rootfs_file = File::open(&update_rootfs_path)?;
    let mut update_rootfs_archive = Archive::new(GzDecoder::new(update_rootfs_file));
    update_rootfs_archive.unpack(&env::current_dir()?.join("rockdev"))?;

    let update_rootfs_dir = env::current_dir()?.join("rockdev/update-rootfs");
    for dir in &["etc", "root"] {
        let src_dir = update_rootfs_dir.join(dir);
        let dst_dir = temp_mount_dir.join(dir);
        if src_dir.exists() {
            copy_dir_all(&src_dir, &dst_dir)?;
        }
    }

    let version_zip = env::current_dir()?.join(format!("upgrade/{}.zip", version));
    Command::new("unzip")
        .arg(&version_zip)
        .arg("-d")
        .arg(&tmp_dir)
        .status()?;

    // Extract filesystem.tar.gz from the unzipped version directory
    let version_dir = tmp_dir.join(version);
    let tar_gz_path = version_dir.join("board/filesystem.tar.gz");

    let file = File::open(&tar_gz_path)?;
    let mut archive = Archive::new(GzDecoder::new(file));

    // Extract the tar.gz archive into the mounted root file system directory
    archive.unpack(&version_dir.join("board"))?;

    // 拷贝文件到 rootfs.img
    let version_dir = tmp_dir.join(format!("{}", version));
    let filesystem_dir = if board_type == "dc11scu" {
        version_dir.join("scu/filesystem")
    } else {
        version_dir.join("board/filesystem")
    };

    for dir in &["etc", "mnt", "root", "usr"] {
        let src_dir = filesystem_dir.join(dir);
        let dst_dir = temp_mount_dir.join(dir);
        if src_dir.exists() {
            copy_dir_all(&src_dir, &dst_dir)?;
        }
    }

    fs::copy(
        version_dir.join(format!("board/{}.bin", board_type)),
        temp_mount_dir.join(format!("mnt/build/{}.bin", board_type)),
    )?;

    // Write board_type to hostname and mnt/board_type

    fs::write(&temp_mount_dir.join("etc/hostname"), board_type)?;
    fs::write(&temp_mount_dir.join("mnt/config/boardtype"), board_type)?;

    let systemd_rc = temp_mount_dir.join("etc/rc.local");
    fs::set_permissions(&systemd_rc, Permissions::from_mode(0o755))?;

    let ssh_key_path = temp_mount_dir.join("root/.ssh");
    fs::set_permissions(&ssh_key_path, Permissions::from_mode(0o600))?;

    for entry in WalkDir::new(ssh_key_path) {
        let entry = entry?;
        let path = entry.path();

        let metadata = fs::metadata(&path)?;
        let permissions = metadata.permissions();
        let mode = permissions.mode();

        if mode & 0o777 != 0o600 {
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
        }
    }

    // 取消挂载
    Command::new("umount").arg(&temp_mount_dir).status()?;

    // 重命名 rootfs.img
    fs::rename(&temp_rootfs_img, &update_rootfs_img)?;

    // 清理临时文件夹
    fs::remove_dir_all(&temp_mount_dir)?;
    fs::remove_dir_all(&version_dir)?;

    Ok(update_rootfs_img)
}

fn copy_dir_all(src: &PathBuf, dst: &PathBuf) -> io::Result<()> {
    if !dst.exists() {
        fs::create_dir_all(dst)?;
    }

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if src_path.is_dir() {
            copy_dir_all(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}
