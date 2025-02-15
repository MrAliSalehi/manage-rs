use crate::libs::api_response::ApiResponse;
use crate::libs::app_config::AppConfigRef;
use crate::libs::shared_state::SharedState;
use crate::libs::TokenClaims;
use crate::models::server_metric::ServerMetric;
use crate::prelude::Res;
use agent_shared::{ClientMessage, ClientMessageDetail, ServerMessage, Signal};
use chrono::Utc;
use jsonwebtoken::{decode, DecodingKey, Validation};
use message_io::network::{Endpoint, NetEvent, Transport};
use message_io::node;
use message_io::node::{NodeEvent, NodeHandler};
use std::net::ToSocketAddrs;
use std::ops::Deref;
use std::sync::LazyLock;

static V: LazyLock<Validation> = LazyLock::new(|| {
    let mut v = Validation::default();
    v.validate_exp = false;
    v
});

pub fn run(state: SharedState) -> Res {
    let secret = state.app_config.pwd.as_bytes().to_owned();
    std::thread::spawn(|| {
        log::info!("creating sub-server io");
        let (handler, listener) = node::split::<Signal>();
        let addr = ("0.0.0.0", 3939).to_socket_addrs()?.next().unwrap();
        match handler.network().listen(Transport::FramedTcp, addr) {
            Ok((id, real_addr)) => println!("sub server listener({id}) running at {}", real_addr),
            Err(_) => {
                println!("Can not listening at {}", addr);
                return Ok(());
            }
        }

        listener.for_each(move |event| match event {
            NodeEvent::Network(nw) => match nw {
                NetEvent::Connected(endpoint, _) => {
                    println!("Client ({}) connected", endpoint.addr());
                }
                NetEvent::Message(endpoint, input_data) => {
                    let Ok(message) = bincode::deserialize::<ClientMessage>(&input_data) else {
                        log::info!(
                            "received unknown data from {endpoint}: {:?}",
                            String::from_utf8_lossy(input_data)
                        );
                        return;
                    };
                    log::info!("{message:?}");
                    let Some(claims) = authenticate_client(&secret, &message.token) else {
                        log::info!("removing unauthorized access: {}", endpoint.addr());
                        handler.network().remove(endpoint.resource_id());
                        return;
                    };
                    let result =
                        process_message(state.clone(), &handler, message, endpoint, claims);
                    if let Err(e) = result {
                        log::error!("{e:?}");
                        return;
                    }
                }
                NetEvent::Disconnected(endpoint) => {
                    println!("Client ({}) disconnected", endpoint.addr());
                }
                _ => {}
            },
            NodeEvent::Signal(signal) => {
                log::info!("received signal {signal:?}");
            }
        });

        eyre::Result::<()>::Ok(())
    });
    Ok(())
}

fn process_message(
    state: SharedState,
    handler: &NodeHandler<Signal>,
    message: ClientMessage,
    endpoint: Endpoint,
    claims: TokenClaims,
) -> Res {
    log::info!(
        "msg: (from {}), (token {}), (msg {message:?})",
        endpoint.addr(),
        claims.sub
    );
    match message.message {
        ClientMessageDetail::Ping => {
            let o = bincode::serialize(&ServerMessage::Ping)?;
            handler.network().send(endpoint, &o);
        }
        ClientMessageDetail::UpdateMetric { metric } => {
            let now = Utc::now().naive_utc();
            log::info!("received metrics from {} in [{} UTC]", endpoint.addr(), now);
            state.db_driver.add_metric(ServerMetric {
                system_status: metric.system_status,
                system_info: metric.system_info,
                server_id: claims.sub,
                time: now,
            })?;
        }
    }
    Ok(())
}

fn authenticate_client(secret: &[u8], token: &Option<String>) -> Option<TokenClaims> {
    Some(
        decode::<TokenClaims>(
            token.as_ref()?,
            &DecodingKey::from_secret(&secret),
            V.deref(),
        )
        .inspect_err(|e| log::error!("{e:?}"))
        .ok()?
        .claims,
    )
}
