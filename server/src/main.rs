//! Starts the baccarat-ev REST API.

use std::path::Path;
use std::process::ExitCode;

use baccarat_ev::{DEFAULT_CONFIG, Payouts, load_payouts};
use baccarat_server::{AppState, app};

const USAGE: &str = "\
REST API for baccarat-ev: exact odds of every bet as JSON.

Usage: baccarat-server [--addr HOST:PORT] [--config FILE]

  --addr HOST:PORT  address to listen on (default: 127.0.0.1:3000)
  --config FILE     payouts for requests that don't set their own
                    (default: baccarat.toml in this folder if it exists, else built-in)

Endpoints: GET /health, GET /openapi.yaml, POST /analyze";

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("error: {message}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), String> {
    let mut addr = "127.0.0.1:3000".to_string();
    let mut config = None;
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--addr" => addr = args.next().ok_or("--addr needs HOST:PORT")?,
            "--config" => config = Some(args.next().ok_or("--config needs a file")?),
            "-h" | "--help" => {
                println!("{USAGE}");
                return Ok(());
            }
            _ => return Err(format!("unknown argument {flag:?}\n\n{USAGE}")),
        }
    }

    let config = config.or_else(|| Path::new(DEFAULT_CONFIG).exists().then(|| DEFAULT_CONFIG.to_string()));
    let payouts = match &config {
        Some(path) => load_payouts(Path::new(path))?,
        None => Payouts::default(),
    };
    let listener = tokio::net::TcpListener::bind(&addr).await.map_err(|error| format!("can't listen on {addr}: {error}"))?;
    let address = listener.local_addr().map_err(|error| error.to_string())?;
    println!(
        "Listening on http://{address} (payouts: {}). Press Ctrl+C to stop.",
        config.as_deref().unwrap_or("built-in defaults")
    );
    axum::serve(listener, app(AppState::new(payouts)))
        .with_graceful_shutdown(async {
            // If Ctrl+C can't be watched, keep serving until the process is killed.
            if tokio::signal::ctrl_c().await.is_err() {
                std::future::pending::<()>().await;
            }
        })
        .await
        .map_err(|error| format!("server stopped: {error}"))
}
