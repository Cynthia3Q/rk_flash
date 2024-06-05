use std::fs;
use std::io::{self, BufRead};
use std::net::TcpStream;
use telnet::Telnet;

async fn send_commands_via_telnet(file_path: &str, address: &str) -> io::Result<()> {
    // 读取文件内容
    let file = fs::File::open(file_path)?;
    let commands = io::BufReader::new(file);

    // 创建一个 Telnet 对象
    let mut telnet = Telnet::connect(address, 23)?;

    // 遍历文件的每一行
    for cmd in commands.lines() {
        let cmd = cmd?;

        // 发送数据
        telnet.write(cmd.as_bytes())?;

        // 读取返回的数据
        let event = telnet.read()?;

        match event {
            telnet::Event::Data(data) => {
                // 判断返回的数据
                if data.is_empty() {
                    println!("No response for command: {}", line);
                } else {
                    println!(
                        "Response for command {}: {}",
                        line,
                        String::from_utf8_lossy(&data)
                    );
                }
            }
            _ => (),
        }
    }

    Ok(())
}
