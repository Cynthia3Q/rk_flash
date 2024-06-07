#![deny(unsafe_code)]

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;
slint::include_modules!();

mod ssh_session;
mod telnet_sessions;

#[derive(Default, Debug, Clone)]
struct SshInfo {
    ip: String,
    username: String,
    password: String,
    board_type: String,
    slot: String,
}

impl From<ssh_info> for SshInfo {
    fn from(ssh_info: ssh_info) -> Self {
        Self {
            ip: ssh_info.ip.to_string(),
            username: ssh_info.username.to_string(),
            password: ssh_info.password.to_string(),
            board_type: ssh_info.board_type.to_string(),
            slot: ssh_info.slot.to_string(),
        }
    }
}

use std::{ops::ControlFlow, rc::Rc};

use slint::{Model, StandardListViewItem, VecModel};

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(start))]
#[tokio::main]
pub async fn main() -> Result<(), slint::PlatformError> {
    // This provides better error messages in debug mode.
    // It's disabled in release mode so it doesn't bloat up the file size.
    #[cfg(all(debug_assertions, target_arch = "wasm32"))]
    console_error_panic_hook::set_once();

    // Disable gettext on macOS due to https://github.com/Koka/gettext-rs/issues/114
    #[cfg(not(target_os = "macos"))]
    //slint::init_translations!(concat!(env!("CARGO_MANIFEST_DIR"), "/lang/"));
    let app = App::new().unwrap();

    let (tx, rx) = std::sync::mpsc::channel();
    app.global::<ControlsPageAdapter>().on_connection_apply({
        move |connection| {
            println!("pressed!!");
            println!(
                "{} {} {} {} {}",
                connection.ip,
                connection.username,
                connection.password,
                connection.board_type,
                connection.slot
            );
            tx.send(connection.clone()).unwrap();
        }
    });

    app.global::<FunctionsPageAdapter>()
        .on_function_test_start({
            let app_weak = app.as_weak();
            move || {
                let app = app_weak.upgrade().unwrap();
                let connection: SshInfo =
                    app.global::<ControlsPageAdapter>().get_connection().into();
                tokio::spawn(session_initialize(connection));
            }
        });

    //let connection: SshInfo = app.global::<ControlsPageAdapter>().get_connection().into();
    //let connection_info = rx.try_recv();
    //session_initialize(connection_info);
    app.run()
}

async fn session_initialize(connection: SshInfo) -> tokio::io::Result<()> {
    ssh_session::connect_ssh(
        &connection.ip,
        22,
        &connection.username,
        &connection.password,
    )
    .expect(" connect ssh err");

    telnet_sessions::connect_telnet(
        connection.ip,
        6800,
        connection.username,
        connection.password,
    )
    .expect("connet telnet err");
    Ok(())
}
