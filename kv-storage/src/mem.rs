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

#[cfg(test)]
mod tests {
    use bytes::Buf;

    use super::*;

    #[test]
    fn get_on_empty() {
        let key = "a";

        let store = MemStorage::new();

        let ans = store.get(key.as_bytes()).unwrap();

        assert_eq!(ans, None);
    }

    #[test]
    fn get_value_after_put_it() {
        let key = Bytes::copy_from_slice("a".as_bytes());
        let value = Bytes::copy_from_slice("abc".as_bytes());

        let mut store = MemStorage::new();

        store.put(key.clone(), value.clone()).unwrap();

        let ans = store.get(key.chunk()).unwrap();

        assert_eq!(ans, Some(value));
    }

    #[test]
    fn double_put_last_win() {
        let key = Bytes::copy_from_slice("a".as_bytes());
        let value_old = Bytes::copy_from_slice("abc".as_bytes());
        let value_new = Bytes::copy_from_slice("def".as_bytes());

        let mut store = MemStorage::new();

        store.put(key.clone(), value_old.clone()).unwrap();
        store.put(key.clone(), value_new.clone()).unwrap();

        let ans = store.get(key.chunk()).unwrap();

        assert_eq!(ans, Some(value_new));
    }

    #[test]
    fn put_delete_check() {
        let key = Bytes::copy_from_slice("a".as_bytes());
        let value = Bytes::copy_from_slice("abc".as_bytes());

        let mut store = MemStorage::new();

        store.put(key.clone(), value.clone()).unwrap();

        store.delete(key.chunk()).unwrap();

        let ans = store.get(key.chunk()).unwrap();

        assert_eq!(ans, None);
    }

    #[test]
    fn delete_not_exist_key() {
        let key = "a";
        let mut store = MemStorage::new();

        let ret = store.delete(key.as_bytes());

        assert!(ret.is_ok())
    }

    #[test]
    fn scan_includes_start_excludes_end_sorted() {
        let mut store = MemStorage::new();

        store
            .put(
                Bytes::copy_from_slice("a".as_bytes()),
                Bytes::copy_from_slice("1".as_bytes()),
            )
            .unwrap();
        store
            .put(
                Bytes::copy_from_slice("b".as_bytes()),
                Bytes::copy_from_slice("2".as_bytes()),
            )
            .unwrap();
        store
            .put(
                Bytes::copy_from_slice("c".as_bytes()),
                Bytes::copy_from_slice("3".as_bytes()),
            )
            .unwrap();
        store
            .put(
                Bytes::copy_from_slice("d".as_bytes()),
                Bytes::copy_from_slice("4".as_bytes()),
            )
            .unwrap();

        let mut iter = store
            .scan("b".as_bytes(), "d".as_bytes(), 10)
            .unwrap()
            .into_iter();

        assert_eq!(
            iter.next().unwrap().clone(),
            (
                Bytes::copy_from_slice("b".as_bytes()),
                Bytes::copy_from_slice("2".as_bytes()),
            )
        );
        assert_eq!(
            iter.next().unwrap().clone(),
            (
                Bytes::copy_from_slice("c".as_bytes()),
                Bytes::copy_from_slice("3".as_bytes()),
            )
        );
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn scan_with_limit() {
        let mut store = MemStorage::new();

        store
            .put(
                Bytes::copy_from_slice("a".as_bytes()),
                Bytes::copy_from_slice("1".as_bytes()),
            )
            .unwrap();
        store
            .put(
                Bytes::copy_from_slice("b".as_bytes()),
                Bytes::copy_from_slice("2".as_bytes()),
            )
            .unwrap();
        store
            .put(
                Bytes::copy_from_slice("c".as_bytes()),
                Bytes::copy_from_slice("3".as_bytes()),
            )
            .unwrap();
        store
            .put(
                Bytes::copy_from_slice("d".as_bytes()),
                Bytes::copy_from_slice("4".as_bytes()),
            )
            .unwrap();

        let mut iter = store
            .scan("b".as_bytes(), "d".as_bytes(), 1)
            .unwrap()
            .into_iter();

        let mut empty_iter = store
            .scan("d".as_bytes(), "b".as_bytes(), 10)
            .unwrap()
            .into_iter();

        assert_eq!(
            iter.next().unwrap().clone(),
            (
                Bytes::copy_from_slice("b".as_bytes()),
                Bytes::copy_from_slice("2".as_bytes()),
            )
        );
        assert_eq!(iter.next(), None);
        assert_eq!(empty_iter.next(), None);
    }
}
