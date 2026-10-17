//! Generated from the canonical `proto/v1/spl/spl-transfers.proto` schema with
//! the pinned Buffa plugin. The independent wrapper does not link the shared
//! workspace's older Prost messages. Regenerate with `make protogen`.

pub mod solana {
    pub mod spl {
        pub mod transfers {
            #[allow(clippy::derivable_impls)]
            pub mod v1 {
                include!("solana.spl.transfers.v1.mod.rs");
            }
        }
    }
}
