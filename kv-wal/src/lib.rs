//! WAL [ checksum: 4 bytes ][ length: 4 bytes ][ payload: `length` bytes ]
mod error;
mod wal;

pub use error::{Error, Result};
pub use wal::Wal;

use crc32fast::Hasher;

const MAX_RECORD_SIZE: u32 = 64 << 20;
const HEADER_SIZE: u32 = 8;

pub fn encode(payload: &[u8], buffer: &mut Vec<u8>) -> Result<()> {
    let len: u32 = payload.len() as u32;

    if len > MAX_RECORD_SIZE {
        return Err(Error::RecordTooLarge("overloading".to_string()));
    }

    let offset = buffer.len();

    buffer.extend_from_slice(&[0u8; 8]);

    buffer.extend_from_slice(payload);

    let len = payload.len() as u32;

    buffer[(offset + 4)..(offset + 8)].copy_from_slice(&len.to_le_bytes());

    let mut hasher = Hasher::new();

    hasher.update(&buffer[(offset + 4)..]);

    let crc = hasher.finalize();

    buffer[offset..(offset + 4)].copy_from_slice(&crc.to_le_bytes());

    Ok(())
}

pub fn decode(buffer: &[u8]) -> Result<Option<(&[u8], u32)>> {
    if (buffer.len() as u32) < HEADER_SIZE {
        return Ok(None);
    }

    let payload_buffer_len: u32 =
        u32::from_le_bytes(buffer[4..8].try_into().expect("that cann't never fail"));
    if payload_buffer_len > MAX_RECORD_SIZE {
        return Err(Error::Corruption(
            "Expected an error for a length over MAX_RECORD_SIZE".to_string(),
        ));
    }

    if payload_buffer_len > (buffer.len() as u32 - HEADER_SIZE) {
        return Ok(None);
    }

    let mut hasher = Hasher::new();
    hasher.update(&buffer[4..(payload_buffer_len + 8) as usize]);
    let crc = hasher.finalize();

    if crc != u32::from_le_bytes(buffer[0..4].try_into().expect("that cann't never fail")) {
        return Err(Error::Corruption("CRC not mached".to_string()));
    }

    Ok(Some((
        &buffer[8..(payload_buffer_len + 8) as usize],
        payload_buffer_len + HEADER_SIZE,
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_the_same_payload() {
        let payload = b"test me";
        let mut buffer: Vec<u8> = Vec::new();
        let bytes_used = payload.len() as u32 + HEADER_SIZE;

        encode(payload, &mut buffer).unwrap();

        let ret = decode(buffer.as_slice()).unwrap().unwrap();

        assert_eq!(ret.0, payload);
        assert_eq!(ret.1, bytes_used);
        assert_eq!(bytes_used, buffer.len() as u32);
    }

    #[test]
    fn empty_payload() {
        let payload = b"";
        let mut buffer: Vec<u8> = Vec::new();
        let bytes_used = payload.len() as u32 + HEADER_SIZE;

        encode(payload, &mut buffer).unwrap();

        let ret = decode(buffer.as_slice()).unwrap().unwrap();

        assert_eq!(ret.0, payload);
        assert_eq!(ret.1, bytes_used);
        assert_eq!(bytes_used, buffer.len() as u32);
    }

    #[test]
    fn two_records_back_to_back() {
        let payload1 = b"first record";
        let payload2 = b"second record";

        let mut buffer = Vec::new();

        encode(payload1, &mut buffer).unwrap();
        encode(payload2, &mut buffer).unwrap();

        let (decoded1, bytes_used1) = decode(&buffer).unwrap().unwrap();

        assert_eq!(decoded1, payload1);

        let (decoded2, bytes_used2) = decode(&buffer[bytes_used1 as usize..]).unwrap().unwrap();

        assert_eq!(decoded2, payload2);

        assert_eq!(bytes_used1 + bytes_used2, buffer.len() as u32);
    }

    #[test]
    fn every_cutoff_returns_none() {
        let payload = b"test payload";
        let mut buffer = Vec::new();

        encode(payload, &mut buffer).unwrap();

        // Every incomplete record must return Ok(None).
        for i in 0..buffer.len() {
            let result = decode(&buffer[..i]).unwrap();

            assert!(result.is_none(), "Expected None for cutoff at byte {}", i);
        }
    }

    #[test]
    fn every_damaged_byte_is_detected() {
        let payload = b"test payload";
        let mut buffer = Vec::new();

        encode(payload, &mut buffer).unwrap();

        for i in 0..buffer.len() {
            let mut corrupted = buffer.clone();

            corrupted[i] ^= 0x01;

            let result = decode(&corrupted);

            assert!(
                !matches!(result, Ok(Some(_))),
                "Damaged record was accepted at byte {}",
                i
            );
        }
    }

    #[test]
    fn length_over_maximum_returns_error() {
        let mut buffer = Vec::new();

        buffer.extend_from_slice(&[0u8; 4]);
        buffer.extend_from_slice(&(MAX_RECORD_SIZE + 1).to_le_bytes());

        let result = decode(&buffer);

        assert!(
            result.is_err(),
            "Expected an error for a length over MAX_RECORD_SIZE"
        );
    }

    #[test]
    fn payload_over_maximum_cannot_be_encoded() {
        let payload = vec![0u8; (MAX_RECORD_SIZE + 1) as usize];
        let mut buffer = Vec::new();

        let result = encode(&payload, &mut buffer);

        assert!(result.is_err());
        assert!(buffer.is_empty());
    }
}
