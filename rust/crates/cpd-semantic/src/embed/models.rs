// models.rs — embedding models jscpd can run itself, and their download.
//
// Every file is pinned by repository revision, size and SHA-256, so what
// runs is exactly what was calibrated. Files come from Hugging Face (or the
// mirror in `HF_ENDPOINT`) into the jscpd cache directory, once, on
// `--semantic-download`; a scan never downloads anything on its own.

use std::io::{IsTerminal, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// Written into the model directory once every file's SHA-256 matched.
const STAMP: &str = ".verified";

/// A file of a model repository.
#[derive(Debug)]
pub struct ModelFile {
    pub name: &'static str,
    pub size: u64,
    pub sha256: &'static str,
}

/// A model the local provider can run.
#[derive(Debug)]
pub struct LocalModel {
    /// Hugging Face repository id, also the `--semantic-model` value.
    pub id: &'static str,
    pub revision: &'static str,
    pub files: &'static [ModelFile],
    /// Longest input in tokens; a longer function is embedded by its head.
    pub max_tokens: usize,
}

pub const JINA_V2_BASE_CODE: LocalModel = LocalModel {
    id: "jinaai/jina-embeddings-v2-base-code",
    revision: "516f4baf13dec4ddddda8631e019b5737c8bc250",
    files: &[
        ModelFile {
            name: "config.json",
            size: 1_216,
            sha256: "e426aa684c7f9a95c5f020aa855faf93a24f065f5fad0c9e17b124670cabdea6",
        },
        ModelFile {
            name: "tokenizer.json",
            size: 2_561_316,
            sha256: "b01c78a902aa4facb2f47f95449f48e2f7bbfea5d2472ee2f6ce92323c6f86e5",
        },
        ModelFile {
            name: "model.safetensors",
            size: 321_767_312,
            sha256: "8b53bfd4ae2cd586004a6ca4a16551b630a2a1b1d655ff1ee9be1286a1781c5b",
        },
    ],
    max_tokens: 1024,
};

pub static MODELS: &[&LocalModel] = &[&JINA_V2_BASE_CODE];

/// The local model called `id`.
pub fn find(id: &str) -> Option<&'static LocalModel> {
    MODELS.iter().copied().find(|m| m.id == id)
}

/// Every model id the local provider knows, for messages.
pub fn names() -> String {
    MODELS.iter().map(|m| m.id).collect::<Vec<_>>().join(", ")
}

impl LocalModel {
    /// Where the files live: `<cache>/models/<owner>--<name>/<revision>`.
    pub fn dir(&self, cache_root: &Path) -> PathBuf {
        cache_root
            .join("models")
            .join(self.id.replace('/', "--"))
            .join(self.revision)
    }

    /// Total download size in bytes.
    pub fn size(&self) -> u64 {
        self.files.iter().map(|f| f.size).sum()
    }

    /// Whether every file is in `dir`, verified. A download stamps the
    /// directory once every file's SHA-256 matched; files that have their
    /// pinned sizes but no stamp (copied in by hand, or left by an earlier
    /// build) are hashed once here, and stamped when they match. A size
    /// alone proves nothing: an interrupted or overlapping download can
    /// leave a file of the right size with the wrong bytes.
    pub fn is_downloaded(&self, dir: &Path) -> bool {
        if !self
            .files
            .iter()
            .all(|f| has_size(&dir.join(f.name), f.size))
        {
            return false;
        }
        if std::fs::read_to_string(dir.join(STAMP)).is_ok_and(|stamp| stamp == self.stamp()) {
            return true;
        }
        let verified = self
            .files
            .iter()
            .all(|f| file_matches(&dir.join(f.name), f));
        if verified {
            let _ = write_atomically(&dir.join(STAMP), self.stamp().as_bytes());
        }
        verified
    }

    /// What the stamp says: the model, its revision and every checksum, so a
    /// stamp from another revision does not count.
    fn stamp(&self) -> String {
        let mut stamp = format!("{} {}\n", self.id, self.revision);
        for file in self.files {
            stamp.push_str(&format!("{}  {}\n", file.sha256, file.name));
        }
        stamp
    }

    /// Download the files missing from `dir`, checking size and checksum.
    pub fn download(&self, dir: &Path, agent: &ureq::Agent, quiet: bool) -> Result<(), String> {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let endpoint = std::env::var("HF_ENDPOINT")
            .ok()
            .filter(|e| !e.is_empty())
            .unwrap_or_else(|| "https://huggingface.co".to_string());
        let endpoint = endpoint.trim_end_matches('/');
        if !quiet {
            eprintln!(
                "Downloading {} ({}) from {endpoint} into {}",
                self.id,
                megabytes(self.size()),
                dir.display()
            );
        }
        for file in self.files {
            let target = dir.join(file.name);
            if file_matches(&target, file) {
                continue;
            }
            let url = format!(
                "{endpoint}/{}/resolve/{}/{}",
                self.id, self.revision, file.name
            );
            fetch(agent, &url, file, &target, quiet)?;
        }
        write_atomically(&dir.join(STAMP), self.stamp().as_bytes())
    }
}

fn has_size(path: &Path, size: u64) -> bool {
    std::fs::metadata(path).is_ok_and(|m| m.len() == size)
}

/// Whether the file at `path` is `file`: its pinned size and SHA-256.
fn file_matches(path: &Path, file: &ModelFile) -> bool {
    has_size(path, file.size) && sha256_of(path).is_ok_and(|digest| digest == file.sha256)
}

fn sha256_of(path: &Path) -> std::io::Result<String> {
    use sha2::Digest;
    let mut input = std::fs::File::open(path)?;
    let mut hasher = sha2::Sha256::new();
    let mut buffer = vec![0u8; 1 << 20];
    loop {
        let n = input.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }
    Ok(hex(&hasher.finalize()))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// A temporary file next to `target` that is this process's alone: two
/// jscpd processes downloading into one cache (parallel CI jobs, containers
/// sharing a volume) never write into each other's file.
fn scratch_path(target: &Path) -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.subsec_nanos());
    let name = target
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    target.with_file_name(format!(
        "{name}.{}-{nanos}-{}.partial",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ))
}

/// A scratch file, removed when dropped unless it was moved into place.
struct Scratch {
    path: PathBuf,
    kept: bool,
}

impl Drop for Scratch {
    fn drop(&mut self) {
        if !self.kept {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

/// Write `bytes` to `target` through a scratch file, so a reader sees the
/// old content or the new, never a part.
fn write_atomically(target: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut scratch = Scratch {
        path: scratch_path(target),
        kept: false,
    };
    std::fs::write(&scratch.path, bytes).map_err(|e| format!("{}: {e}", scratch.path.display()))?;
    std::fs::rename(&scratch.path, target).map_err(|e| format!("{}: {e}", target.display()))?;
    scratch.kept = true;
    Ok(())
}

fn megabytes(bytes: u64) -> String {
    match bytes {
        0..1_000_000 => format!("{:.0} KB", bytes as f64 / 1_000.0),
        _ => format!("{:.0} MB", bytes as f64 / 1_000_000.0),
    }
}

/// Stream `url` into `target`, through a scratch file of this process that
/// becomes the target only once its size and SHA-256 match, and is removed
/// on any error.
fn fetch(
    agent: &ureq::Agent,
    url: &str,
    file: &ModelFile,
    target: &Path,
    quiet: bool,
) -> Result<(), String> {
    use sha2::Digest;
    let mut response = agent.get(url).call().map_err(|e| format!("{url}: {e}"))?;
    let status = response.status().as_u16();
    if status != 200 {
        return Err(format!("{url}: HTTP {status}"));
    }
    let mut scratch = Scratch {
        path: scratch_path(target),
        kept: false,
    };
    let partial = scratch.path.clone();
    let mut out = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&partial)
        .map_err(|e| format!("{}: {e}", partial.display()))?;
    let mut reader = response
        .body_mut()
        .with_config()
        .limit(file.size + 1)
        .reader();
    let mut hasher = sha2::Sha256::new();
    let mut buffer = vec![0u8; 1 << 20];
    let (mut done, mut shown) = (0u64, 0u64);
    let live = !quiet && std::io::stderr().is_terminal();
    loop {
        let n = reader
            .read(&mut buffer)
            .map_err(|e| format!("{url}: {e}"))?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
        out.write_all(&buffer[..n])
            .map_err(|e| format!("{}: {e}", partial.display()))?;
        done += n as u64;
        if live && (done - shown > file.size / 100 || done == file.size) {
            shown = done;
            eprint!("\r  {} {:>3}%", file.name, done * 100 / file.size.max(1));
        }
    }
    if live {
        eprintln!();
    }
    out.sync_all()
        .map_err(|e| format!("{}: {e}", partial.display()))?;
    drop(out);
    let digest = hex(&hasher.finalize());
    if done != file.size || digest != file.sha256 {
        return Err(format!(
            "{url}: got {done} bytes with SHA-256 {digest}, expected {} bytes with {}",
            file.size, file.sha256
        ));
    }
    match std::fs::rename(&partial, target) {
        Ok(()) => scratch.kept = true,
        // Another jscpd put the same file there first and still holds it
        // open (Windows refuses to replace an open file): theirs will do.
        Err(_) if file_matches(target, file) => {}
        Err(e) => return Err(format!("{}: {e}", target.display())),
    }
    if !quiet {
        eprintln!("  {} {} verified", file.name, megabytes(file.size));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_catalog_knows_the_default_model() {
        let model = find("jinaai/jina-embeddings-v2-base-code").unwrap();
        assert_eq!(model.size(), 1_216 + 2_561_316 + 321_767_312);
        assert!(find("unclemusclez/jina-embeddings-v2-base-code").is_none());
        let dir = model.dir(Path::new("/cache"));
        assert_eq!(
            dir,
            Path::new("/cache/models/jinaai--jina-embeddings-v2-base-code")
                .join("516f4baf13dec4ddddda8631e019b5737c8bc250")
        );
        assert!(!model.is_downloaded(&dir));
    }

    static HELLO: LocalModel = LocalModel {
        id: "test/hello",
        revision: "r1",
        files: &[ModelFile {
            name: "hello.txt",
            size: 5,
            sha256: "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824",
        }],
        max_tokens: 8,
    };

    use crate::embed::test_dir;

    #[test]
    fn a_file_counts_once_its_checksum_matched_not_its_size() {
        let dir = test_dir("models-verify");
        std::fs::write(dir.join("hello.txt"), "HELLO").unwrap();
        assert!(
            !HELLO.is_downloaded(&dir),
            "the right size with the wrong bytes"
        );
        assert!(!dir.join(STAMP).exists());

        std::fs::write(dir.join("hello.txt"), "hello").unwrap();
        assert!(HELLO.is_downloaded(&dir), "hashed once");
        assert_eq!(
            std::fs::read_to_string(dir.join(STAMP)).unwrap(),
            HELLO.stamp(),
            "and stamped, so later runs read the stamp instead of hashing"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn scratch_files_are_unique_and_removed_unless_kept() {
        let dir = test_dir("models-scratch");
        let target = dir.join("model.safetensors");
        let (a, b) = (scratch_path(&target), scratch_path(&target));
        assert_ne!(a, b, "two downloads never share a file");
        {
            let scratch = Scratch {
                path: a.clone(),
                kept: false,
            };
            std::fs::write(&scratch.path, "part").unwrap();
        }
        assert!(!a.exists(), "an abandoned scratch file is removed");
        write_atomically(&target, b"whole").unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"whole");
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1, "no leftovers");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
