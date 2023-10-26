use tokio::io::{AsyncReadExt, AsyncWriteExt};
use crate::utils::{lookup_ipv4, new_tls_stream, generate_uuid};
use crate::proto;
use serde_json;
use tokio::select;
use std::net::SocketAddr;
use tokio::net::TcpStream;
use tokio_rustls::client::TlsStream as TlsClientStream;
use crate::{
    tls_client_read_to, 
    error::AppTypeResult, AppOption, AppResult,MappingConfig,AppError
};

const META_MSG_END_FLAG: [u8;1] = [0;1];

pub async fn start_client_node(option: AppOption) -> AppResult<()> {
    log::debug!("proxy client running ...");
    let ca_file = option.ca_cert.clone().unwrap();
    let cert_file = option.cert.clone().unwrap();
    let key_file = option.key.clone().unwrap();

    log::info!("connect to server: {}", option.server.unwrap().to_string());
    let server_signal_addr = SocketAddr::new(option.server.clone().unwrap(), option.signal_port);
    let mut tls_stream = new_tls_stream("localhost", server_signal_addr, &ca_file, &cert_file, &key_file).await;
    let client_id = generate_uuid();
    let client_name = String::from("client1");

    let meta_msg:String = format!("main:{}:{}", client_name, client_id);
    tls_stream.write(meta_msg.clone().as_bytes()).await.unwrap();
    tls_stream.write(&[0; 1]).await.unwrap();

    loop {
        let mut recv_buffer: Vec<u8> = Vec::new();
        let result = tls_client_read_to(&mut tls_stream, &mut recv_buffer, 0).await;
        match result {
            Ok(size) => {
                log::info!("main recv size: {}", size);
            },
            Err(e) => {
                let err_kind = e.kind();
                match err_kind {
                    std::io::ErrorKind::UnexpectedEof => {
                        log::info!("server connection closed");
                        return Ok(());
                    },
                    _ => {
                        log::error!("main error: {}", e);
                        return Err(e.into());
                    }
                }
                
            }
        }        
        let res = String::from_utf8(recv_buffer).unwrap();
        log::debug!("client read data: {}", res);

        let proto_cmd: proto::ProtoCmd = serde_json::from_str(&res).unwrap();
        match proto_cmd {
            proto::ProtoCmd::Request(req) => {
                let status: String = String::from("Ok");
                let message: String = String::from("proccess success");
                if let Some(req_body) = req.body {
                    match req_body {
                        proto::ProtoCmdBody::ProxyRequest{bind_id, client, mapping} => {
                            client_forward(option.clone(), bind_id, client, &mapping).await.unwrap();
                        }
                        _ => {
                            //TODO:
                        }
                    }
                }
                
                let rspcmd = proto::ProtoCmd::Response(proto::ProtoCmdResponse::new(req.id.clone(), req.cmd_type.clone(), status, message, None));
                let json = serde_json::to_string(&rspcmd).unwrap();
                log::debug!("client send data: {}", json);
                tls_stream.write(json.as_bytes()).await.unwrap();
                tls_stream.write(&META_MSG_END_FLAG).await.unwrap();
            },
            proto::ProtoCmd::Response(rsp) => {

            }
        }
    }
}

async fn client_forward(option: AppOption, bind_id:String, client:String, mapping: &MappingConfig) -> AppResult<()>  {
    let ca_file = option.ca_cert.clone().unwrap();
    let cert_file = option.cert.clone().unwrap();
    let key_file = option.key.clone().unwrap();
    
    let server_data_addr = SocketAddr::new(option.server.clone().unwrap(), option.data_port);
    let dst_addr:SocketAddr = mapping.forward.parse().unwrap();

    let meta_msg:String = format!("data:{}:{}", client, bind_id);
    
    tokio::spawn(async move { 
        log::debug!("connect to {}", server_data_addr.to_string());
        let mut tls_fwd_stream = new_tls_stream("localhost", server_data_addr, &ca_file, &cert_file, &key_file).await;
        log::debug!("connected to {}", server_data_addr.to_string());
        tls_fwd_stream.write(meta_msg.clone().as_bytes()).await.unwrap();
        tls_fwd_stream.write(&META_MSG_END_FLAG).await.unwrap();
        tls_fwd_stream.flush().await.unwrap();
        log::debug!("connect to app {:?}", dst_addr);
        let mut dst_stream = TcpStream::connect(dst_addr).await.unwrap();
        log::debug!("connected to app {:?}", dst_addr);
        let result = client_data_forward(&mut tls_fwd_stream, &mut dst_stream).await;
        match result {
            Ok(size) => {
                log::info!("proccess tx[{}] success", bind_id)
            },
            Err(e) => {
                log::error!("proccess tx[{}] error: {}", bind_id, e)
            }
        }                       
    });
   
    Ok(())
}

async fn client_data_forward(tls_stream:&mut TlsClientStream<TcpStream>, tcp_stream: &mut TcpStream) -> Result<usize, tokio::io::Error>  {
    let mut tls_recv_buffer:[u8; 1024] = [0; 1024];
    let mut dst_recv_buffer:[u8; 1024] = [0; 1024];
    log::trace!("start process data ....");
    loop {
        select! {
            fwd_res = tls_stream.read(&mut tls_recv_buffer) => {
                match fwd_res {
                    Ok(size) => {
                        if size > 0 {
                            log::trace!("send data to app size:{}", size);
                            tcp_stream.write_all(&tls_recv_buffer[0..size]).await?;
                            tcp_stream.flush().await?;
                            log::trace!("sended data to app size:{}", size);
                        } else {
                            //log::info!("forward connection closed");
                            //break;
                        }
                    },
                    Err(e) => {
                        let err_kind = e.kind();
                        match err_kind {
                            std::io::ErrorKind::UnexpectedEof => {
                                log::info!("tls stream  connection closed");
                                break;
                            },
                            _ => {
                                log::error!("read from tls stream error: {}", e);
                                return Err(e);
                            }
                        }
                    }
                }
                
            },

            dst_res = tcp_stream.read(&mut dst_recv_buffer) => {
                match dst_res {
                    Ok(size) => {
                        if size > 0 {

                            log::trace!("send data to forward port size:{}", size);
                            tls_stream.write_all(&dst_recv_buffer[0..size]).await?;
                            tls_stream.flush().await?;
                            log::trace!("sended data to forward port size:{}", size);
                        } else {
                            //
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
            }
        }
    }

    Ok(0)
}