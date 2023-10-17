use tokio::net::TcpListener;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use crate::utils::{new_tls_acceptor};
use crate::{proto, mappings};
use serde_json;
use tokio::sync::{mpsc,oneshot,watch};

use tokio::select;
use tokio::time:: {
    sleep, Duration
};
use std::collections::HashMap;
use std::net::SocketAddr;
use tokio::net::TcpStream;
use tokio_rustls::{
    server::TlsStream as TlsServerStream,
};
use crate::{
    get_datetime14,
    generate_uuid,
    tls_server_read_to,
};
use crate::{
    error::AppTypeResult, AppOption, AppResult,MappingConfig,AppError
};

const META_MSG_END_FLAG: [u8;1] = [0;1];
const FORWARD_CONNECTION_BIND_TIMEOUT: u64 = 5;
const MAIN_CONNECTION_KEEPALIVE_TIMEOUT: u64 = 10;

pub async fn start_server_node(option: AppOption, main_cli_rx: watch::Receiver<String>) -> AppResult<()> {
    let datetime =  get_datetime14();
    log::info!("proxy server running ...");
    let ca_file = option.ca_cert.clone().unwrap();
    let cert_file = option.cert.clone().unwrap();
    let key_file = option.key.clone().unwrap();

    let server_signal_addr = SocketAddr::new(option.listen.clone(), option.signal_port);
    let data_signal_addr = SocketAddr::new(option.listen.clone(), option.data_port);
    let tls_acceptor = new_tls_acceptor(&ca_file, &cert_file, &key_file);

    let main_listener = TcpListener::bind(server_signal_addr).await.unwrap();
    let data_listener = TcpListener::bind(data_signal_addr).await.unwrap();
    let mut bind_queue = HashMap::new();
    
    //let (cmd_tx, mut cmd_rx) = mpsc::channel::<(String, SocketAddr)>(32);

    let (clear_tx, mut clear_rx) = mpsc::channel::<String>(1000);
    let (fwd_tx, mut fwd_rx) = mpsc::channel::<(String, String, TlsServerStream<TcpStream>, SocketAddr)>(1000);
    let (proxy_tx, mut proxy_rx) = mpsc::channel::<(String, String,  oneshot::Sender<(String, String, TlsServerStream<TcpStream>, SocketAddr)>)>(1000);
    let (_socket, _peer_addr)  = main_listener.accept().await.unwrap();
    let mut main_tls_stream = tls_acceptor.accept(_socket).await.unwrap();
    let mut recv_buffer: Vec<u8> = Vec::new();
    let size = tls_server_read_to(&mut main_tls_stream, &mut recv_buffer, 0).await.unwrap();
    let res = String::from_utf8(recv_buffer).unwrap();
    log::info!("Received client connection: {}", res);
    let bind_v:Vec<&str> = res.split(":").collect();
    let stream_type= bind_v[0].to_string();
    if stream_type != "main" {
        log::error!("recv stream type: {}, error", stream_type);
        return Ok(());
    }

    let proxy_task = server_start_proxy(&option.mappings, proxy_tx, main_cli_rx).await.unwrap();
    log::debug!("start proxy ....");
    
    loop {
        let mut recv_buffer: Vec<u8> = Vec::new();
        select! {
            tls_msg = tls_server_read_to(&mut main_tls_stream, &mut recv_buffer, 0) => {
                match tls_msg {
                    Ok(size)=> {
                        if size > 0 {
                            let res = String::from_utf8(recv_buffer).unwrap();
                            log::debug!("recv from client: {}", res);
                            let recv_cmd: proto::ProtoCmd = serde_json::from_str(&res).unwrap();
                        } else {
                            log::debug!("signal connection read {}", size);
                            //return Ok(());
                        }
                    },
                    Err(e) => {
                        let err_kind = e.kind();
                        match err_kind {
                            std::io::ErrorKind::UnexpectedEof => {
                                log::info!("client connection closed");
                                return Ok(());
                            },
                            _ => {
                                log::error!("Failed to receive message from client: {}", e);
                                return Err(e.into());
                            }
                        }
                        
                    }
                }
                
            },
           
            data_accept = data_listener.accept() => {
                let (_socket, _peer_addr) = data_accept.unwrap(); 
                let tls_accept_result = tls_acceptor.accept(_socket).await;
                match tls_accept_result {
                    Ok(mut tls_stream) => {
                        log::debug!("forward: Accepted fwd conn with TLS");

                        let mut recv_buffer: Vec<u8> = Vec::new();
                        let size = tls_server_read_to(&mut tls_stream, &mut recv_buffer, 0).await.unwrap();
                        let res = String::from_utf8(recv_buffer).unwrap();
                        log::debug!("Received from forward connection: {}", res);
                        let bind_v:Vec<&str> = res.split(":").collect();
                        let stream_type= bind_v[0].to_string();
                        if stream_type != "data" {
                            log::error!("Received msg type error: {}", res);
                            continue;
                        }

                        let client_id= bind_v[1].to_string();
                        let bind_id= bind_v[2].to_string();
                        log::debug!("forward: {}", res);

                        fwd_tx.send((bind_id, client_id, tls_stream, _peer_addr)).await.unwrap();
                    },
                    Err(e) => {
                        log::error!("Failed to accept TLS connection: {}", e);
                    }
                }

                
            },

            fwd_msg = fwd_rx.recv() => {
                if let Some(msg) = fwd_msg {
                    let (bind_id, client_id, tls_stream, _peer_addr) = msg;
                    log::debug!("bind request client:{} id:{} ", client_id, bind_id);
                    if bind_queue.contains_key(&bind_id) {
                        let tx: oneshot::Sender<(String, String, TlsServerStream<TcpStream>, SocketAddr)>  = bind_queue.remove(&bind_id).unwrap();
                        if(!tx.is_closed()) {
                            tx.send((bind_id, client_id,  tls_stream, _peer_addr)).unwrap();
                        } else {
                            log::debug!("proxy tx is closed, ignore: {}", bind_id);
                        }
                        
                    } else {
                        log::error!("Cannot find match binding for: {}", bind_id);
                    }
                }
            },

            proxy_msg = proxy_rx.recv() => {
                if let Some(msg) = proxy_msg {
                    let (_id, _mapping_name, _tx) = msg;
                    log::debug!("proxy new id: {}", _id);

                    let proxy_mapping = option.mappings.iter().find(|x| x.name == _mapping_name).unwrap().clone();
                    bind_queue.insert(_id.clone(), _tx);
                    let clear_tx = clear_tx.clone();
                    let cls_bind_id = _id.clone();
                    tokio::spawn(async move {
                        sleep(Duration::from_secs(FORWARD_CONNECTION_BIND_TIMEOUT)).await;
                        if !clear_tx.is_closed() {
                            clear_tx.send(cls_bind_id).await.unwrap();
                        }
                        
                    });
                    
                    let proto_body = proto::ProtoCmdBody::ProxyRequest { bind_id: _id.clone(), client: String::from("client1"), mapping: proxy_mapping};
                    let reqcmd = proto::ProtoCmd::Request(proto::ProtoCmdRequest::new(String::from("conn"), Some(proto_body)));

                    let json = serde_json::to_string(&reqcmd).unwrap();
                    log::trace!("server: send data: {}", json);
                    main_tls_stream.write(json.as_bytes()).await.unwrap();
                    main_tls_stream.write(&META_MSG_END_FLAG).await.unwrap();
                    main_tls_stream.flush().await.unwrap();
                }
            },
            clear_msg = clear_rx.recv() => {
                if let Some(bind_id) = clear_msg {
                    //TODO: optimize id clear
                    if bind_queue.contains_key(&bind_id) {
                        bind_queue.remove(&bind_id);
                        log::error!("clear bind client: {}", bind_id);
                    }
                }
            },
            _ = sleep(Duration::from_secs(MAIN_CONNECTION_KEEPALIVE_TIMEOUT)) => {
                let reqcmd = proto::ProtoCmd::Request(proto::ProtoCmdRequest::new(String::from("keepalive"), None));

                let json = serde_json::to_string(&reqcmd).unwrap();
                log::trace!("server: send data: {}", json);
                main_tls_stream.write(json.as_bytes()).await.unwrap();
                main_tls_stream.write(&META_MSG_END_FLAG).await.unwrap();
                main_tls_stream.flush().await.unwrap();
            }
            
        }
    }

}

async fn server_start_proxy(mappings:& Vec<MappingConfig>
    , proxy_tx: mpsc::Sender<(String, String, oneshot::Sender<(String, String, TlsServerStream<TcpStream>, SocketAddr)>)>
    , maincli_rx: watch::Receiver<String>
) -> Result<(), tokio::io::Error> {
    for mapping in mappings {
        let cli_rx = maincli_rx.clone();
        let proxy_listener = TcpListener::bind(mapping.listen.unwrap()).await.unwrap();
        let proxy_tx2 = proxy_tx.clone();
        let mapping_name = mapping.name.clone();

        tokio::spawn(async move {
            let mut cli_rx = cli_rx.clone();
            loop {
                select! {
                    accept_result = proxy_listener.accept() => {
                        let (mut _socket, mut _peer_addr) = accept_result.unwrap();
                        let bind_id = generate_uuid();
                        let proxy_tx2 = proxy_tx2.clone();
                        let mapping_name = mapping_name.clone();

                        log::debug!("new bind id: {}", bind_id);
                        tokio::spawn(async move {
                            let (tx, rx) = oneshot::channel::<(String, String, TlsServerStream<TcpStream>, SocketAddr)>();
                            let proxy_tx2 = proxy_tx2.clone();
                            proxy_tx2.send((bind_id.clone(), mapping_name, tx)).await.unwrap();   
                            let (_id, _client_id, mut _fw_socket, _fw_peer_addr) = rx.await.unwrap();
                            log::trace!("start process id: {} ------------", bind_id);
                            let result = server_data_forward(&mut _fw_socket, &mut _socket).await;
                            match result {
                                Ok(size) => {
                                    log::info!("proccess tx[{}] success", bind_id)
                                },
                                Err(e) => {
                                    let err_kind = e.kind();
                                    match err_kind {
                                        std::io::ErrorKind::UnexpectedEof => {
                                            log::info!("proccess tx[{}] connection close", bind_id);
                                        },
                                        _ => {
                                            log::error!("proccess tx[{}] error: {}", bind_id, e);
                                        }
                                    }
                                    
                                }
                            }
                        });
                    },
                    cmd_msg = cli_rx.changed() => {
                        log::debug!("proxy task recv app quit msg");
                        break;
                    }
                }
                
            }
        });
    }

    Ok(())
}

async fn server_data_forward(tls_stream:&mut TlsServerStream<TcpStream>, tcp_stream: &mut TcpStream) -> Result<usize, tokio::io::Error>  {
    let mut tls_recv_buffer:[u8; 1024] = [0; 1024];
    let mut dst_recv_buffer:[u8; 1024] = [0; 1024];
    log::trace!("start forward data ....");
    loop {

        select! {
            dst_res = tcp_stream.read(&mut dst_recv_buffer) => {
                match dst_res {
                    Ok(size) => {
                        if size > 0 {
                            log::trace!("send data to forward size:{}", size);
                            tls_stream.write_all(&dst_recv_buffer[0..size]).await.unwrap();
                            tls_stream.flush().await.unwrap();
                            log::debug!("sended data to forward size:{}", size);
                        } else {
                            //log::trace!("proxy client closed");
                            //break;
                        }
                    },
                    Err(e) => {
                        let err_kind = e.kind();
                        match err_kind {
                            std::io::ErrorKind::UnexpectedEof => {
                                log::info!("tcp stream connection closed");
                                break;
                            },
                            _ => {
                                log::error!("read from tcp stream error: {}", e);
                                return Err(e);
                            }
                        }
                    }
                }
            },
            fwd_res = tls_stream.read(&mut tls_recv_buffer) => {
                match fwd_res {
                    Ok(size) => {
                        if size > 0 {
                            log::trace!("send data to source size:{}", size);
                            tcp_stream.write_all(&tls_recv_buffer[0..size]).await.unwrap();
                            tcp_stream.flush().await.unwrap();
                            log::trace!("sended data to source size:{}", size);
                        } else {
                            //log::trace!("forward connection closed");
                            //break;
                        }
                    },
                    Err(e) => {
                        let err_kind = e.kind();
                        match err_kind {
                            std::io::ErrorKind::UnexpectedEof => {
                                log::debug!("tls stream connection closed");
                                break;
                            },
                            _ => {
                                log::error!("read from tls stream error: {}", e);
                                return Err(e);
                            }
                        } 
                    }
                }
            }
            
        }
    }
    
    Ok(0)
}