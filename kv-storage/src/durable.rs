use bytes::Bytes;
use kv_wal::Wal;

use crate::{Command, MemStorage, Result, Storage};

/// Every write calls fsync, so writes are limited to a few hundred or a few thousand per second, depending on the disk.
#[derive(Debug)]
pub struct DurableStorage {
    wal: kv_wal::Wal,
    mem_storage: MemStorage,
}

impl DurableStorage {
    pub fn open(path: impl AsRef<std::path::Path>) -> Result<Self> {
        let wal = Wal::open(path)?;
        let mut durable_storage = DurableStorage {
            wal: wal.0,
            mem_storage: MemStorage::new(),
        };

        for v in wal.1.iter() {
            let command = Command::decode(v)?;

            match command {
                Command::Put { key, value } => durable_storage.mem_storage.put(key, value)?,
                Command::Delete { key } => durable_storage.mem_storage.delete(&key)?,
            }
        }

        Ok(durable_storage)
    }
}

impl Storage for DurableStorage {
    fn get(&self, key: &[u8]) -> crate::Result<Option<bytes::Bytes>> {
        self.mem_storage.get(key)
    }

    fn put(&mut self, key: bytes::Bytes, value: bytes::Bytes) -> crate::Result<()> {
        let command = Command::Put {
            key: key.clone(),
            value: value.clone(),
        };
        let mut out = Vec::new();
        command.encode(&mut out)?;

        self.wal.append(out.as_slice())?;
        self.wal.sync()?;

        self.mem_storage.put(key, value)?;
        Ok(())
    }

    fn delete(&mut self, key: &[u8]) -> crate::Result<()> {
        let command = Command::Delete {
            key: Bytes::copy_from_slice(key),
        };
        let mut out = Vec::new();
        command.encode(&mut out)?;

        self.wal.append(out.as_slice())?;
        self.wal.sync()?;

        self.mem_storage.delete(key)?;
        Ok(())
    }

    fn scan(
        &self,
        start: &[u8],
        end: &[u8],
        limit: usize,
    ) -> crate::Result<Vec<(bytes::Bytes, bytes::Bytes)>> {
        self.mem_storage.scan(start, end, limit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Storage;
    use bytes::Bytes;
    use std::fs::OpenOptions;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn put_survives_reopen() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("data.wal");

        {
            let mut storage = DurableStorage::open(&path).unwrap();

            storage
                .put(Bytes::from_static(b"a"), Bytes::from_static(b"1"))
                .unwrap();

            storage
                .put(Bytes::from_static(b"b"), Bytes::from_static(b"2"))
                .unwrap();
        }

        let storage = DurableStorage::open(&path).unwrap();

        assert_eq!(storage.get(b"a").unwrap(), Some(Bytes::from_static(b"1")));

        assert_eq!(storage.get(b"b").unwrap(), Some(Bytes::from_static(b"2")));
    }

    #[test]
    fn delete_survives_reopen() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("data.wal");

        {
            let mut storage = DurableStorage::open(&path).unwrap();

            storage
                .put(Bytes::from_static(b"a"), Bytes::from_static(b"1"))
                .unwrap();

            storage.delete(b"a").unwrap();
        }

        let storage = DurableStorage::open(&path).unwrap();

        assert_eq!(storage.get(b"a").unwrap(), None);
    }

    #[test]
    fn last_write_wins_after_reopen() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("data.wal");

        {
            let mut storage = DurableStorage::open(&path).unwrap();

            storage
                .put(Bytes::from_static(b"a"), Bytes::from_static(b"1"))
                .unwrap();

            storage
                .put(Bytes::from_static(b"a"), Bytes::from_static(b"2"))
                .unwrap();
        }

        let storage = DurableStorage::open(&path).unwrap();

        assert_eq!(storage.get(b"a").unwrap(), Some(Bytes::from_static(b"2")));
    }

    #[test]
    fn scan_sorted_after_reopen() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("data.wal");

        {
            let mut storage = DurableStorage::open(&path).unwrap();

            storage
                .put(Bytes::from_static(b"c"), Bytes::from_static(b"3"))
                .unwrap();

            storage
                .put(Bytes::from_static(b"a"), Bytes::from_static(b"1"))
                .unwrap();

            storage
                .put(Bytes::from_static(b"b"), Bytes::from_static(b"2"))
                .unwrap();
        }

        let storage = DurableStorage::open(&path).unwrap();

        let result = storage.scan(b"a", b"z", 10).unwrap();

        assert_eq!(
            result,
            vec![
                (Bytes::from_static(b"a"), Bytes::from_static(b"1")),
                (Bytes::from_static(b"b"), Bytes::from_static(b"2")),
                (Bytes::from_static(b"c"), Bytes::from_static(b"3")),
            ]
        );
    }

    #[test]
    fn many_reopens() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("data.wal");

        for i in 0..5 {
            let mut storage = DurableStorage::open(&path).unwrap();

            let key = Bytes::from(format!("k{i}"));
            let value = Bytes::from(format!("v{i}"));

            storage.put(key, value).unwrap();

            // Storage is dropped at the end of this iteration.
        }

        let storage = DurableStorage::open(&path).unwrap();

        for i in 0..5 {
            let key = format!("k{i}");
            let value = Bytes::from(format!("v{i}"));

            assert_eq!(storage.get(key.as_bytes()).unwrap(), Some(value));
        }
    }

    #[test]
    fn torn_tail_end_to_end() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("data.wal");

        {
            let mut storage = DurableStorage::open(&path).unwrap();

            storage
                .put(Bytes::from_static(b"a"), Bytes::from_static(b"1"))
                .unwrap();
        }

        // Append an incomplete WAL record.
        {
            let mut file = OpenOptions::new().append(true).open(&path).unwrap();

            file.write_all(&[1, 2, 3]).unwrap();
            file.sync_all().unwrap();
        }

        // Opening should recover the valid records and discard the torn tail.
        let storage = DurableStorage::open(&path).unwrap();

        assert_eq!(storage.get(b"a").unwrap(), Some(Bytes::from_static(b"1")));
    }

    #[test]
    fn bad_command_refuses_to_open() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("data.wal");

        // Create a valid WAL containing an invalid storage command.
        {
            let (mut wal, _) = Wal::open(&path).unwrap();

            wal.append(&[9]).unwrap();
            wal.sync().unwrap();
        }

        // The WAL itself is valid, but the command is not.
        let result = DurableStorage::open(&path);

        assert!(result.is_err());

        match result {
            Err(crate::Error::Corruption(_)) => {}
            other => panic!("expected Corruption error, got: {other:?}"),
        }
    }
}
