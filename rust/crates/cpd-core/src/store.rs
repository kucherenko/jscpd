use rustc_hash::FxHashMap;

/// Lightweight reference stored in the rolling-hash window store.
#[derive(Debug, Clone, PartialEq)]
pub struct SourceRef {
    pub source_id: String,
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_store_set_and_get_roundtrip() {
        let mut store = MemoryStore::new();
        let sref = SourceRef { source_id: "a.js".to_string(), token_index: 5 };
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
        store.set(1, SourceRef { source_id: "x".to_string(), token_index: 0 });
        assert!(!store.is_empty());
        store.clear();
        assert!(store.is_empty());
    }

    #[test]
    fn store_is_object_safe() {
        // This test proves the trait is object-safe: if it compiles, it passes
        let mut mem = MemoryStore::new();
        let store: &mut dyn Store = &mut mem;
        store.set(7, SourceRef { source_id: "b.rs".to_string(), token_index: 3 });
        assert_eq!(store.len(), 1);
    }
}
