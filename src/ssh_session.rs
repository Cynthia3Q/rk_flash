use ssh2::Session;
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};

pub fn connect_ssh(ip: String, port: u16, user: String, passwd: String) -> std::io::Result<()> {
    let tcp = TcpStream::connect(format!("{}:{}", ip, port)).unwrap();

    let mut sess = Session::new().unwrap();
    sess.set_tcp_stream(tcp);
    sess.handshake().unwrap();
    sess.userauth_password(&user, &passwd).unwrap();

    let mut channel = sess.channel_session().unwrap();
    channel.exec("ls").unwrap();

    let mut s = String::new();
    channel.read_to_string(&mut s).unwrap();
    println!("{}", s);

    channel.wait_close().unwrap();
    println!("{}", channel.exit_status().unwrap());
    println!("Connecting to {}:{} with password {}", ip, port, passwd);

    Ok(())
}

#[allow(dead_code)]
fn connect_via_telnet(
    address: String,
    port: String,
    command: String,
) -> impl std::future::Future<Output = String> {
    async move {
        let mut telnet =
            Telnet::connect((address.as_str(), port.parse::<u16>().unwrap()), 256).unwrap();
        telnet.write(command.as_bytes()).unwrap();
        let event = telnet.read().unwrap();
        match event {
            telnet::Event::Data(buffer) => String::from_utf8_lossy(&buffer).into_owned(),
            _ => "Failed to receive response".into(),
        }
    }
}
