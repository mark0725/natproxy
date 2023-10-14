use tokio::sync::{mpsc, watch};
use tokio::time::{
    sleep, Duration
};
use tokio::select;

use crate::client::start_client_node;
use crate::server::start_server_node;


use crate::{
    AppOption, AppResult
};


pub struct App {
    option: AppOption,
}

const SERVER_CONNECTION_RESET_TIMEOUT:u64 = 3;
const CLIENT_CONNECTION_RESET_TIMEOUT:u64 = 5;

impl App {
    pub fn new(option: AppOption) -> App {
        Self {
            option
        }
    }

    pub async fn start(&mut self) -> AppResult<()> {
        //let (tx, mut rx) = mpsc::channel::<mpsc::Sender<String>>(32);

        if self.option.role == "server" {
            loop {
                let (main_cli_tx, main_cli_rx) = watch::channel::<String>(String::from("cmd"));

                let result = start_server_node(self.option.clone(), main_cli_rx).await;
                main_cli_tx.send(String::from("app-quit")).unwrap_or(());
                log::info!("Server node stoped.");
                log::info!("Reset server for new connection after {} seconds.", SERVER_CONNECTION_RESET_TIMEOUT);
                sleep(Duration::from_secs(SERVER_CONNECTION_RESET_TIMEOUT)).await;
            }

        } else {
            loop {
                let result = start_client_node(self.option.clone()).await?;
                log::info!("Client node stoped.");
                log::info!("Create new connection after {} seconds", CLIENT_CONNECTION_RESET_TIMEOUT);
                sleep(Duration::from_secs(CLIENT_CONNECTION_RESET_TIMEOUT)).await;
            }
        }

    }
}
