use natproxy::{AppOption, AppResult, App};
use std::env;
use dotenvy;
use log::LevelFilter;
use simple_logger::SimpleLogger;
use natproxy::Logger;

async fn run_main() -> AppResult<()> {
    let option = AppOption::parse_env()?;  

    let log_level = match option.log_level.as_str() {
        "trace" => LevelFilter::Trace,
        "debug" => LevelFilter::Debug,
        "info" => LevelFilter::Info,
        "warn" => LevelFilter::Warn,
        "error" => LevelFilter::Error,
        _ => LevelFilter::Info
    };

    let logger = SimpleLogger::new()
        .with_level(LevelFilter::Off)
        //.with_local_timestamps()
        .with_module_level("natproxy", log_level)
        .init();
    //log::set_boxed_logger(Box::new(Logger {})).unwrap();

    let mut app = App::new(option);
    app.start().await?;

    Ok(())
}

// #[forever_rs::main]
#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    log::set_max_level(LevelFilter::Info);

    if let Err(e) = run_main().await {
        log::error!("runtime error:{:?}", e);
    }
}