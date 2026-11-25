use bytes::Bytes;

use crate::durable::DurableStorage;
use crate::mem::MemStorage;
use crate::storage::Storage;

// 1. Getting a missing key returns None.
fn get_missing<S: Storage>(store: &mut S) {
    let result = store.get(b"missing").unwrap();

    assert_eq!(result, None);
}

// 2. Put followed by get returns the inserted value.
fn put_then_get<S: Storage>(store: &mut S) {
    store
        .put(Bytes::from_static(b"key"), Bytes::from_static(b"value"))
        .unwrap();

    let result = store.get(b"key").unwrap();

    assert_eq!(result, Some(Bytes::from_static(b"value")));
}

// 3. The last write to a key wins.
fn last_write_wins<S: Storage>(store: &mut S) {
    store
        .put(Bytes::from_static(b"key"), Bytes::from_static(b"first"))
        .unwrap();

    store
        .put(Bytes::from_static(b"key"), Bytes::from_static(b"second"))
        .unwrap();

    let result = store.get(b"key").unwrap();

    assert_eq!(result, Some(Bytes::from_static(b"second")));
}

// 4. Put followed by delete makes the key disappear.
fn put_delete_get<S: Storage>(store: &mut S) {
    store
        .put(Bytes::from_static(b"key"), Bytes::from_static(b"value"))
        .unwrap();

    store.delete(b"key").unwrap();

    let result = store.get(b"key").unwrap();

    assert_eq!(result, None);
}

// 5. Deleting a missing key succeeds.
fn delete_missing<S: Storage>(store: &mut S) {
    let result = store.delete(b"missing");

    assert!(result.is_ok());
}

// 6. Scan respects bounds and returns keys in sorted order.
fn scan_bounds_order<S: Storage>(store: &mut S) {
    store
        .put(Bytes::from_static(b"a"), Bytes::from_static(b"1"))
        .unwrap();
    store
        .put(Bytes::from_static(b"b"), Bytes::from_static(b"2"))
        .unwrap();
    store
        .put(Bytes::from_static(b"c"), Bytes::from_static(b"3"))
        .unwrap();
    store
        .put(Bytes::from_static(b"d"), Bytes::from_static(b"4"))
        .unwrap();

    // Start is inclusive, end is exclusive.
    let result = store.scan(b"b", b"d", 10).unwrap();

    let expected = vec![
        (Bytes::from_static(b"b"), Bytes::from_static(b"2")),
        (Bytes::from_static(b"c"), Bytes::from_static(b"3")),
    ];

    assert_eq!(result, expected);
}

// 7. Scan respects limits and handles reversed ranges.
fn scan_limit_reversed<S: Storage>(store: &mut S) {
    store
        .put(Bytes::from_static(b"a"), Bytes::from_static(b"1"))
        .unwrap();
    store
        .put(Bytes::from_static(b"b"), Bytes::from_static(b"2"))
        .unwrap();
    store
        .put(Bytes::from_static(b"c"), Bytes::from_static(b"3"))
        .unwrap();

    // Limit should restrict the number of returned entries.
    let result = store.scan(b"a", b"d", 2).unwrap();

    let expected = vec![
        (Bytes::from_static(b"a"), Bytes::from_static(b"1")),
        (Bytes::from_static(b"b"), Bytes::from_static(b"2")),
    ];

    assert_eq!(result, expected);

    // Reversed range should return an empty result.
    let result = store.scan(b"c", b"a", 10).unwrap();

    assert!(result.is_empty());
}

// Run every test against a fresh storage instance.
fn run_all<S: Storage>(mut make_store: impl FnMut() -> S) {
    let mut store = make_store();
    get_missing(&mut store);

    let mut store = make_store();
    put_then_get(&mut store);

    let mut store = make_store();
    last_write_wins(&mut store);

    let mut store = make_store();
    put_delete_get(&mut store);

    let mut store = make_store();
    delete_missing(&mut store);

    let mut store = make_store();
    scan_bounds_order(&mut store);

    let mut store = make_store();
    scan_limit_reversed(&mut store);
}

// Run the conformance suite against MemStorage.
#[test]
fn mem_conformance() {
    run_all(MemStorage::new);
}

// Run the conformance suite against DurableStorage.
#[test]
fn durable_conformance() {
    let dir = tempfile::tempdir().unwrap();
    let mut counter = 0;

    run_all(|| {
        let path = dir.path().join(format!("wal-{counter}"));
        counter += 1;

        DurableStorage::open(path).unwrap()
    });
}
