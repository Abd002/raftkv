use crate::{Result, decode, encode};
use std::{
    fs::OpenOptions,
    io::{Read, Write},
    path::Path,
};

pub struct Wal {
    file: std::fs::File,
    buf: Vec<u8>,
    #[expect(dead_code)]
    poisoned: bool,
}

impl Wal {
    pub fn open<A: AsRef<Path>>(path: A) -> Result<(Wal, Vec<Vec<u8>>)> {
        let mut file = OpenOptions::new()
            .read(true)
            .append(true)
            .create(true)
            .open(path)?;

        let mut content = Vec::new();
        file.read_to_end(&mut content)?;

        let mut payloads: Vec<Vec<u8>> = Vec::new();
        let mut offset = 0;
        while let Some((payload, used)) = decode(&content[offset..])? {
            payloads.push(payload.to_vec());
            offset += used as usize;
        }
        let poisoned = offset != content.len();
        Ok((
            Wal {
                file,
                buf: content,
                poisoned,
            },
            payloads,
        ))
    }

    /// Not durable until sync returns Ok.
    pub fn append(&mut self, payload: &[u8]) -> Result<()> {
        let offset = self.buf.len();
        encode(payload, &mut self.buf)?;
        self.file.write_all(&self.buf[offset..])?;
        Ok(())
    }

    pub fn sync(&mut self) -> Result<()> {
        self.file.sync_data()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::Wal;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn open_new_file_is_empty() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("wal");

        let (wal, records) = Wal::open(&path).unwrap();

        assert!(records.is_empty());
        assert!(path.exists());

        drop(wal);
    }

    #[test]
    fn append_sync_reopen() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("wal");

        {
            let (mut wal, _) = Wal::open(&path).unwrap();

            wal.append(b"a").unwrap();
            wal.append(b"bb").unwrap();
            wal.append(b"ccc").unwrap();

            wal.sync().unwrap();
        }

        let (_wal, records) = Wal::open(&path).unwrap();

        assert_eq!(
            records,
            vec![b"a".to_vec(), b"bb".to_vec(), b"ccc".to_vec(),]
        );
    }

    #[test]
    fn reopen_then_append_keeps_old_records() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("wal");

        {
            let (mut wal, _) = Wal::open(&path).unwrap();
            wal.append(b"a").unwrap();
            wal.sync().unwrap();
        }

        {
            let (mut wal, records) = Wal::open(&path).unwrap();

            assert_eq!(records, vec![b"a".to_vec()]);

            wal.append(b"b").unwrap();
            wal.sync().unwrap();
        }

        let (_wal, records) = Wal::open(&path).unwrap();

        assert_eq!(records, vec![b"a".to_vec(), b"b".to_vec()]);
    }

    #[test]
    fn file_size_matches_format() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("wal");

        {
            let (mut wal, _) = Wal::open(&path).unwrap();

            wal.append(b"a").unwrap();
            wal.append(b"bb").unwrap();

            wal.sync().unwrap();
        }

        let metadata = fs::metadata(&path).unwrap();

        assert_eq!(metadata.len(), 19);
    }
}
