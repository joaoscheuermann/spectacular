fn main() {
    if let Err(error) =
        daemon::server::build_production_service(daemon::server::ServerConfig::default())
    {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
