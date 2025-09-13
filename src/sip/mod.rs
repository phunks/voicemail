use crate::lazy_regex;
use crate::sip::play_file::recved_call;
use crate::utils::local_time;
use crate::web::db::Pool;
use anyhow::{Error, Result};
use clap::Parser;
use play_file::{build_rtp_conn, play_audio_file, play_echo, write_pcm};
use rsip::Header;
use rsip::{prelude::HeadersExt, typed::MediaType};
use rsipstack::{
    EndpointBuilder, Error as RsError,
    dialog::{
        authenticate::Credential,
        dialog::{Dialog, DialogState, DialogStateReceiver, DialogStateSender},
        dialog_layer::DialogLayer,
        registration::Registration,
        server_dialog::ServerInviteDialog,
    },
    transaction::{TransactionReceiver, endpoint::EndpointInnerRef},
    transport::{TransportLayer, udp::UdpConnection},
};
use std::{
    env,
    net::IpAddr,
    sync::{Arc, LazyLock},
    time::Duration,
};
use tokio::{
    select,
    sync::{Mutex, mpsc::unbounded_channel},
    time::sleep,
};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info};

mod play_file;

lazy_regex!(
    RE: r"<[^:]+:(?<a>[^@]+)@.*$"
);

#[derive(Debug, Clone)]
struct MediaSessionOption {
    pub cancel_token: CancellationToken,
    pub from: Option<Header>,
    pub external_ip: Option<String>,
    pub rtp_start_port: u16,
    pub echo: bool,
    pub rec: bool,
}

impl MediaSessionOption {
    pub fn set_header(&mut self, header: Header) {
        self.from = Some(header);
    }
    pub fn get_header(&self) -> Option<Header> {
        self.from.clone()
    }
}

/// A SIP client example that sends a REGISTER request to a SIP server.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// SIP port
    #[arg(long, default_value = "5060")]
    port: u16,

    /// SIP port
    #[arg(long, default_value = "5061")]
    rtp_start_port: u16,

    /// echo
    #[arg(long, default_value = "false")]
    echo: bool,

    /// voice record
    #[arg(long, default_value = "false")]
    rec: bool,

    /// External IP address
    #[arg(long)]
    external_ip: Option<String>,

    /// SIP server address
    #[arg(long)]
    sip_server: Option<String>,

    /// SIP user
    #[arg(long)]
    user: Option<String>,

    /// SIP password
    #[arg(long)]
    password: Option<String>,
}

pub fn get_first_non_loopback_interface() -> Result<IpAddr> {
    for i in get_if_addrs::get_if_addrs()? {
        if !i.is_loopback() {
            match i.addr {
                get_if_addrs::IfAddr::V4(ref addr) => return Ok(std::net::IpAddr::V4(addr.ip)),
                _ => continue,
            }
        }
    }
    Err(Error::from(RsError::Error(
        "No IPV4 interface found".to_string(),
    )))
}

pub async fn voice_mail(pool: Pool) -> Result<()> {
    if let Err(e) = dotenv::dotenv() {
        info!("Failed to load .env file: {}", e);
    }

    let args = Args::parse();

    info!("Starting SIP client");

    let mut sip_server = args
        .sip_server
        .unwrap_or(env::var("SIP_SERVER").unwrap_or_default());

    if !sip_server.starts_with("sip:") && !sip_server.starts_with("sips:") {
        sip_server = format!("sip:{}", sip_server);
    }

    let sip_server = rsip::Uri::try_from(sip_server).ok();
    let sip_username = args
        .user
        .unwrap_or(env::var("SIP_USERNAME").unwrap_or_default());
    let sip_password = args
        .password
        .unwrap_or(env::var("SIP_PASSWORD").unwrap_or_default());

    let token = CancellationToken::new();
    let opt = Arc::new(Mutex::new(MediaSessionOption {
        cancel_token: token.clone(),
        from: None,
        external_ip: args.external_ip.clone(),
        rtp_start_port: args.rtp_start_port,
        echo: args.echo,
        rec: args.rec,
    }));

    let transport_layer = TransportLayer::new(token.clone());

    let external_ip = args
        .external_ip
        .unwrap_or(env::var("EXTERNAL_IP").unwrap_or_default());

    let external = if external_ip.is_empty() {
        None
    } else {
        Some(format!("{}:{}", external_ip, args.port).parse()?)
    };

    let addr = get_first_non_loopback_interface().expect("get first non loopback interface");
    let connection = UdpConnection::create_connection(
        format!("{}:{}", addr, args.port).parse()?,
        external,
        Some(token.child_token()),
    )
    .await?;

    transport_layer.add_transport(connection.into());

    let endpoint = EndpointBuilder::new()
        .with_cancel_token(token.clone())
        .with_transport_layer(transport_layer)
        .build();

    let credential = Credential {
        username: sip_username.clone(),
        password: sip_password,
        realm: None,
    };

    let incoming = endpoint.incoming_transactions()?;
    let dialog_layer = Arc::new(DialogLayer::new(endpoint.inner.clone()));

    let (state_sender, state_receiver) = unbounded_channel();

    let first_addr = endpoint
        .get_addrs()
        .first()
        .ok_or(crate::Error::Error("no address found".to_string()))?
        .clone();

    let contact = rsip::Uri {
        scheme: Some(rsip::Scheme::Sip),
        auth: Some(rsip::Auth {
            user: sip_username,
            password: None,
        }),
        host_with_port: first_addr.addr,
        params: vec![],
        headers: vec![],
    };

    select! {
        _ = endpoint.serve() => {
            info!("user agent finished");
        }
        r = process_registration(endpoint.inner.clone(), sip_server, credential.clone(), token.clone()) => {
            info!("register loop finished {:?}", r);
        }
        r = process_incoming_request(dialog_layer.clone(), incoming, state_sender.clone(), contact.clone(), opt.clone()) => {
            info!("serve loop finished {:?}", r);
        }
        r = process_dialog(dialog_layer.clone(), state_receiver, pool, opt.clone()) => {
            info!("dialog loop finished {:?}", r);
        }
    }
    Ok(())
}

async fn process_registration(
    endpoint: EndpointInnerRef,
    sip_server: Option<rsip::Uri>,
    credential: Credential,
    cancel_token: CancellationToken,
) -> Result<()> {
    let sip_server = match sip_server {
        Some(uri) => uri,
        None => {
            cancel_token.cancelled().await;
            return Ok(());
        }
    };

    let mut registration = Registration::new(endpoint, Some(credential));
    loop {
        let resp = registration.register(sip_server.clone(), None).await?;
        debug!("received response: {}", resp.to_string());
        if resp.status_code != rsip::StatusCode::OK {
            return Err(Error::from(RsError::Error(
                "Failed to register".to_string(),
            )));
        }
        sleep(Duration::from_secs(registration.expires().max(50) as u64)).await;
    }
}

async fn process_incoming_request(
    dialog_layer: Arc<DialogLayer>,
    mut incoming: TransactionReceiver,
    state_sender: DialogStateSender,
    contact: rsip::Uri,
    opt: Arc<Mutex<MediaSessionOption>>,
) -> Result<()> {
    while let Some(mut tx) = incoming.recv().await {
        info!("Received transaction: {:?}", tx.key);

        let recv_from = tx
            .original
            .headers
            .iter()
            .filter(|x| x.to_string().starts_with("From"))
            .collect::<Vec<_>>();
        opt.lock().await.set_header(recv_from[0].clone());

        if tx.original.to_header()?.tag()?.as_ref().is_some() {
            match dialog_layer.match_dialog(&tx.original) {
                Some(mut d) => {
                    tokio::spawn(async move {
                        d.handle(&mut tx).await?;
                        Ok::<_, Error>(())
                    });
                    continue;
                }
                None => {
                    info!("dialog not found: {}", tx.original);
                    // tx.reply(rsip::StatusCode::CallTransactionDoesNotExist)
                    //     .await?;
                    continue;
                }
            }
        }
        // out dialog, new server dialog
        match tx.original.method {
            rsip::Method::Invite | rsip::Method::Ack => {
                let mut dialog = match dialog_layer.get_or_create_server_invite(
                    &tx,
                    state_sender.clone(),
                    None,
                    Some(contact.clone()),
                ) {
                    Ok(d) => d,
                    Err(e) => {
                        // 481 Dialog/Transaction Does Not Exist
                        info!("Failed to obtain dialog: {:?}", e);
                        tx.reply(rsip::StatusCode::CallTransactionDoesNotExist)
                            .await?;
                        continue;
                    }
                };
                tokio::spawn(async move {
                    dialog.handle(&mut tx).await?;
                    Ok::<_, Error>(())
                });
            }
            _ => {
                info!("Received request: {:?}", tx.original.method);
                tx.reply(rsip::StatusCode::OK).await?;
            }
        }
    }
    Ok::<_, Error>(())
}

async fn process_dialog(
    dialog_layer: Arc<DialogLayer>,
    state_receiver: DialogStateReceiver,
    pool: Pool,
    opt: Arc<Mutex<MediaSessionOption>>,
) -> Result<()> {
    let mut state_receiver = state_receiver;
    while let Some(state) = state_receiver.recv().await {
        match state {
            DialogState::Calling(id) => {
                info!("Calling dialog {}", id);
                let dialog = match dialog_layer.get_dialog(&id) {
                    Some(d) => d,
                    None => {
                        info!("Dialog not found {}", id);
                        continue;
                    }
                };
                match dialog {
                    Dialog::ServerInvite(d) => {
                        // play example pcmu of handling incoming call
                        process_invite(opt.clone(), pool.clone(), d).await?;
                    }
                    Dialog::ClientInvite(_) => {
                        info!("Client invite dialog {}", id);
                    }
                }
            }
            DialogState::Early(id, resp) => {
                info!("Early dialog {} {}", id, resp);
            }
            DialogState::Terminated(id, reason) => {
                info!("Dialog terminated {} {:?}", id, reason);
                dialog_layer.remove_dialog(&id);
            }
            _ => {
                info!("Received dialog state: {}", state);
            }
        }
    }
    Ok(())
}

async fn process_invite(
    opt: Arc<Mutex<MediaSessionOption>>,
    pool: Pool,
    dialog: ServerInviteDialog,
) -> Result<()> {
    let ssrc = rand::random::<u32>();

    let caller = RE
        .captures_iter(&opt.lock().await.get_header().unwrap().to_string())
        .map(|cap| cap["a"].to_owned())
        .collect::<String>();

    let body = String::from_utf8_lossy(dialog.initial_request().body()).to_string();
    let offer = match sdp_rs::SessionDescription::try_from(body.as_str()) {
        Ok(s) => s,
        Err(e) => {
            info!("Failed to parse offer SDP: {:?} {}", e, body);
            return Err(Error::from(RsError::Error(
                "Failed to parse SDP".to_string(),
            )));
        }
    };

    let peer_addr = match offer.connection {
        Some(c) => c.connection_address.base,
        None => {
            info!("No connection address in offer SDP");
            return Err(Error::from(RsError::Error(
                "No connection address in offer SDP".to_string(),
            )));
        }
    };

    let peer_port = offer
        .media_descriptions
        .first()
        .map(|m| m.media.port)
        .ok_or(Error::from(RsError::Error(
            "No audio port in offer SDP".to_string(),
        )))?;
    let payload_type = offer
        .media_descriptions
        .first()
        .and_then(|m| m.media.fmt.parse::<u8>().ok())
        .unwrap_or(0);

    let (conn, answer) = build_rtp_conn(opt.clone(), ssrc, payload_type).await?;

    let headers = vec![rsip::typed::ContentType(MediaType::Sdp(vec![])).into()];
    dialog.accept(Some(headers), Some(answer.clone().into()))?;

    info!(
        "Accepted call with answer SDP peer address: {} port: {} payload_type: {}",
        peer_addr, peer_port, payload_type
    );

    let peer_addr = format!("{}:{}", peer_addr, peer_port);
    let rtp_token = dialog.cancel_token().child_token();
    let lock = opt.lock().await;
    let rec = lock.rec;
    let echo = lock.echo;

    tokio::spawn(async move {
        select! {
            _ = async {
                let headers = vec![rsip::typed::ContentType(MediaType::Sdp(vec![])).into()];
                match dialog.accept(Some(headers), Some(answer.clone().into())) {
                    Ok(_) => info!("Accepted call with answer SDP peer address: {} port: {} payload_type: {}", peer_addr, peer_port, payload_type),
                    Err(e) => {
                        error!("Failed to accept call: {:?}", e);
                        return;
                    }
                }
                if echo {
                    play_echo(conn, rtp_token).await.expect("play echo");
                } else if rec {
                    let id = local_time().parse::<usize>().unwrap();
                    recved_call(&pool, id, caller).await.expect("");
                    play_audio_file(conn.clone(), rtp_token.clone(), ssrc, "voicemail", peer_addr, payload_type)
                        .await
                        .expect("play example file");
                    write_pcm(conn, pool, rtp_token, id).await.expect("rec voice");
                } else {
                    play_audio_file(conn, rtp_token, ssrc, "voicemail", peer_addr, payload_type)
                        .await
                        .expect("play example file");
                }
            } => {
                info!("answer receiver finished");
            }
        }
        dialog.bye().await.expect("send BYE");
    });
    Ok(())
}
