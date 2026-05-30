pub mod error;
pub mod event;
pub mod registry;
pub mod root;

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
}
