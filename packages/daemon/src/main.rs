#[tokio::main]
async fn main() {
    if let Err(error) =
        daemon::server::serve_production(daemon::server::ServerConfig::default()).await
    {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
