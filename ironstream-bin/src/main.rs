use ironstream_core::Server;
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Read configuration from environment variables
    let admin_token = env::var("IRONSTREAM_ADMIN_TOKEN")
        .expect("IRONSTREAM_ADMIN_TOKEN environment variable must be set");

    let api_endpoint = env::var("IRONSTREAM_API_ENDPOINT")
        .expect("IRONSTREAM_API_ENDPOINT environment variable must be set");

    let rate_limit_count = env::var("IRONSTREAM_RATE_LIMIT_COUNT")
        .unwrap_or_else(|_| "100".to_string())
        .parse::<u32>()
        .expect("IRONSTREAM_RATE_LIMIT_COUNT must be a valid number");

    let rate_limit_seconds = env::var("IRONSTREAM_RATE_LIMIT_SECONDS")
        .unwrap_or_else(|_| "60".to_string())
        .parse::<u64>()
        .expect("IRONSTREAM_RATE_LIMIT_SECONDS must be a valid number");

    let port: u16 = env::var("IRONSTREAM_PORT")
        .ok()
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(3113);

    let server = Server::new(
        admin_token.clone(),
        api_endpoint.clone(),
        rate_limit_count,
        rate_limit_seconds,
        Some(port),
    );

    println!("Starting server...");

    println!(
        "Admin Token: {}
API Endpoint: {}
Port: {}",
        admin_token, api_endpoint, port
    );

    server.run().await?;

    Ok(())
}
