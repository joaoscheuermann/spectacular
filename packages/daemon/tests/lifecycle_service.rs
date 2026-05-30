mod integration {
    mod lifecycle_service {
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/integration/lifecycle_service.rs"
        ));
    }
}
