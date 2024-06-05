use glib::variant::Handle;
// init.rs
use gtk::prelude::*;
use gtk::{Align, Box, Button, Entry, Label, Orientation, Window, WindowPosition, WindowType};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use telnet::Telnet;
//ssh crate
use crate::ssh_session::connect_ssh;
use crate::telnet_sessions::{self, connect_telnet};
use gtk::{CssProvider, StyleContext, STYLE_PROVIDER_PRIORITY_APPLICATION};
use std::path::Path;

//use tokio_stream::wrappers::ReceiverStream;
// 定义一个新的 Trait
pub async fn create_initial_window() -> std::io::Result<()> {
    let window = Window::new(WindowType::Toplevel);
    window.set_title("TestingGUI");
    window.set_default_size(700, 500);

    let provider = CssProvider::new();

    // 将你的 CSS 样式添加到 CssProvider
    provider
        .load_from_path("ui/style.css")
        .expect("Failed to load CSS");
    let context = window.get_style_context();
    context.add_provider(&provider, STYLE_PROVIDER_PRIORITY_APPLICATION);

    let vbox = Box::new(Orientation::Vertical, 0);
    vbox.set_halign(Align::Start); // 设置vbox在水平方向上靠左
    vbox.set_valign(Align::Start); // 设置vbox在垂直方向上靠上
    window.add(&vbox);

    let button_box = Box::new(Orientation::Vertical, 0);
    button_box.set_spacing(10); // 设置按钮之间的间距

    let ssh_button = Button::with_label("SSH");
    ssh_button.set_size_request(100, 50); // 设置按钮的宽度和高度
    ssh_button.set_halign(Align::Start); // 设置按钮在水平方向上靠左
    ssh_button.connect_clicked(|_| {
        let ssh_window = Rc::new(RefCell::new(Window::new(WindowType::Toplevel)));
        ssh_window.borrow_mut().set_title("SSH Connection");
        ssh_window.borrow_mut().set_default_size(500, 270);

        let vbox = Box::new(Orientation::Vertical, 10);
        ssh_window.borrow().add(&vbox);

        let ip_label = Label::new(Some("IP Address:"));
        vbox.add(&ip_label);

        let ip_entry = Entry::new();
        vbox.add(&ip_entry);

        let user_label = Label::new(Some("Username:"));
        vbox.add(&user_label);

        let user_entry = Entry::new();
        vbox.add(&user_entry);

        let passwd_label = Label::new(Some("Password:"));
        vbox.add(&passwd_label);

        let passwd_entry = Entry::new();
        passwd_entry.set_visibility(false); // 设置密码输入框为不可见，以隐藏输入的密码
        vbox.add(&passwd_entry);

        let port_label = Label::new(Some("Port:"));
        vbox.add(&port_label);

        let port_entry = Entry::new();
        port_entry.set_text("22"); // 设置默认端口为22
        vbox.add(&port_entry);

        let connect_button = Button::with_label("Connect");
        connect_button.connect_clicked({
            let ssh_window = ssh_window.clone();
            move |_| {
                let ip = ip_entry.get_text().to_string();
                let user = user_entry.get_text().to_string();
                let passwd = passwd_entry.get_text().to_string();
                let port = port_entry.get_text().to_string().parse::<u16>().unwrap();

                connect_ssh(ip, port, user, passwd).unwrap();

                ssh_window.borrow().close();
            }
        });
        vbox.add(&connect_button);

        ssh_window.borrow_mut().set_position(WindowPosition::Mouse);
        ssh_window.borrow_mut().show_all();
    });
    button_box.add(&ssh_button);

    let telnet_button = Button::with_label("telnet");
    telnet_button.set_size_request(100, 50); // 设置按钮的宽度和高度
    telnet_button.set_halign(Align::Start); // 设置按钮在水平方向上靠左
    telnet_button.connect_clicked(|_| {
        let ssh_window = Rc::new(RefCell::new(Window::new(WindowType::Toplevel)));
        ssh_window.borrow_mut().set_title("SSH Connection");
        ssh_window.borrow_mut().set_default_size(500, 270);

        let vbox = Box::new(Orientation::Vertical, 10);
        ssh_window.borrow().add(&vbox);

        let ip_label = Label::new(Some("IP Address:"));
        vbox.add(&ip_label);

        let ip_entry = Entry::new();
        ip_entry.set_text("192.168.126.123"); // 设置默认ip
        vbox.add(&ip_entry);

        let user_label = Label::new(Some("Username:"));
        vbox.add(&user_label);

        let user_entry = Entry::new();
        user_entry.set_text("root");
        vbox.add(&user_entry);

        let passwd_label = Label::new(Some("Password:"));
        vbox.add(&passwd_label);

        let passwd_entry = Entry::new();
        passwd_entry.set_text("root");
        passwd_entry.set_visibility(false); // 设置密码输入框为不可见，以隐藏输入的密码
        vbox.add(&passwd_entry);

        let port_label = Label::new(Some("Port:"));
        vbox.add(&port_label);

        let port_entry = Entry::new();
        port_entry.set_text("6800"); // 设置默认端口为22
        vbox.add(&port_entry);

        let connect_button = Button::with_label("Connect");
        connect_button.connect_clicked({
            let ssh_window = ssh_window.clone();
            move |_| {
                let ip = ip_entry.get_text().to_string();
                let user = user_entry.get_text().to_string();
                let passwd = passwd_entry.get_text().to_string();
                let port = port_entry.get_text().to_string().parse::<u16>().unwrap();

                let telnet_session: Arc<Mutex<Telnet>> = Arc::new(Mutex::new(
                    Telnet::connect(format!("{}:{}", ip, port), 1024).unwrap(),
                ));
                let telnet_session_clone = Arc::clone(&telnet_session);
                // 在新的任务中运行异步代码
                let connect_task = tokio::spawn(async move {
                    *telnet_session_clone.lock().unwrap() =
                        connect_telnet(ip.clone(), port, user.clone(), passwd.clone())
                            .await
                            .unwrap();
                    telnet_session_clone
                        .lock()
                        .unwrap()
                        .write("set ?\n".as_bytes())
                        .unwrap();

                    connect_telnet(ip, port, user, passwd).await.unwrap();
                });
                let future = async move {
                    connect_task.await.unwrap();
                    let mut telnet_session = telnet_session.lock().unwrap();
                    let telnet = &mut *telnet_session;
                    telnet.write("set ?\n".as_bytes()).unwrap();
                    println!("333333\n");
                };

                ssh_window.borrow().close();
            }
        });
        vbox.add(&connect_button);

        ssh_window.borrow_mut().set_position(WindowPosition::Mouse);
        ssh_window.borrow_mut().show_all();
    });
    button_box.add(&telnet_button);

    vbox.add(&button_box); // 将button_box添加到vbox中

    window.connect_delete_event(|_, _| {
        gtk::main_quit();
        Inhibit(false)
    });
    window.show_all();

    Ok(())
}
