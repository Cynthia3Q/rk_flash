use chrono::Local;
use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::time::Duration;
use telnet::{Event, Telnet};
use tokio::task;

pub async fn connect_telnet(
    ip: String,
    port: u16,
    user: String,
    passwd: String,
) -> tokio::io::Result<Telnet> {
    let date = Local::now();
    let filename = format!("log_{}.txt", date.format("%Y_%m_%d"));
    let mut log_file = OpenOptions::new()
        .write(true)
        .append(true)
        .create(true)
        .open(&filename)
        .unwrap();
    //let (tx, mut rx) = mpsc::channel(100);
    let handle = tokio::spawn(async move {
        let mut telnet_stream =
            Telnet::connect((ip, port), 1024).expect("Couldn't connect to the server...");

        let event = telnet_stream.read_timeout(Duration::from_secs(1)).unwrap();
        match event {
            Event::Data(buffer) => {
                println!("{}", String::from_utf8_lossy(&buffer));
            }
            Event::NoData => {
                println!("No data");
            }
            Event::TimedOut => {}
            Event::Error(err) => {
                println!("Error: {:?}", err);
            }
            _ => {}
        }

        telnet_stream.write(user.as_bytes()).unwrap();
        telnet_stream.write(b"\n").unwrap();
        telnet_stream.write(passwd.as_bytes()).unwrap();
        telnet_stream.write(b"\n").unwrap();
        telnet_stream.write("?".as_bytes()).unwrap();
        telnet_stream.write(b"\n").unwrap();

        for _ in 0..50 {
            let event =
                task::block_in_place(|| telnet_stream.read_timeout(Duration::from_micros(100)));
            match event {
                Ok(Event::Data(buffer)) => {
                    let received_data = String::from_utf8_lossy(&buffer).to_string();
                    println!("{}", received_data);
                    let date = Local::now();
                    let timestamp = date.format("%Y-%m-%d %H:%M:%S").to_string();
                    writeln!(log_file, "[{}] {}", timestamp, received_data).unwrap();
                    //tx.send("1").await.unwrap();
                }
                Ok(_) => {}
                Err(e) => {
                    eprintln!("Error reading from telnet stream: {:?}", e);
                    break;
                }
            }
        }
        return telnet_stream;
    });
    println!("1111\n");
    let telnet_stream = handle.await.unwrap();

    println!("22222\n");
    /*
    tokio::spawn(async move {
        while let Some(received_data) = rx.recv().await {
            println!("{}", received_data);
            //处理收到的数据
        }
    });*/
    Ok(telnet_stream)
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
