use crate::{Error, Result};
use bytes::Bytes;

#[derive(Debug, PartialEq, Clone)]
pub enum Command {
    Put { key: Bytes, value: Bytes },
    Delete { key: Bytes },
}

impl Command {
    pub fn encode(&self, out: &mut Vec<u8>) -> Result<()> {
        match self {
            Command::Put { key, value } => {
                out.push(1u8);

                out.extend_from_slice(&u32::to_le_bytes(key.len() as u32));

                out.extend_from_slice(key);

                out.extend_from_slice(value);
            }
            Command::Delete { key } => {
                out.push(2u8);

                out.extend_from_slice(&u32::to_le_bytes(key.len() as u32));

                out.extend_from_slice(key);
            }
        }

        Ok(())
    }

    pub fn decode(payload: &[u8]) -> Result<Command> {
        if payload.is_empty() {
            return Err(Error::Corruption("payload is empty".to_string()));
        }

        let tag = u8::from_le_bytes(payload[0..1].try_into().unwrap());

        if payload.len() < 5 {
            return Err(Error::Corruption("payload len in < 5".to_string()));
        }

        match tag {
            1 => {
                let key_size = (u32::from_le_bytes(payload[1..5].try_into().unwrap())) as usize;

                if key_size + 5 > payload.len() {
                    return Err(Error::Corruption(
                        "key size larger than the payload".to_string(),
                    ));
                }
                let key = &payload[5..(key_size + 5)];

                let value = &payload[(key_size + 5)..];

                Ok(Command::Put {
                    key: Bytes::copy_from_slice(key),
                    value: Bytes::copy_from_slice(value),
                })
            }
            2 => {
                let key_size = (u32::from_le_bytes(payload[1..5].try_into().unwrap())) as usize;

                if key_size + 5 != payload.len() {
                    return Err(Error::Corruption(
                        "key size larger that the payload".to_string(),
                    ));
                }
                let key = &payload[5..(key_size + 5)];

                Ok(Command::Delete {
                    key: Bytes::copy_from_slice(key),
                })
            }
            x => Err(Error::Corruption(format!("invalid tag : {x}"))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bytes(value: &'static [u8]) -> Bytes {
        Bytes::from_static(value)
    }

    fn round_trip(command: Command) {
        let mut encoded = Vec::new();
        command.encode(&mut encoded).unwrap();

        let decoded = Command::decode(&encoded).unwrap();

        assert_eq!(decoded, command);
    }

    #[test]
    fn put_round_trip() {
        round_trip(Command::Put {
            key: bytes(b"k"),
            value: bytes(b"v"),
        });
    }

    #[test]
    fn delete_round_trip() {
        round_trip(Command::Delete { key: bytes(b"k") });
    }

    #[test]
    fn put_empty_value() {
        round_trip(Command::Put {
            key: bytes(b"k"),
            value: bytes(b""),
        });
    }

    #[test]
    fn put_empty_key() {
        round_trip(Command::Put {
            key: bytes(b""),
            value: bytes(b"v"),
        });
    }

    #[test]
    fn put_exact_bytes() {
        let command = Command::Put {
            key: bytes(b"ab"),
            value: bytes(b"c"),
        };

        let mut encoded = Vec::new();
        command.encode(&mut encoded).unwrap();

        assert_eq!(encoded, vec![1, 2, 0, 0, 0, b'a', b'b', b'c']);
    }

    #[test]
    fn unknown_tag() {
        assert!(matches!(Command::decode(&[9]), Err(Error::Corruption(_))));
    }

    #[test]
    fn empty_payload() {
        assert!(matches!(Command::decode(&[]), Err(Error::Corruption(_))));
    }

    #[test]
    fn key_length_too_big() {
        assert!(matches!(
            Command::decode(&[1, 255, 0, 0, 0, b'k']),
            Err(Error::Corruption(_))
        ));
    }

    #[test]
    fn delete_with_extra_bytes() {
        assert!(matches!(
            Command::decode(&[2, 1, 0, 0, 0, b'k', b'x']),
            Err(Error::Corruption(_))
        ));
    }
}
