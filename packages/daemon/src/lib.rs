pub mod error;
pub mod event;
pub mod process;
pub mod registry;
pub mod root;
pub mod server;
pub mod service;
pub mod worker_session;

#[cfg(test)]
mod tests {
    mod root {
        include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/unit/root.rs"));
    }

    mod registry {
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/unit/registry.rs"
        ));
    }

    mod service {
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/unit/service.rs"
        ));
    }

    mod server {
        include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/unit/server.rs"));
    }

    mod process {
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/unit/process.rs"
        ));
    }

    mod worker_session {
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/unit/worker_session.rs"
        ));
    }
}
