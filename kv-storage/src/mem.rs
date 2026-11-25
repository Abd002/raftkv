use std::collections::BTreeMap;
use std::ops::Bound::{Excluded, Included};

use bytes::Bytes;

use crate::storage::Storage;

#[derive(Debug, Default)]
pub struct MemStorage {
    map: BTreeMap<Bytes, Bytes>,
}

impl MemStorage {
    pub fn new() -> Self {
        MemStorage {
            map: BTreeMap::new(),
        }
    }
}

impl Storage for MemStorage {
    fn get(&self, key: &[u8]) -> crate::Result<Option<bytes::Bytes>> {
        let value = self.map.get(key).cloned();

        Ok(value)
    }

    fn put(&mut self, key: bytes::Bytes, value: bytes::Bytes) -> crate::Result<()> {
        self.map.insert(key, value);

        Ok(())
    }

    fn delete(&mut self, key: &[u8]) -> crate::Result<()> {
        self.map.remove(key);

        Ok(())
    }

    fn scan(
        &self,
        start: &[u8],
        end: &[u8],
        limit: usize,
    ) -> crate::Result<Vec<(bytes::Bytes, bytes::Bytes)>> {
        if start >= end {
            return Ok(Vec::new());
        }

        let iter = self.map.range::<[u8], _>((Included(start), Excluded(end)));

        let mut ret: Vec<(Bytes, Bytes)> = Vec::new();

        for (count, (key, value)) in iter.enumerate() {
            if count == limit {
                break;
            }

            ret.push((key.clone(), value.clone()));
        }

        Ok(ret)
    }
}
