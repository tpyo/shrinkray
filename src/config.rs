use serde::Deserialize;
use std::env;
use std::fs::File;
use std::net::SocketAddr;

#[derive(Deserialize, Clone, Debug)]
pub struct S3Config {
    pub access_key_id: String,
    pub secret_access_key: String,
    pub region: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Config {
    pub server_address: SocketAddr,
    pub management_address: SocketAddr,
    pub read_timeout: u64,
    pub routing: Vec<ConfigRouting>,
    pub proxies: Vec<ipnet::IpNet>,
    pub s3: Option<S3Config>,
    pub signing_secret: Option<String>,
    pub otel_collector_endpoint: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ConfigRouting {
    pub path: String,
    pub endpoint: String,
}

pub fn read_config() -> Result<Config, Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    let file = if args.len() > 1 {
        args[1].clone()
    } else {
        "config/config.json".to_string()
    };

    Ok(serde_json::from_reader(File::open(file)?)?)
}
