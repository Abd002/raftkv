use crate::{Error, Result, decode, encode};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::Path,
};

#[derive(Debug)]
pub struct Wal {
    file: File,
    buf: Vec<u8>,
    poisoned: bool,
}

impl Wal {
    /// Zero-filled tails left after a crash can cause WAL recovery to fail.
    /// configure the Wal
    /// return all data in the file in a vector
    pub fn open<A: AsRef<Path>>(path: A) -> Result<(Wal, Vec<Vec<u8>>)> {
        let new_file = !path.as_ref().exists();

        let mut file = OpenOptions::new()
            .read(true)
            .append(true)
            .create(true)
            .open(&path)?;

        if new_file {
            let parent = path
                .as_ref()
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or(Path::new("."));

            File::open(parent)?.sync_all()?;
        }
        let mut content = Vec::new();
        file.read_to_end(&mut content)?;

        let mut payloads: Vec<Vec<u8>> = Vec::new();
        let mut offset = 0;
        while let Some((payload, used)) = decode(&content[offset..])
            .map_err(|e| Error::Corruption(format!("Corruption: record at offset {offset}: {e}")))?
        {
            payloads.push(payload.to_vec());
            offset += used as usize;
        }
        if offset < content.len() {
            file.set_len(offset as u64)?;
            file.sync_all()?;

            eprintln!(
                "WAL recovery: truncated {} bytes from invalid tail",
                content.len() - offset
            );
        }

        Ok((
            Wal {
                file,
                buf: content,
                poisoned: false,
            },
            payloads,
        ))
    }

    /// Not durable until sync returns Ok.
    pub fn append(&mut self, payload: &[u8]) -> Result<()> {
        if self.poisoned {
            return Err(Error::Poisoned);
        }

        self.buf.clear();

        encode(payload, &mut self.buf)?;
        self.file
            .write_all(&self.buf)
            .inspect_err(|_| self.poisoned = true)?;
        Ok(())
    }

    pub fn sync(&mut self) -> Result<()> {
        if self.poisoned {
            return Err(Error::Poisoned);
        }

        self.file
            .sync_data()
            .inspect_err(|_| self.poisoned = true)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::{Error, MAX_RECORD_SIZE, Wal, encode};
    use std::{
        fs::{self, OpenOptions},
        io::Write,
    };
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

    #[test]
    fn torn_tail_is_cut_off() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("wal");

        {
            let (mut wal, _) = Wal::open(&path).unwrap();

            wal.append(b"a").unwrap();
            wal.append(b"b").unwrap();
            wal.sync().unwrap();
        }

        let mut v = Vec::new();
        encode(b"ccc", &mut v).unwrap();

        {
            let mut file = OpenOptions::new().append(true).open(&path).unwrap();

            file.write_all(&v[..5]).unwrap();
        }

        let (_wal, records) = Wal::open(&path).unwrap();

        assert_eq!(records, vec![b"a".to_vec(), b"b".to_vec()]);

        let metadata = fs::metadata(&path).unwrap();
        assert_eq!(metadata.len(), 18);
    }

    #[test]
    fn append_after_torn_tail() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("wal");

        {
            let (mut wal, _) = Wal::open(&path).unwrap();

            wal.append(b"a").unwrap();
            wal.append(b"b").unwrap();
            wal.sync().unwrap();
        }

        let mut v = Vec::new();
        encode(b"ccc", &mut v).unwrap();

        {
            let mut file = OpenOptions::new().append(true).open(&path).unwrap();

            file.write_all(&v[..5]).unwrap();
        }

        {
            let (mut wal, records) = Wal::open(&path).unwrap();

            assert_eq!(records, vec![b"a".to_vec(), b"b".to_vec()]);

            wal.append(b"d").unwrap();
            wal.sync().unwrap();
        }

        let (_wal, records) = Wal::open(&path).unwrap();

        assert_eq!(records, vec![b"a".to_vec(), b"b".to_vec(), b"d".to_vec()]);
    }

    #[test]
    fn corruption_in_middle_refuses_to_open() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("wal");

        {
            let (mut wal, _) = Wal::open(&path).unwrap();

            wal.append(b"aaaa").unwrap();
            wal.append(b"bbbb").unwrap();
            wal.append(b"cccc").unwrap();

            wal.sync().unwrap();
        }

        let mut bytes = fs::read(&path).unwrap();

        bytes[20] ^= 1;

        fs::write(&path, bytes).unwrap();

        let result = Wal::open(&path);

        assert!(matches!(result, Err(Error::Corruption(_))));

        let error = result.unwrap_err().to_string();

        assert!(error.contains("12"), "unexpected error: {error}");
    }

    #[test]
    fn too_large_does_not_poison() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("wal");

        {
            let (mut wal, _) = Wal::open(&path).unwrap();

            let payload = vec![0u8; MAX_RECORD_SIZE as usize + 1];

            let result = wal.append(&payload);

            assert!(matches!(result, Err(Error::RecordTooLarge)));

            wal.append(b"ok").unwrap();
            wal.sync().unwrap();
        }

        let (_wal, records) = Wal::open(&path).unwrap();

        assert_eq!(records, vec![b"ok".to_vec()]);
    }
}
