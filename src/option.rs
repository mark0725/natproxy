use std::{
    fs::File,
    io::{self, BufReader, Read},
    net::{IpAddr, SocketAddr},
    sync::Arc,
    env,
};

use commander::Commander;
use rustls::{Certificate, PrivateKey};

use serde::{Deserialize, Serialize};
use tokio_rustls::{rustls, TlsAcceptor};

use crate::{MappingConfig, AppError, AppResult, mappings};


pub struct Builder {
    inner: AppResult<AppOption>,
}
impl Builder {
    #[inline]
    pub fn new() -> Builder {
        Builder {
            inner: Ok(AppOption::default()),
        }
    }

    pub fn role(self, role: String) -> Builder {
        self.and_then(|mut option| {
            option.role = role;
            Ok(option)
        })
    }


    pub fn listen_addr(self, addr: SocketAddr) -> Builder {
        self.and_then(|mut option| {
            option.listen = addr;
            Ok(option)
        })
    }


    pub fn server(self, addr: Option<SocketAddr>) -> Builder {
        self.and_then(|mut option| {
            option.server = addr;
            Ok(option)
        })
    }

    pub fn ca_cert(self, ca_cert: Option<String>) -> Builder {
        self.and_then(|mut option| {
            option.ca_cert = ca_cert;
            Ok(option)
        })
    }
   
    pub fn cert(self, cert: Option<String>) -> Builder {
        self.and_then(|mut option| {
            option.cert = cert;
            Ok(option)
        })
    }

    pub fn key(self, key: Option<String>) -> Builder {
        self.and_then(|mut option| {
            option.key = key;
            Ok(option)
        })
    }

    pub fn log_level(self, log_level: Option<String>) -> Builder {
        self.and_then(|mut option| {
            option.log_level = log_level.unwrap_or(String::from("info"));
            Ok(option)
        })
    }

    pub fn password(self, password: Option<String>) -> Builder {
        self.and_then(|mut option| {
            option.proxy_pass = password;
            Ok(option)
        })
    }

    pub fn mappings(self, mappings: String) -> Builder {
        self.and_then(|mut option| {
            option.mappings = serde_json::from_str(&mappings).unwrap();
            Ok(option)
        })
    }


    fn and_then<F>(self, func: F) -> Self
    where
        F: FnOnce(AppOption) -> AppResult<AppOption>,
    {
        Builder {
            inner: self.inner.and_then(func),
        }
    }
}

fn default_proxy_on() -> Vec<String> {
    vec![String::from("tcp")]
}

fn default_listen_addr() -> SocketAddr {
    "0.0.0.0:8001".parse().unwrap()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppOption {
    #[serde(default)]
    pub role: String,

    #[serde(default = "default_listen_addr")]
    pub listen: SocketAddr,

    pub server: Option<SocketAddr>,

    /// ca证书文件
    pub ca_cert: Option<String>,
    /// 公开的证书公钥文件
    pub cert: Option<String>,
    /// 隐私的证书私钥文件
    pub key: Option<String>,

    #[serde(default)]
    pub log_level: String,

    #[serde(default = "default_proxy_on")]
    pub proxy_on: Vec<String>,

    pub proxy_pass: Option<String>,

    #[serde(default)]
    pub mappings: Vec<MappingConfig>,
   
}

impl Default for AppOption {
    fn default() -> Self {
        Self {
            role: "server".to_string(),
            listen: default_listen_addr(),
            server: None,
        
            ca_cert: None,
            cert: None,
            key: None,

            log_level: "info".to_string(),

            proxy_on: default_proxy_on(),
            proxy_pass: None,
            
            mappings: vec![],
            
        }
    }
}

const NATPROXY_ENV_PREFIX: &str = "NATPROXY_";

impl AppOption {
    pub fn builder() -> Builder {
        Builder::new()
    }

    pub fn parse_env() -> AppResult<AppOption> {
        let command = Commander::new()
            .version(&env!("CARGO_PKG_VERSION").to_string())
            .usage("--listen 0.0.0.0:8001")
            .usage_desc("natproxy -R server --listen 0.0.0.0:8001")
            .option_list(
                "--proxy_on [value]",
                "proxy enables: http https socks5 tcp httpreverse",
                None,
            )
            .option_str(
                "-R, --role value",
                "client, server, all: client and server",
                None,
            )
            .option_str("-c, --config", "config file path", None)
            // .option("--proxy value", "是否只接收来自代理的连接", Some(false))
            .option_str("--ca value", "The trusted CA certificate file in PEM format used to verify the cert", None)
            .option_str("--cert value", "Certificate used for mTLS between server/client nodes.", None)
            .option_str("--key value", "Certificate key", None)
            .option_str(
                "-L, --listen value",
                "server listen address",
                Some("0.0.0.0:8001".to_string()),
            )
            .option_str("-S, --server value", "server address: 127.0.0.1:8001", None)
            .option_str("--pass value", "proxy password", None)
            .option_str("--log value", "log level", None)
            .option_str("--mappings value", "proxy mappings", None)
            .parse_env_or_exit();

        if let Some(config) = command.get_str("c") {
            let mut file = File::open(config)?;
            let mut contents = String::new();
            file.read_to_string(&mut contents)?;
            let option = serde_yaml::from_str::<AppOption>(&contents).unwrap();
            log::debug!("options = {:?}", option);
            return Ok(option);
        }

        let mut builder = Self::builder();
        for (k,v) in env::vars() {
            if k.starts_with(NATPROXY_ENV_PREFIX) {
                log::trace!("env key:{}  value:{}", k, v);
                match &k[NATPROXY_ENV_PREFIX.len()..] {
                    "ROLE" => {
                        builder = builder.role(v);
                    }
                    "LISTEN" => {
                        builder = builder.listen_addr(v.parse::<SocketAddr>().unwrap());
                    }
                    "SERVER" => {
                        builder = builder.server(v.parse().ok());
                    }
                    "CA_CERT" => {
                        builder = builder.ca_cert(Some(v));
                    }
                    "CERT" => {
                        builder = builder.cert(Some(v));
                    }
                    "KEY" => {
                        builder = builder.key(Some(v));
                    }
                    "LOG_LEVEL" => {
                        builder = builder.log_level(Some(v));
                    }
                    "PASS" => {
                        builder = builder.password(Some(v));
                    }
                    "MAPPINGS" => {
                        builder = builder.mappings(v);
                    }
                    _ => {}
                }
            }
        
        }

        let listen_host = command.get_str("listen");
        match listen_host {
            Some(val) => builder = builder.listen_addr(val.parse::<SocketAddr>().unwrap()),
            None => {}
        }

        let role = command.get_str("role");
        match role {
            Some(val) => builder = builder.role(val),
            None => {}
        }

        let pass = command.get_str("pass");
        match pass {
            Some(val) => builder = builder.password(Some(val)),
            None => {}
        }


        let ca = command.get_str("ca");
        match ca {
            Some(val) => builder = builder.ca_cert(Some(val)),
            None => {}
        }

        let cert = command.get_str("cert");
        match cert {
            Some(val) => builder = builder.cert(Some(val)),
            None => {}
        }

        let key = command.get_str("key");
        match key {
            Some(val) => builder = builder.key(Some(val)),
            None => {}
        }

        let log = command.get_str("log");
        match log {
            Some(val) => builder = builder.log_level(Some(val)),
            None => {}
        }

        let server = command.get_str("S");
        match server {
            Some(val) => builder = builder.server(val.parse::<SocketAddr>().ok()),
            None => {}
        }

        let mappings = command.get_str("mappings");
        match mappings {
            Some(val) => builder = builder.mappings(val),
            None => {}
        }
      
        builder.inner
    }
}