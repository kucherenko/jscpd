use rustc_hash::FxHashMap;

/// Lightweight reference stored in the rolling-hash window store.
///
/// `file_idx` indexes into the per-call `prepared` array inside a single
/// `detect_in_group` invocation. The scope is intra-call only — the value is
/// meaningless across format groups. This is enforced by each
/// `detect_in_group` call owning its own `MemoryStore`, so no cross-call
/// `file_idx` collision is possible.
#[derive(Debug, Clone, PartialEq)]
pub struct SourceRef {
    pub file_idx: usize,
    pub token_index: usize,
}

/// Backing store for the sliding-window hash lookup during clone detection.
/// Object-safe: usable as `&mut dyn Store`.
pub trait Store: Send + Sync {
    fn get(&self, key: u64) -> Option<&SourceRef>;
    fn set(&mut self, key: u64, val: SourceRef);
    fn clear(&mut self);
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// Performance hint — pre-allocates capacity for `n` window entries.
    ///
    /// Correctness must not depend on this being called or honoured.
    /// Implementations that do not benefit from pre-allocation may leave
    /// this as a no-op (the default).
    fn reserve(&mut self, _n: usize) {}
}

/// In-memory FxHashMap-backed store.
pub struct MemoryStore {
    inner: FxHashMap<u64, SourceRef>,
}

impl MemoryStore {
    pub fn new() -> Self {
        Self { inner: FxHashMap::default() }
    }
}

impl Default for MemoryStore {
    fn default() -> Self {
        Self::new()
    }
}

impl Store for MemoryStore {
    fn get(&self, key: u64) -> Option<&SourceRef> {
        self.inner.get(&key)
    }

    fn set(&mut self, key: u64, val: SourceRef) {
        self.inner.insert(key, val);
    }

    fn clear(&mut self) {
        self.inner.clear();
    }

    fn len(&self) -> usize {
        self.inner.len()
    }

    fn reserve(&mut self, n: usize) {
        self.inner.reserve(n);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_store_set_and_get_roundtrip() {
        let mut store = MemoryStore::new();
        let sref = SourceRef { file_idx: 0, token_index: 5 };
        store.set(42u64, sref.clone());
        assert_eq!(store.get(42u64), Some(&sref));
    }

    #[test]
    fn memory_store_get_missing_returns_none() {
        let store = MemoryStore::new();
        assert_eq!(store.get(999u64), None);
    }

    #[test]
    fn memory_store_clear_empties_store() {
        let mut store = MemoryStore::new();
        store.set(1, SourceRef { file_idx: 0, token_index: 0 });
        assert!(!store.is_empty());
        store.clear();
        assert!(store.is_empty());
    }

    #[test]
    fn store_is_object_safe() {
        // This test proves the trait is object-safe: if it compiles, it passes
        let mut mem = MemoryStore::new();
        let store: &mut dyn Store = &mut mem;
        store.set(7, SourceRef { file_idx: 1, token_index: 3 });
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn reserve_is_a_hint_and_does_not_affect_correctness() {
        let mut store = MemoryStore::new();
        store.reserve(1000);
        let sref = SourceRef { file_idx: 2, token_index: 1 };
        store.set(10, sref.clone());
        assert_eq!(store.get(10), Some(&sref));
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn reserve_works_via_dyn_store() {
        let mut mem = MemoryStore::new();
        let store: &mut dyn Store = &mut mem;
        // reserve() is callable through dyn Store without breaking object safety
        store.reserve(100);
        store.set(5, SourceRef { file_idx: 3, token_index: 0 });
        assert_eq!(store.len(), 1);
    }
}
