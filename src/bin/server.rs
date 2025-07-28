use clap::{Arg, Command};
use doomsday_rs::config::Config;
use doomsday_rs::server::DoomsdayServer;
use tracing_subscriber;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    
    let matches = Command::new("doomsday-server")
        .version(doomsday_rs::version::VERSION)
        .about("Doomsday certificate monitoring server")
        .arg(
            Arg::new("config")
                .short('c')
                .long("config")
                .value_name("FILE")
                .help("Configuration file path")
                .default_value("ddayconfig.yml")
        )
        .get_matches();
    
    let config_path = matches.get_one::<String>("config").unwrap();
    
    let config = if std::path::Path::new(config_path).exists() {
        Config::from_file(config_path)?
    } else {
        tracing::warn!("Config file {} not found, using default configuration", config_path);
        Config::default()
    };
    
    config.validate()?;
    
    let server = DoomsdayServer::new(config).await?;
    server.serve().await?;
    
    Ok(())
}