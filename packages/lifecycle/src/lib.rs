pub mod proto;

#[cfg(test)]
extern crate self as lifecycle;

#[cfg(test)]
mod tests {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/unit/proto_contract.rs"
    ));
}
