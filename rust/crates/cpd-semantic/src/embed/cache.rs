// cache.rs — vectors kept between runs: a folder per set of scanned paths,
// and in it one file per model and request shape.
//
// A file is an 8-byte magic, the dimension count as a little-endian u32, then
// records of a 16-byte text hash and the vector. Records are appended, one
// write each, so two runs sharing the file never interleave inside one.

use serde_json::Value;
use std::collections::{HashMap, HashSet};
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
/// The share of a file's vectors that may go unused by a run that embedded
/// something new before the file is rewritten with only that run's vectors.
/// Unused vectors belong to functions that changed or were deleted, so they
/// pile up as the code changes.
const MAX_UNUSED_SHARE: f64 = 0.25;

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
    format!(
        "{}-{:016x}.bin",
        slug(model, 60),
        xxh3_64(key.to_string().as_bytes())
    )
}

/// `<root>/embeddings/<name>-<hash of the scanned paths>`: the folder of the
/// vector files of runs over `scanned`, in any order. A path inside another
/// adds no files to the scan, so it does not count either. A run over other
/// paths, a subfolder say, gets a folder of its own, so the cleanup after one
/// run never drops vectors that another still uses. The name is that of the
/// folder the paths share, for a reader of the cache directory.
pub fn project_dir(root: &Path, scanned: &[PathBuf]) -> PathBuf {
    let mut paths: Vec<PathBuf> = scanned
        .iter()
        .map(|p| std::fs::canonicalize(p).unwrap_or_else(|_| p.clone()))
        .collect();
    paths.sort();
    paths.dedup();
    // Sorted, an ancestor comes before everything inside it.
    let mut outer: Vec<PathBuf> = Vec::with_capacity(paths.len());
    for path in paths {
        if !outer.iter().any(|o| path.starts_with(o)) {
            outer.push(path);
        }
    }
    let paths = outer;
    let key: Vec<String> = paths
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    let shared = paths.iter().skip(1).fold(paths.first().cloned(), |acc, p| {
        acc.map(|a| {
            a.components()
                .zip(p.components())
                .take_while(|(x, y)| x == y)
                .map(|(x, _)| x)
                .collect::<PathBuf>()
        })
    });
    let name = shared.as_deref().and_then(Path::file_name).map_or_else(
        || "project".to_string(),
        |n| n.to_string_lossy().into_owned(),
    );
    root.join("embeddings").join(format!(
        "{}-{:016x}",
        slug(&name, 40),
        xxh3_64(key.join("\n").as_bytes())
    ))
}

/// `text` as a file name: letters, digits, `.` and `-` kept, the rest `_`,
/// at most `max` characters.
fn slug(text: &str, max: usize) -> String {
    text.chars()
        .map(
            |c| match c.is_ascii_alphanumeric() || matches!(c, '.' | '-') {
                true => c,
                false => '_',
            },
        )
        .take(max)
        .collect()
}

#[derive(Default)]
pub struct Cache {
    pub dims: usize,
    pub vectors: HashMap<u128, Vec<f32>>,
    /// Bytes in the file when it was read.
    size: u64,
    /// Replace the file on the next save instead of appending to it.
    rewrite: bool,
}

impl Cache {
    /// An empty cache whose first save replaces the file with this run's
    /// vectors: `--semantic-rebuild-cache`.
    pub fn replacing() -> Self {
        Cache {
            rewrite: true,
            ..Cache::default()
        }
    }
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
        rewrite: false,
    }
}

/// Append `fresh` to the cache file, creating it when missing. The file is
/// rewritten with only the vectors of this run's `keys` instead when more
/// than [`MAX_UNUSED_SHARE`] of its vectors went unused by this run, when it
/// grew past [`MAX_BYTES`], when it holds another dimension count, or when
/// it was read by [`Cache::replacing`].
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
    let used: HashSet<u128> = keys.iter().copied().collect();
    let unused = cache.vectors.keys().filter(|k| !used.contains(k)).count();
    let stale = unused as f64 > MAX_UNUSED_SHARE * (cache.vectors.len() + fresh.len()) as f64;
    if cache.rewrite || stale || cache.size > MAX_BYTES || on_disk != Some(cache.dims) {
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
        cache.rewrite = false;
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

    use crate::embed::test_dir as scratch;

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
    fn a_replacing_cache_keeps_only_this_runs_vectors() {
        let path = scratch("replace").join("m.bin");
        let mut old = Cache {
            dims: 2,
            ..Cache::default()
        };
        save(
            &path,
            &mut old,
            &[(1, vec![1.0, 2.0]), (2, vec![3.0, 4.0])],
            &[1, 2],
        )
        .unwrap();
        let mut cache = Cache::replacing();
        cache.dims = 2;
        save(&path, &mut cache, &[(1, vec![5.0, 6.0])], &[1]).unwrap();
        let loaded = load(&path);
        assert_eq!(loaded.vectors.len(), 1, "the old vector of 2 is gone");
        assert_eq!(loaded.vectors[&1], vec![5.0, 6.0], "1 is embedded anew");
        // Later saves of the same run append again.
        save(&path, &mut cache, &[(3, vec![7.0, 8.0])], &[1, 3]).unwrap();
        assert_eq!(load(&path).vectors.len(), 2);
    }

    #[test]
    fn a_file_mostly_of_unused_vectors_is_rewritten() {
        let path = scratch("unused").join("m.bin");
        let v = |x: u128| vec![x as f32, 1.0];
        let mut cache = Cache {
            dims: 2,
            ..Cache::default()
        };
        let first: Vec<(u128, Vec<f32>)> = (1..=4).map(|k| (k, v(k))).collect();
        save(&path, &mut cache, &first, &[1, 2, 3, 4]).unwrap();
        // One function of four changed: 1 of 5 vectors unused, appended.
        let mut cache = load(&path);
        save(&path, &mut cache, &[(5, v(5))], &[2, 3, 4, 5]).unwrap();
        assert_eq!(load(&path).vectors.len(), 5);
        // Another changed and one deleted: 3 of 6 unused, rewritten.
        let mut cache = load(&path);
        save(&path, &mut cache, &[(6, v(6))], &[3, 4, 6]).unwrap();
        let mut kept: Vec<u128> = load(&path).vectors.keys().copied().collect();
        kept.sort_unstable();
        assert_eq!(kept, vec![3, 4, 6]);
    }

    #[test]
    fn each_set_of_scanned_paths_has_a_folder_of_its_own() {
        let base = scratch("projects");
        let (app, web) = (base.join("app"), base.join("web"));
        std::fs::create_dir_all(&app).unwrap();
        std::fs::create_dir_all(&web).unwrap();
        let root = base.join("cache");
        let both = project_dir(&root, &[app.clone(), web.clone()]);
        assert_eq!(both, project_dir(&root, &[web.clone(), app.clone()]));
        let one = project_dir(&root, std::slice::from_ref(&app));
        assert_ne!(both, one, "a subfolder scan gets a folder of its own");
        let inner = app.join("src");
        std::fs::create_dir_all(&inner).unwrap();
        assert_eq!(
            project_dir(&root, &[inner.clone(), app.clone()]),
            one,
            "a path inside another adds nothing"
        );
        assert!(both.starts_with(root.join("embeddings")));
        let name = one.file_name().unwrap().to_string_lossy().into_owned();
        assert!(name.starts_with("app-"), "{name}");
    }

    #[test]
    fn garbage_reads_as_an_empty_cache() {
        let path = scratch("garbage").join("m.bin");
        std::fs::write(&path, b"not a cache").unwrap();
        assert!(load(&path).vectors.is_empty());
        assert!(load(&path.with_extension("missing")).vectors.is_empty());
    }
}
