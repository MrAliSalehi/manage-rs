use crate::models::{Config, ServerMetric};
use agent_shared::{AddServerMetric, ClientMessage, ClientMessageDetail, ServerMessage, Signal};
use machine_info::Machine;
use message_io::network::{Endpoint, NetEvent, Transport};
use message_io::node;
use message_io::node::{NodeEvent, NodeHandler, NodeListener};
use serde::{Deserialize, Serialize};
use std::cell::{Cell, RefCell};
use std::net::SocketAddr;
use std::ops::Deref;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{sleep, JoinHandle};
use std::time::Duration;

mod models;
mod systemd_manager;
pub const VERSION_NUMBER: u16 = 1;

fn main() -> eyre::Result<()> {
    let mut args = std::env::args();
    if let Some(first) = args.nth(1) {
        if first.eq("init") {
            // the init without token and API_URL wont happen!
            let token = args.nth(2).unwrap();
            let api_host = args.nth(3).unwrap();
            init(token, api_host)?;
            return Ok(());
        }
    }
    //let config = serde_json::from_str::<Config>(&std::fs::read_to_string(CONFIG_FILE_PATH)?)?;

    // You can change the transport to Udp or Ws (WebSocket).
    let endpoint = "127.0.0.1:3939";
    let token = "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxIiwiaWF0IjoxNzM5NjQzOTEzLCJleHAiOjB9.RHOj1JdGfqzFo5L80WQc68_KqGibOkDrGXQIfa4i-0g";
    //let endpoint = config.api_host;

    let disconnected = Arc::new(AtomicBool::new(false));
    loop {
        let (handler, listener) = node::split::<()>();
        let (server_id, local_addr) = handler.network().connect(Transport::FramedTcp, endpoint)?;
        let handler_cl = handler.clone();
        run_metric_thread(
            token.to_owned(),
            server_id,
            handler_cl,
            disconnected.clone(),
        );

        run_listener(
            handler,
            listener,
            endpoint,
            local_addr,
            disconnected.clone(),
        );
        if !disconnected.load(Ordering::Relaxed) {
            break;
        }
        sleep(Duration::from_secs(10));
    }

    Ok(())
}

fn run_metric_thread(
    token: String,
    server_id: Endpoint,
    handler_cl: NodeHandler<()>,
    dc: Arc<AtomicBool>,
) -> JoinHandle<()> {
    std::thread::spawn(move || {
        let mut m = Machine::new();

        loop {
            if dc.load(Ordering::Relaxed) {
                sleep(Duration::from_secs(10));
                continue;
            }
            let system_info = m.system_info();
            let system_status = m.system_status().ok();
            let message = ClientMessage {
                token: Some(token.to_owned()),
                message: ClientMessageDetail::UpdateMetric {
                    metric: AddServerMetric {
                        system_status,
                        system_info,
                    },
                },
            };

            let output_data = bincode::serialize(&message).unwrap();
            handler_cl.network().send(server_id, &output_data);
            sleep(Duration::from_secs(10));
        }
        return ();
    })
}

fn run_listener<T: Send>(
    handler: NodeHandler<T>,
    listener: NodeListener<T>,
    endpoint: &str,
    local_addr: SocketAddr,
    disconnected: Arc<AtomicBool>,
) {
    listener.for_each(move |event| match event.network() {
        NetEvent::Connected(server_id, established) => {
            if established {
                println!(
                    "Connected to server at {}\nClient identified by local port: {}",
                    server_id.addr(),
                    local_addr.port()
                );
                disconnected.store(false, Ordering::Relaxed);
            } else {
                println!("cant connect to server at {}, retrying...", endpoint);
                disconnected.store(true, Ordering::Relaxed);
                handler.stop();
                return;
            }
        }
        NetEvent::Message(_, input_data) => {
            let message = bincode::deserialize::<ServerMessage>(&input_data);
            println!(
                "msg: {message:?}\nraw:{:?}",
                String::from_utf8_lossy(input_data)
            )
        }
        NetEvent::Disconnected(_) => {
            println!("Server is disconnected, trying to reconnect...");
            disconnected.store(true, Ordering::Relaxed);
            handler.stop();
        }
        _ => {}
    });
}

const AGENT_RESOURCE_DIR: &str = "/usr/share/managers_agent";
const CONFIG_FILE_PATH: &str = "/usr/share/managers_agent/agent_config.json";
fn init(token: String, api_host: String) -> eyre::Result<()> {
    let config = Config {
        api_host,
        auth_token: token,
    };
    std::fs::write(CONFIG_FILE_PATH, serde_json::to_string(&config)?.as_bytes())?;
    systemd_manager::init_systemd()?;
    Ok(())
}
