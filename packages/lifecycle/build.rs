fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=proto/doric/lifecycle/v1.proto");
    println!("cargo:rerun-if-env-changed=PROTOC");

    if std::env::var_os("PROTOC").is_none() {
        let protoc = protoc_bin_vendored::protoc_bin_path()?;
        std::env::set_var("PROTOC", protoc);
    }

    tonic_prost_build::configure()
        .compile_protos(&["proto/doric/lifecycle/v1.proto"], &["proto"])?;

    Ok(())
}
