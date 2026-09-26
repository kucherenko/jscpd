// models.rs — embedding models jscpd can run itself, and their download.
//
// Every file is pinned by repository revision, size and SHA-256, so what
// runs is exactly what was calibrated. Files come from Hugging Face (or the
// mirror in `HF_ENDPOINT`) into the jscpd cache directory, once, on
// `--semantic-download`; a scan never downloads anything on its own.

use std::io::{IsTerminal, Read, Write};
use std::path::{Path, PathBuf};

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

    /// Whether every file is in `dir` with its pinned size. The checksum is
    /// verified once, when the file is downloaded.
    pub fn is_downloaded(&self, dir: &Path) -> bool {
        self.files
            .iter()
            .all(|f| std::fs::metadata(dir.join(f.name)).is_ok_and(|m| m.len() == f.size))
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
            if std::fs::metadata(&target).is_ok_and(|m| m.len() == file.size) {
                continue;
            }
            let url = format!(
                "{endpoint}/{}/resolve/{}/{}",
                self.id, self.revision, file.name
            );
            fetch(agent, &url, file, &target, quiet)?;
        }
        Ok(())
    }
}

fn megabytes(bytes: u64) -> String {
    match bytes {
        0..1_000_000 => format!("{:.0} KB", bytes as f64 / 1_000.0),
        _ => format!("{:.0} MB", bytes as f64 / 1_000_000.0),
    }
}

/// Stream `url` into `target`, through a `.partial` file that becomes the
/// target only once its size and SHA-256 match.
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
    let partial = target.with_extension("partial");
    let mut out =
        std::fs::File::create(&partial).map_err(|e| format!("{}: {e}", partial.display()))?;
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
    drop(out);
    let digest: String = hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    if done != file.size || digest != file.sha256 {
        let _ = std::fs::remove_file(&partial);
        return Err(format!(
            "{url}: got {done} bytes with SHA-256 {digest}, expected {} bytes with {}",
            file.size, file.sha256
        ));
    }
    std::fs::rename(&partial, target).map_err(|e| format!("{}: {e}", target.display()))?;
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
}
