pub mod event;
pub mod identity;
pub mod proto;
pub mod redaction;
pub mod repo;
pub mod status;

#[cfg(test)]
extern crate self as lifecycle;

#[cfg(test)]
mod tests {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/unit/proto_contract.rs"
    ));

    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/unit/redaction.rs"
    ));

    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/unit/domain.rs"));
}
