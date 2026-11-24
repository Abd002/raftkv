use crate::Result;
use bytes::Bytes;

pub trait Storage {
    /// return Some(value) if it exist else it will return None
    fn get(&self, key: &[u8]) -> Result<Option<Bytes>>;

    /// return what if it successfully applied or not
    /// replace any existing value for that key
    fn put(&mut self, key: Bytes, value: Bytes) -> Result<()>;

    /// Ok() means it applied to the storage successfully
    /// deleting missing key return Ok()
    fn delete(&mut self, key: &[u8]) -> Result<()>;

    /// scan return is sorted by key
    /// if start >= end, it will return empty list
    /// start is included, end is excluded
    /// use limit to limit the number of keys returned it will return the first x ordered pairs
    fn scan(&self, start: &[u8], end: &[u8], limit: usize) -> Result<Vec<(Bytes, Bytes)>>;
}
