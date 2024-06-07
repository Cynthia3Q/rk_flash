use crate::SshInfo;
use ssh2::Session;
use std::io::{Read, Result, Write};
use std::net::{TcpStream, ToSocketAddrs};

pub fn connect_ssh(ip: &String, port: u16, user: &String, passwd: &String) -> std::io::Result<()> {
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

    channel.close().unwrap();
    channel.wait_close().unwrap();
    println!("{}", channel.exit_status().unwrap());
    println!("Connecting to {}:{} with password {}", ip, port, passwd);

    Ok(())
}

//todo
pub fn cmd_via_ssh(ssh_auth: &SshInfo, cmd: &String) -> Result<String> {
    let tcp = TcpStream::connect(format!("{}:{}", ssh_auth.ip, 22)).unwrap();

    let mut sess = Session::new().unwrap();
    sess.set_tcp_stream(tcp);
    sess.handshake().unwrap();
    sess.userauth_password(&ssh_auth.username, &ssh_auth.password)
        .unwrap();

    let mut channel = sess.channel_session().unwrap();
    channel.exec(&cmd).unwrap();

    let mut s = String::new();
    channel.read_to_string(&mut s).unwrap();

    channel.close().unwrap();
    channel.wait_close().unwrap();
    println!("{}", channel.exit_status().unwrap());

    Ok(s)
}

#[allow(dead_code)]
fn connect_via_ssh(
    address: String,
    port: String,
    username: String,
    password: String,
    command: String,
) -> impl std::future::Future<Output = String> {
    async move {
        let tcp = TcpStream::connect(format!("{}:{}", address, port)).unwrap();
        let mut session = Session::new().unwrap();
        session.set_tcp_stream(tcp);
        session.handshake().unwrap();
        session.userauth_password(&username, &password).unwrap();

        if session.authenticated() {
            let mut channel = session.channel_session().unwrap();
            channel.exec(&command).unwrap();
            let mut s = String::new();
            channel.read_to_string(&mut s).unwrap();
            channel.wait_close().unwrap();
            s
        } else {
            "Authentication failed".into()
        }
    }
}
