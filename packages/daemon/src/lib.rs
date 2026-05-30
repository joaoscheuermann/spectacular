pub mod error;
pub mod event;
pub mod registry;
pub mod root;
pub mod server;
pub mod service;

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
}
