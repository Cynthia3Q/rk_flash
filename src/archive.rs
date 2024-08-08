use zip::ZipArchive;

fn extract_zip(file_path: &str, destination: &str) -> io::Result<()> {
    // 打开 ZIP 文件
    let zip_file = File::open(file_path)?;
    let mut archive = ZipArchive::new(std::io::BufReader::new(zip_file))?;

    // 解压 ZIP 文件中的每个文件
    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let outpath = std::path::Path::new(destination).join(file.sanitized_name());

        if file.is_dir() {
            // 创建文件夹
            std::fs::create_dir_all(&outpath)?;
        } else {
            // 创建文件
            if let Some(p) = outpath.parent() {
                if !p.exists() {
                    std::fs::create_dir_all(p)?;
                }
            }
            let mut outfile = File::create(&outpath)?;
            io::copy(&mut file, &mut outfile)?;
        }

        // 设置解压文件的权限
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Some(mode) = file.unix_mode() {
                std::fs::set_permissions(&outpath, std::fs::Permissions::from_mode(mode))?;
            }
        }
    }
    Ok(())
}
