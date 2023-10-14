use std::fs::File;
use std::io;
use std::io::BufReader;
use std::sync::Arc;
use rustls::{
    RootCertStore,
    server::AllowAnyAuthenticatedClient,
};
use tokio::{net::TcpStream, io::AsyncReadExt};
use tokio_rustls::{
    TlsAcceptor,
    TlsConnector,
    rustls::{self},
    client::TlsStream as TlsClientStream,
    server::TlsStream as TlsServerStream,
};
use tokio::select;
fn load_certs(filename: &str) -> Vec<rustls::Certificate> {
    let certfile = File::open(filename).expect("cannot open certificate file");
    let mut reader = BufReader::new(certfile);
    rustls_pemfile::certs(&mut reader)
        .unwrap()
        .iter()
        .map(|v| rustls::Certificate(v.clone()))
        .collect()
}

fn load_private_key(filename: &str) -> rustls::PrivateKey {
    let keyfile = File::open(filename).expect("cannot open private key file");
    let mut reader = BufReader::new(keyfile);

    loop {
        match rustls_pemfile::read_one(&mut reader).expect("cannot parse private key .pem file") {
            Some(rustls_pemfile::Item::RSAKey(key)) => return rustls::PrivateKey(key),
            Some(rustls_pemfile::Item::PKCS8Key(key)) => return rustls::PrivateKey(key),
            None => break,
            _ => {}
        }
    }

    panic!(
        "no keys found in {:?} (encrypted keys not supported)",
        filename
    );
}

fn make_client_config(ca_file: &str, certs_file: &str, key_file: &str) -> Arc<rustls::ClientConfig> {
    let cert_file = File::open(&ca_file).expect("Cannot open CA file");
    let mut reader = BufReader::new(cert_file);

    let mut root_store = RootCertStore::empty();
    root_store.add_parsable_certificates(&rustls_pemfile::certs(&mut reader).unwrap());

    let suites = rustls::DEFAULT_CIPHER_SUITES.to_vec();
    let versions = rustls::DEFAULT_VERSIONS.to_vec();

    let certs = load_certs(certs_file);
    let key = load_private_key(key_file);

    let config = rustls::ClientConfig::builder()
        .with_cipher_suites(&suites)
        .with_safe_default_kx_groups()
        .with_protocol_versions(&versions)
        .expect("inconsistent cipher-suite/versions selected")
        .with_root_certificates(root_store)
        .with_client_auth_cert(certs, key)
        .expect("invalid client auth certs/key");
    Arc::new(config)
}

fn make_server_config(ca_file: &str, certs_file: &str, key_file: &str) -> Arc<rustls::ServerConfig> {
    
    let roots = load_certs(ca_file);
    let certs = load_certs(certs_file);
    let mut client_auth_roots = RootCertStore::empty();
    for root in roots {
        client_auth_roots.add(&root).unwrap();
    }
    let client_auth = AllowAnyAuthenticatedClient::new(client_auth_roots);

    let privkey = load_private_key(key_file);
    let suites = rustls::ALL_CIPHER_SUITES.to_vec();
    let versions = rustls::ALL_VERSIONS.to_vec();

    let mut config = rustls::ServerConfig::builder()
        .with_cipher_suites(&suites)
        .with_safe_default_kx_groups()
        .with_protocol_versions(&versions)
        .expect("inconsistent cipher-suites/versions specified")
        .with_client_cert_verifier(client_auth.boxed())
        .with_single_cert_with_ocsp_and_sct(certs, privkey, vec![], vec![])
        .expect("bad certificates/private key");

    config.key_log = Arc::new(rustls::KeyLogFile::new());
    config.session_storage = rustls::server::ServerSessionMemoryCache::new(256);
    Arc::new(config)
}

pub async fn new_tls_stream(domain: &str, addr: std::net::SocketAddr, 
    ca_file: &str, cert_file: &str, key_file: &str) -> TlsClientStream<TcpStream> {
    let config = make_client_config(&ca_file, &cert_file, &key_file);

    let connector = TlsConnector::from(config);

    let stream = TcpStream::connect(&addr).await.unwrap();
    let domain = rustls::ServerName::try_from(domain).map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "invalid dnsname")).unwrap();
    let stream = connector.connect(domain, stream).await.unwrap();
    stream
}

pub fn new_tls_acceptor(ca_file: &str, cert_file: &str, key_file: &str) -> TlsAcceptor {
    let config = make_server_config(&ca_file, &cert_file, &key_file);
    let acceptor = TlsAcceptor::from(config);
    acceptor
}


pub async fn tls_server_read_to(tls_stream: &mut TlsServerStream<TcpStream>, buffer: &mut Vec<u8>, end_byte:u8) -> io::Result<usize> {
    let mut buf:[u8; 1] = [0; 1];
    loop {
        let result = tls_stream.read_exact(&mut buf).await;
        match result {
            Ok(size) => {
                if size == 0 {
                    break;
                }
                if buf[0] == end_byte {
                    return Ok(buffer.len());
                }
                buffer.push(buf[0]);
            }
            Err(e) => return Err(e),
        }
    }

    io::Result::Ok(buffer.len())
}

pub async fn tls_client_read_to(tls_stream: &mut TlsClientStream<TcpStream>, buffer: &mut Vec<u8>, end_byte:u8) -> io::Result<usize> {
    let mut buf:[u8; 1] = [0; 1];
    loop {
        let result = tls_stream.read_exact(&mut buf).await;
        match result {
            Ok(size) => {
                if size == 0 {
                    break;
                }
                
                if buf[0] == end_byte {
                    return Ok(buffer.len());
                }

                buffer.push(buf[0]);
            }
            Err(e) => return Err(e),
        }
    }

    io::Result::Ok(buffer.len())
}
