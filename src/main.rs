use natproxy::{AppOption, AppResult, App};
use std::env;
use dotenvy;
use log::LevelFilter;
use lite_log::LiteLogger;

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

    let logger = LiteLogger::new()
        .with_level(LevelFilter::Off)
        .with_local_timestamps()
        .with_module_level("natproxy", log_level)
        .init();

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