//! Offline interoperability check: length-prefixed protobuf blocks in/out.
use std::io::{self, Read, Write};
use buffa::Message;
use substreams_database_change::pb::sf::substreams::sink::database::v1::DatabaseChanges;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut input = io::stdin().lock();
    let mut output = io::stdout().lock();
    loop {
        let mut length = [0; 4];
        match input.read_exact(&mut length) {
            Ok(()) => {},
            Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => break,
            Err(error) => return Err(error.into()),
        }
        let size = u32::from_le_bytes(length) as usize;
        if size > 64 * 1024 * 1024 { return Err("oversized input".into()); }
        let mut bytes = vec![0; size];
        input.read_exact(&mut bytes)?;
        let changes = DatabaseChanges::decode_from_slice(bytes.as_slice())?;
        let encoded = svm_dex::compress_database_changes(changes)?.encode_to_vec();
        output.write_all(&(encoded.len() as u32).to_le_bytes())?;
        output.write_all(&encoded)?;
    }
    Ok(())
}
