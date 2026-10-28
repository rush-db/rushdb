//! `derived::last_key` finds the largest key under a prefix with forward
//! seeks only; it must agree with a full scan on any key set, including keys
//! that are prefixes of others and bytes at 0x00 and 0xFF.

use std::collections::BTreeMap;

use rushdb_lmpg::derived::last_key;
use rushdb_lmpg::reader::TryRead;
use rushdb_storage::StorageError;

type KeyValueRows = Vec<(Vec<u8>, Vec<u8>)>;

struct Map(BTreeMap<Vec<u8>, Vec<u8>>);

impl TryRead for Map {
    fn get(&self, k: &[u8]) -> Result<Option<Vec<u8>>, StorageError> {
        Ok(self.0.get(k).cloned())
    }
    fn range_prefix(&self, p: &[u8]) -> Result<KeyValueRows, StorageError> {
        Ok(self
            .0
            .iter()
            .filter(|(k, _)| k.starts_with(p))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect())
    }
    fn range(&self, start: &[u8], end: &[u8]) -> Result<KeyValueRows, StorageError> {
        Ok(self
            .0
            .iter()
            .filter(|(k, _)| k.as_slice() >= start && (end.is_empty() || k.as_slice() < end))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect())
    }
}

#[test]
fn last_key_matches_a_scan() {
    let mut seed = 0x9E37_79B9_7F4A_7C15_u64;
    let mut next = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    let mut pick = |n: usize| usize::try_from(next() % n as u64).expect("below n");
    let alphabet = [0x00, 0x01, 0x7F, 0x80, 0xFE, 0xFF];
    for round in 0..300 {
        let mut map = BTreeMap::new();
        for _ in 0..pick(12) {
            let head = alphabet[pick(3)];
            let mut key = vec![head];
            for _ in 0..pick(5) {
                key.push(alphabet[pick(alphabet.len())]);
            }
            map.insert(key, Vec::new());
        }
        let map = Map(map);
        for prefix in [vec![], vec![0x00], vec![0x01], vec![0x7F], vec![0x00, 0xFF]] {
            let want = map
                .0
                .keys()
                .filter(|k| k.starts_with(&prefix))
                .max()
                .cloned();
            assert_eq!(
                last_key(&map, &prefix).unwrap(),
                want,
                "round {round} prefix {prefix:?}"
            );
        }
    }
}
