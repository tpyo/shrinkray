use serde::Deserialize;
use std::env;
use std::fs::File;
use std::net::SocketAddr;

#[derive(Deserialize, Clone, Debug)]
pub struct S3Config {
    pub access_key_id: String,
    pub secret_access_key: String,
    pub region: String,
    pub endpoint: Option<String>,
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
    pub max_megapixels: Option<f64>,
    pub max_output_resolution: Option<u32>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ConfigRouting {
    pub path: String,
    pub endpoint: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server_address: "0.0.0.0:9000".parse().unwrap(),
            management_address: "0.0.0.0:9001".parse().unwrap(),
            read_timeout: 30,
            routing: Vec::new(),
            proxies: Vec::new(),
            s3: None,
            signing_secret: None,
            otel_collector_endpoint: None,
            max_megapixels: None,
            max_output_resolution: None,
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_config() {
        let file = File::open("config/config.json").expect("failed to open config file");
        let config: Config = serde_json::from_reader(file).expect("failed to parse config");

        assert_eq!(config.server_address.to_string(), "0.0.0.0:9000");
        assert_eq!(config.management_address.to_string(), "0.0.0.0:9001");
        assert_eq!(config.read_timeout, 5);
        assert_eq!(
            config.otel_collector_endpoint,
            Some("http://alloy:4317".to_string())
        );
        assert_eq!(config.signing_secret, None);
        assert_eq!(config.max_megapixels, None);
        assert_eq!(config.max_output_resolution, None);

        // Test S3 config
        let s3_config = config.s3.expect("S3 config should be present");
        assert_eq!(s3_config.access_key_id, "");
        assert_eq!(s3_config.secret_access_key, "");
        assert_eq!(s3_config.region, "us-east-1");
        assert_eq!(s3_config.endpoint, None);

        // Test routing
        assert_eq!(config.routing.len(), 3);
        assert_eq!(config.routing[0].path, "samples/{*path}");
        assert_eq!(
            config.routing[0].endpoint,
            "https://shrinkray.photo/samples/"
        );
        assert_eq!(config.routing[1].path, "files/{*path}");
        assert_eq!(config.routing[1].endpoint, "file:///app/files/");
        assert_eq!(config.routing[2].path, "{*path}");
        assert_eq!(config.routing[2].endpoint, "s3://bucket-name/");

        // Test proxies
        assert_eq!(config.proxies.len(), 2);
    }
}
