pub mod doric {
    pub mod lifecycle {
        pub mod v1 {
            include!(concat!(env!("OUT_DIR"), "/doric.lifecycle.v1.rs"));
        }
    }
}
