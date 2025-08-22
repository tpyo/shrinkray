use aws_sign_v4::AwsSign;
use reqwest::{Client, Response, header::HeaderMap};
use std::path::{Path, PathBuf};
use url::Url;

use crate::config::Config;
use crate::error::{Error, Result};

impl From<tokio::io::Error> for Error {
    fn from(err: tokio::io::Error) -> Self {
        match err.kind() {
            std::io::ErrorKind::NotFound => Error::NotFound,
            _ => Error::Io(format!("{:?} : {:?}", &err.kind(), &err)),
        }
    }
}

async fn get_file_from_file(path: &str) -> Result<Vec<u8>> {
    let full_path: PathBuf = Path::new(&path).canonicalize()?;
    Ok(tokio::fs::read(&full_path).await?)
}

async fn get_file_from_http(url: &str, config: &Config) -> Result<Vec<u8>> {
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(config.read_timeout))
        .build()?;
    let response = send_request(&client, url, HeaderMap::new()).await?;

    if response.status() == reqwest::StatusCode::OK {
        return Ok(response.bytes().await?.to_vec());
    }

    match response.status() {
        reqwest::StatusCode::NOT_FOUND => Err(Error::NotFound),
        reqwest::StatusCode::FORBIDDEN => Err(Error::NotFound),
        code => Err(Error::Io(format!(
            "unexpected response from HTTP backend: {}",
            code
        ))),
    }
}

async fn get_file_from_s3(bucket: &str, path: &str, config: &Config) -> Result<Vec<u8>> {
    if config.s3.is_none() {
        return Err(Error::InvalidBackend);
    }
    if let Some(s3config) = &config.s3 {
        let url = format!(
            "http://{}.s3.{}.amazonaws.com{}",
            &bucket, &s3config.region, path
        );
        let datetime = chrono::Utc::now();
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(config.read_timeout))
            .build()?;
        let response = send_request(
            &client,
            &url,
            generate_sigv4_headers(&datetime, &url, config),
        )
        .await?;

        if response.status() == reqwest::StatusCode::OK {
            return Ok(response.bytes().await?.to_vec());
        }

        return match response.status() {
            reqwest::StatusCode::NOT_FOUND => Err(Error::NotFound),
            reqwest::StatusCode::FORBIDDEN => Err(Error::NotFound),
            code => Err(Error::Io(format!(
                "unexpected response from S3 backend: {}",
                code
            ))),
        };
    }

    Err(Error::InvalidBackend)
}

#[tracing::instrument(skip_all, fields(shrinkray.file = url))]
pub async fn get_file_from_backend(url: &str, config: &Config) -> Result<Vec<u8>> {
    let url = Url::parse(url)?;
    match url.scheme() {
        "file" => get_file_from_file(url.path()).await,
        "http" | "https" => get_file_from_http(url.as_str(), config).await,
        "s3" => get_file_from_s3(url.host_str().unwrap(), url.path(), config).await,
        _ => Err(Error::InvalidBackend),
    }
}

async fn send_request(client: &Client, url: &str, headers: HeaderMap) -> Result<Response> {
    client
        .get(url)
        .headers(headers)
        .body("")
        .send()
        .await
        .map_err(Error::Http)
}

fn generate_sigv4_headers(
    datetime: &chrono::DateTime<chrono::Utc>,
    url: &str,
    config: &Config,
) -> HeaderMap {
    let mut headers = HeaderMap::new();

    // Extract host from URL
    let host = url.split('/').nth(2).unwrap_or_default();

    headers.insert("host", host.parse().unwrap());
    headers.insert(
        "x-amz-date",
        datetime
            .format("%Y%m%dT%H%M%SZ")
            .to_string()
            .parse()
            .unwrap(),
    );
    headers.insert(
        "x-amz-content-sha256",
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
            .parse()
            .unwrap(),
    );

    let signature = generate_sigv4_signature("GET", url, datetime, &headers, config);

    headers.insert(reqwest::header::AUTHORIZATION, signature.parse().unwrap());

    headers
}

fn generate_sigv4_signature<'a>(
    method: &'a str,
    url: &'a str,
    datetime: &'a chrono::DateTime<chrono::Utc>,
    headers: &'a HeaderMap,
    config: &'a Config,
) -> String {
    if let Some(s3config) = &config.s3 {
        return AwsSign::new(
            method,
            url,
            datetime,
            headers,
            &s3config.region,
            &s3config.access_key_id,
            &s3config.secret_access_key,
            "s3",
            "",
        )
        .sign();
    }
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config;
    use chrono::TimeZone;
    use reqwest::header::HeaderValue;

    // Mock configuration for testing
    fn mock_config() -> config::Config {
        config::Config {
            otel_collector_endpoint: None,
            server_address: "127.0.0.1:9000".parse().unwrap(),
            management_address: "127.0.0.1:9001".parse().unwrap(),
            read_timeout: 10,
            routing: vec![],
            proxies: vec![],
            signing_secret: Some("super_secret_key".to_string()),
            s3: Some(config::S3Config {
                access_key_id: "test-access-key".to_string(),
                secret_access_key: "test-secret-key".to_string(),
                region: "test-region".to_string(),
            }),
        }
    }

    #[test]
    fn test_generate_signature() {
        let datetime = chrono::Utc::with_ymd_and_hms(&chrono::Utc, 2024, 2, 20, 12, 0, 0).unwrap();
        let url = "http://test-bucket.s3.test-region.amazonaws.com/test/file.txt";
        let config = mock_config();
        let headers = generate_sigv4_headers(&datetime, url, &config);
        let sig = generate_sigv4_signature("GET", url, &datetime, &headers, &config);
        let expected = "AWS4-HMAC-SHA256 Credential=test-access-key/20240220/test-region/s3/aws4_request,SignedHeaders=authorization;host;x-amz-content-sha256;x-amz-date,Signature=f2fd6ad1970f41610dabb7a31fe53c4c7fafc44c14166ac3f3de2e2af91875b5";
        assert_eq!(sig, expected);
    }

    #[test]
    fn test_generate_headers() {
        // Fixed datetime for testing
        let datetime = chrono::Utc::with_ymd_and_hms(&chrono::Utc, 2024, 2, 20, 12, 0, 0).unwrap();
        let url = "http://test-bucket.s3.test-region.amazonaws.com/test/file.txt";
        let config = mock_config();

        let headers = generate_sigv4_headers(&datetime, url, &config);

        assert_eq!(
            headers.get("host").unwrap(),
            &HeaderValue::from_static("test-bucket.s3.test-region.amazonaws.com")
        );
        assert_eq!(
            headers.get("x-amz-date").unwrap(),
            &HeaderValue::from_static("20240220T120000Z")
        );
    }
}
