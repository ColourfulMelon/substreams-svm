//! The exact pre-Buffa output for all pinned blocks and coverage combinations.
use buffa::Message;
use flate2::read::GzDecoder;
use sha2::{Digest, Sha256};
use spl_transfers_substreams::map_events_impl;
use std::io::Read;
use substreams_solana::pb::sf::solana::r#type::v1::Block;

const FIXTURES: [&[u8]; 4] = [
    include_bytes!("../../transfers/tests/fixtures/solana-mainnet-349019252.pb.gz"),
    include_bytes!("../../transfers/tests/fixtures/solana-mainnet-350000009.pb.gz"),
    include_bytes!("../../transfers/tests/fixtures/solana-mainnet-350000010.pb.gz"),
    include_bytes!("../../transfers/tests/fixtures/solana-mainnet-350000026.pb.gz"),
];
const COVERAGE: [&str; 4] = ["", "all", "inner_instructions", "token_2022,mint_burn,unchecked_transfers"];
// Recorded from the original PR225 projector/wrapper at 3de4050, before the
// Buffa migration. One row per pinned block, in COVERAGE order.
const PRE_BUFFA_SHA256: [[&str; 4]; 4] = [
    [
        "657333056c64daa2b0e715bf19b7b6d146e68d1361b1a4a8c5bfc2f0de0ee2b2",
        "cbd47268658134f3b80e64a44a4e93786fbc12707917ee895d9fdf1bb31b6849",
        "0892364c747994ee188bdd9478deb614ebdbea3bd3342c5d34760a1699bdfbd3",
        "ac283ce2b3c3b7906a51762deb60599a4e2d99cd99abcf516c535202ec0852c5",
    ],
    [
        "42c9c41d5bc9fef20fc800a072563d4044f22f55c2b9052d936feb09f97cca3a",
        "360e2d5d66c74159b08c5a00cce1ad47a48296b39428c049a5aa3b7ae90cd7fd",
        "855576d03060acc3356ced13ce882f475189a30b9199e3e25e9033105fd29e2d",
        "aea108e895f5ea2daf38c961a59bd23297ecbce09f1077525840e260c2180e0d",
    ],
    [
        "966a13f505022082925467f0c8107dfb7ab9569f3acc03e4e57e11ba789c766d",
        "230150ad06bf9eaca3dba6dce56230b285323643d4c7239ff7d1eff685b2bde9",
        "89e8dca0c02ce8732f7abc39d868a66ce580f215d7a54fb908241d0d97e8d10a",
        "1b96491b2f5368b5ca581bfd245884d3679afeecf28337bd41f9a69e450c3d15",
    ],
    [
        "11352706e35f11d44e2a5a7260b5ec342500017d61e6fcdae465fa54ca9181d8",
        "ea2a10f78f3b42e32f7c81a623d26a733ddcddcfd781a013d2b538d8db20f6ba",
        "aa6589985b5c40e3c9cc4d4af2e7a4d81944d436b520d2a5359d4df01c93ac14",
        "1e87f091a8c2aa105e91270d513c3f64fb66ac77a14f8a90a8c0ba2913a9728d",
    ],
];

#[test]
fn exact_wire_output_matches_pre_buffa() {
    for (fixture, expected) in FIXTURES.into_iter().zip(PRE_BUFFA_SHA256) {
        let mut bytes = Vec::new();
        GzDecoder::new(fixture).read_to_end(&mut bytes).unwrap();
        let block = Block::decode_from_slice(bytes.as_slice()).unwrap();
        for (params, expected) in COVERAGE.into_iter().zip(expected) {
            let output = map_events_impl(params, &block).unwrap().encode_to_vec();
            assert_eq!(format!("{:x}", Sha256::digest(output)), expected, "slot {} / {params:?}", block.slot);
        }
    }
}
