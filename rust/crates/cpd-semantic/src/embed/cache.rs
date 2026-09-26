// cache.rs — vectors kept between runs, one file per model and request shape.
//
// A file is an 8-byte magic, the dimension count as a little-endian u32, then
// records of a 16-byte text hash and the vector. Records are appended, one
// write each, so two runs sharing the file never interleave inside one.

use serde_json::Value;
use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use xxhash_rust::xxh3::xxh3_64;

pub const CACHE_DIR_ENV: &str = "JSCPD_CACHE_DIR";
/// Part of the cache key: bump it when the text given to the model changes
/// shape, so vectors of the old texts are not reused.
const TEXT_VERSION: u32 = 1;
const MAGIC: &[u8; 8] = b"JSCPDEM1";
/// A cache file past this size is rewritten with only this run's vectors.
const MAX_BYTES: u64 = 256 * 1024 * 1024;

/// Where jscpd keeps caches and models: `$JSCPD_CACHE_DIR`, else the
/// platform's user cache directory.
pub fn root() -> Option<PathBuf> {
    let var = |name: &str| {
        std::env::var_os(name)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    };
    if let Some(dir) = var(CACHE_DIR_ENV) {
        return Some(dir);
    }
    if cfg!(windows) {
        var("LOCALAPPDATA").map(|d| d.join("jscpd").join("cache"))
    } else if cfg!(target_os = "macos") {
        var("HOME").map(|h| h.join("Library").join("Caches").join("jscpd"))
    } else {
        var("XDG_CACHE_HOME")
            .map(|d| d.join("jscpd"))
            .or_else(|| var("HOME").map(|h| h.join(".cache").join("jscpd")))
    }
}

/// `<model>-<hash of everything that changes vectors>.bin`.
pub fn file_name(model: &str, identity: &Value) -> String {
    let key = serde_json::json!({ "text": TEXT_VERSION, "identity": identity });
    let slug: String = model
        .chars()
        .map(
            |c| match c.is_ascii_alphanumeric() || matches!(c, '.' | '-') {
                true => c,
                false => '_',
            },
        )
        .take(60)
        .collect();
    format!("{slug}-{:016x}.bin", xxh3_64(key.to_string().as_bytes()))
}

#[derive(Default)]
pub struct Cache {
    pub dims: usize,
    pub vectors: HashMap<u128, Vec<f32>>,
    /// Bytes in the file when it was read.
    size: u64,
}

/// Read a cache file. A missing, foreign or damaged file reads as empty; a
/// torn last record (an interrupted write) is dropped.
pub fn load(path: &Path) -> Cache {
    let Ok(bytes) = std::fs::read(path) else {
        return Cache::default();
    };
    if bytes.len() < 12 || &bytes[..8] != MAGIC {
        return Cache::default();
    }
    let dims = u32::from_le_bytes(bytes[8..12].try_into().unwrap_or_default()) as usize;
    if dims == 0 {
        return Cache::default();
    }
    let record = 16 + dims * 4;
    let mut vectors = HashMap::new();
    for chunk in bytes[12..].chunks_exact(record) {
        let key = u128::from_le_bytes(chunk[..16].try_into().unwrap_or_default());
        let vector = chunk[16..]
            .chunks_exact(4)
            .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            .collect();
        vectors.insert(key, vector);
    }
    Cache {
        dims,
        vectors,
        size: bytes.len() as u64,
    }
}

/// Append `fresh` to the cache file, creating it when missing. A file grown
/// past [`MAX_BYTES`] (or holding another dimension count) is rewritten
/// with only the vectors of this run's `keys`.
pub fn save(
    path: &Path,
    cache: &mut Cache,
    fresh: &[(u128, Vec<f32>)],
    keys: &[u128],
) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let record = |key: u128, v: &[f32]| {
        let mut buf = Vec::with_capacity(16 + v.len() * 4);
        buf.extend_from_slice(&key.to_le_bytes());
        for x in v {
            buf.extend_from_slice(&x.to_le_bytes());
        }
        buf
    };
    let on_disk = header_dims(path);
    if cache.size > MAX_BYTES || on_disk != Some(cache.dims) {
        let mut buf = MAGIC.to_vec();
        buf.extend_from_slice(&(cache.dims as u32).to_le_bytes());
        let mut written = std::collections::HashSet::new();
        for (key, v) in fresh {
            if written.insert(*key) {
                buf.extend(record(*key, v));
            }
        }
        if on_disk == Some(cache.dims) {
            // Compaction: keep what this run used.
            for key in keys {
                if let Some(v) = cache.vectors.get(key)
                    && written.insert(*key)
                {
                    buf.extend(record(*key, v));
                }
            }
        }
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, &buf)?;
        std::fs::rename(&tmp, path)?;
        cache.size = buf.len() as u64;
        return Ok(());
    }
    let mut file = std::fs::OpenOptions::new().append(true).open(path)?;
    for (key, v) in fresh {
        file.write_all(&record(*key, v))?;
    }
    Ok(())
}

fn header_dims(path: &Path) -> Option<usize> {
    let mut header = [0u8; 12];
    let mut file = std::fs::File::open(path).ok()?;
    std::io::Read::read_exact(&mut file, &mut header).ok()?;
    (&header[..8] == MAGIC)
        .then(|| u32::from_le_bytes([header[8], header[9], header[10], header[11]]) as usize)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("jscpd-semantic-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn file_name_depends_on_the_identity() {
        let a = file_name(
            "jinaai/jina-embeddings-v2-base-code",
            &serde_json::json!({"x": 1}),
        );
        let b = file_name(
            "jinaai/jina-embeddings-v2-base-code",
            &serde_json::json!({"x": 2}),
        );
        assert!(a.starts_with("jinaai_jina-embeddings-v2-base-code-"), "{a}");
        assert!(a.ends_with(".bin"));
        assert_ne!(a, b);
    }

    #[test]
    fn cache_round_trips_appends_and_survives_a_torn_record() {
        let path = scratch("cache").join("embeddings").join("m.bin");
        let mut cache = Cache {
            dims: 3,
            ..Cache::default()
        };
        save(&path, &mut cache, &[(1, vec![1.0, 2.0, 3.0])], &[1]).unwrap();
        let mut cache = load(&path);
        assert_eq!(cache.dims, 3);
        assert_eq!(cache.vectors[&1], vec![1.0, 2.0, 3.0]);
        save(&path, &mut cache, &[(2, vec![4.0, 5.0, 6.0])], &[1, 2]).unwrap();
        let loaded = load(&path);
        assert_eq!(loaded.vectors.len(), 2);
        assert_eq!(loaded.vectors[&2], vec![4.0, 5.0, 6.0]);
        // Half a record at the end, as an interrupted run leaves it.
        std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap()
            .write_all(&[7; 9])
            .unwrap();
        assert_eq!(load(&path).vectors.len(), 2);
    }

    #[test]
    fn a_cache_of_another_dimension_count_is_replaced() {
        let path = scratch("dims").join("m.bin");
        let mut cache = Cache {
            dims: 2,
            ..Cache::default()
        };
        save(&path, &mut cache, &[(1, vec![1.0, 2.0])], &[1]).unwrap();
        let mut cache = Cache {
            dims: 3,
            ..Cache::default()
        };
        save(&path, &mut cache, &[(5, vec![1.0, 2.0, 3.0])], &[5]).unwrap();
        let loaded = load(&path);
        assert_eq!(loaded.dims, 3);
        assert_eq!(loaded.vectors.keys().copied().collect::<Vec<_>>(), vec![5]);
    }

    #[test]
    fn garbage_reads_as_an_empty_cache() {
        let path = scratch("garbage").join("m.bin");
        std::fs::write(&path, b"not a cache").unwrap();
        assert!(load(&path).vectors.is_empty());
        assert!(load(&path.with_extension("missing")).vectors.is_empty());
    }
}
