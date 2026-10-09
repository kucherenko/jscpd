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

/// The network a model's weights belong to, which decides the code that
/// runs it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Architecture {
    /// JinaBERT v2: ALiBi attention, the mean of the token states.
    JinaBert,
    /// NomicBERT: rotary attention, the first token's state.
    NomicBert,
}

/// A model the local provider can run.
#[derive(Debug)]
pub struct LocalModel {
    /// Hugging Face repository id.
    pub id: &'static str,
    pub revision: &'static str,
    pub files: &'static [ModelFile],
    /// Longest input in tokens; a longer function is embedded by its head.
    pub max_tokens: usize,
    pub architecture: Architecture,
}

pub const CODERANKEMBED: LocalModel = LocalModel {
    id: "nomic-ai/CodeRankEmbed",
    revision: "3c4b60807d71f79b43f3c4363786d9493691f8b1",
    files: &[
        ModelFile {
            name: "config.json",
            size: 1_525,
            sha256: "5ff856a41d0f53ef2d74520627d464bd75c2efd8f26f381bd528654895c29b6c",
        },
        ModelFile {
            name: "tokenizer.json",
            size: 711_649,
            sha256: "91f1def9b9391fdabe028cd3f3fcc4efd34e5d1f08c3bf2de513ebb5911a1854",
        },
        ModelFile {
            name: "model.safetensors",
            size: 546_938_168,
            sha256: "827529bcd58aef0d9082e66eeff7e7d53a02f62bd005f841a26b3d3e2fb17ebe",
        },
    ],
    max_tokens: 1024,
    architecture: Architecture::NomicBert,
};

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
    architecture: Architecture::JinaBert,
};

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
        if self.is_stamped(dir) {
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

    /// Whether every file in `dir` has its pinned size and the directory
    /// carries this model's stamp. Unlike [`Self::is_downloaded`], it never
    /// hashes a file or writes a stamp, so it is cheap enough for a listing.
    pub fn is_stamped(&self, dir: &Path) -> bool {
        self.files
            .iter()
            .all(|f| has_size(&dir.join(f.name), f.size))
            && std::fs::read_to_string(dir.join(STAMP)).is_ok_and(|stamp| stamp == self.stamp())
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
    fn a_model_lives_in_a_folder_of_its_revision() {
        let model = &CODERANKEMBED;
        assert_eq!(model.size(), 1_525 + 711_649 + 546_938_168);
        let dir = model.dir(Path::new("/cache"));
        assert_eq!(
            dir,
            Path::new("/cache/models/nomic-ai--CodeRankEmbed")
                .join("3c4b60807d71f79b43f3c4363786d9493691f8b1")
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
        architecture: Architecture::JinaBert,
    };

    use crate::embed::test_dir;

    #[test]
    fn a_file_counts_once_its_checksum_matched_not_its_size() {
        let dir = test_dir("models-verify");
        std::fs::write(dir.join("hello.txt"), "HELLO").unwrap();
        assert!(!HELLO.is_stamped(&dir), "the right size, no stamp");
        assert!(
            !HELLO.is_downloaded(&dir),
            "the right size with the wrong bytes"
        );
        assert!(!dir.join(STAMP).exists());

        std::fs::write(dir.join("hello.txt"), "hello").unwrap();
        assert!(!HELLO.is_stamped(&dir), "not hashed yet");
        assert!(!dir.join(STAMP).exists(), "and nothing written");
        assert!(HELLO.is_downloaded(&dir), "hashed once");
        assert!(HELLO.is_stamped(&dir));
        assert_eq!(
            std::fs::read_to_string(dir.join(STAMP)).unwrap(),
            HELLO.stamp(),
            "and stamped, so later runs read the stamp instead of hashing"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    use crate::embed::test_server::{Reply, Server};

    const HELLO_PATH: &str = "/test/hello/resolve/r1/hello.txt";

    /// The names of the files in `dir`.
    fn listing(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    /// Fetch HELLO's one file from a server giving `reply`, into a fresh
    /// directory.
    fn fetch_hello(name: &str, reply: Reply) -> (Result<(), String>, PathBuf) {
        let dir = test_dir(name);
        let server = Server::start(vec![reply]);
        let url = format!("{}{HELLO_PATH}", server.url);
        let result = fetch(
            &crate::embed::http::agent(),
            &url,
            &HELLO.files[0],
            &dir.join("hello.txt"),
            true,
        );
        server.requests();
        (result, dir)
    }

    #[test]
    fn a_fetched_file_lands_whole_once_its_checksum_matched() {
        let (result, dir) = fetch_hello("models-fetch-ok", Reply::status(200, "hello"));
        result.unwrap();
        assert_eq!(std::fs::read(dir.join("hello.txt")).unwrap(), b"hello");
        assert_eq!(listing(&dir), ["hello.txt"], "no scratch file left");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_file_with_the_wrong_checksum_is_rejected_and_nothing_is_kept() {
        let (result, dir) = fetch_hello("models-fetch-sha", Reply::status(200, "HELLO"));
        let err = result.unwrap_err();
        assert!(err.contains("got 5 bytes with SHA-256"), "{err}");
        assert!(
            err.contains(&format!("expected 5 bytes with {}", HELLO.files[0].sha256)),
            "{err}"
        );
        assert!(listing(&dir).is_empty(), "{:?}", listing(&dir));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_file_too_long_or_too_short_is_rejected_and_nothing_is_kept() {
        for (name, body) in [
            ("models-fetch-long", "hello world"),
            ("models-fetch-short", "hell"),
        ] {
            let (result, dir) = fetch_hello(name, Reply::status(200, body));
            assert!(result.is_err(), "{body}");
            assert!(listing(&dir).is_empty(), "{body}: {:?}", listing(&dir));
            std::fs::remove_dir_all(&dir).unwrap();
        }
    }

    #[test]
    fn an_interrupted_download_is_rejected_and_nothing_is_kept() {
        // The server promises five bytes and hangs up after three.
        let (result, dir) = fetch_hello(
            "models-fetch-cut",
            Reply::Raw(b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\nhel".to_vec()),
        );
        let err = result.unwrap_err();
        assert!(err.contains(HELLO_PATH), "the error names the file: {err}");
        assert!(listing(&dir).is_empty(), "{:?}", listing(&dir));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_status_other_than_200_is_an_error_naming_the_url() {
        for status in [403, 404, 500] {
            let (result, dir) = fetch_hello("models-fetch-status", Reply::status(status, "hello"));
            let err = result.unwrap_err();
            assert!(
                err.ends_with(&format!("{HELLO_PATH}: HTTP {status}")),
                "{err}"
            );
            assert!(listing(&dir).is_empty());
            std::fs::remove_dir_all(&dir).unwrap();
        }
    }

    #[test]
    fn a_redirect_to_the_file_store_is_followed() {
        // Hugging Face answers with a redirect to its CDN.
        let dir = test_dir("models-fetch-redirect");
        let server = Server::start(vec![
            Reply::status(302, "").with_header("Location", "/cdn/hello.txt"),
            Reply::status(200, "hello"),
        ]);
        let url = format!("{}{HELLO_PATH}", server.url);
        let agent = crate::embed::http::agent();
        let result = fetch(&agent, &url, &HELLO.files[0], &dir.join("hello.txt"), true);
        let paths: Vec<String> = server.requests().into_iter().map(|r| r.path).collect();
        result.unwrap();
        assert_eq!(paths, [HELLO_PATH, "/cdn/hello.txt"]);
        assert_eq!(std::fs::read(dir.join("hello.txt")).unwrap(), b"hello");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_download_fetches_what_is_missing_and_stamps_the_folder() {
        let server = Server::start(vec![Reply::status(200, "hello")]);
        // SAFETY: no other test of this crate reads HF_ENDPOINT or calls
        // into C code that reads the environment.
        unsafe { std::env::set_var("HF_ENDPOINT", format!("{}/", server.url)) };
        let dir = test_dir("models-download").join("hello");
        let agent = crate::embed::http::agent();
        let result = HELLO.download(&dir, &agent, true);
        let requests = server.requests();
        result.unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].method, "GET");
        assert_eq!(requests[0].path, HELLO_PATH, "the pinned revision");
        assert!(HELLO.is_stamped(&dir));
        assert!(HELLO.is_downloaded(&dir));
        // Everything is there: a second download asks the server for
        // nothing (it is gone, so any request would fail).
        HELLO.download(&dir, &agent, true).unwrap();
        std::fs::remove_dir_all(dir.parent().unwrap()).unwrap();
    }

    #[test]
    fn writing_atomically_replaces_the_file_and_leaves_nothing_else() {
        let dir = test_dir("models-atomic");
        let target = dir.join("model.safetensors");
        write_atomically(&target, b"old").unwrap();
        write_atomically(&target, b"whole").unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"whole");
        assert_eq!(listing(&dir), ["model.safetensors"], "no leftovers");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn sizes_read_in_kilobytes_or_megabytes() {
        assert_eq!(megabytes(1_525), "2 KB");
        assert_eq!(megabytes(546_938_168), "547 MB");
    }
}
