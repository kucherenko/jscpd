// The embedders `--semantic` makes, through the public API: the local model
// must be downloaded first, a download is verified, and an embeddings API's
// vectors are cached between runs. Every server is on 127.0.0.1.

#[path = "../src/embed/test_server.rs"]
#[allow(dead_code)]
mod test_server;

use cpd_semantic::embed::models::{JINA_V2_BASE_CODE, LocalModel};
use cpd_semantic::{Provider, SemanticOptions, download, embedder, missing_model, model_list};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use test_server::{Reply, Server};

/// The cache directory of this test binary, set once before any test reads
/// it.
fn cache_root() -> &'static Path {
    static ROOT: OnceLock<PathBuf> = OnceLock::new();
    ROOT.get_or_init(|| {
        let root = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("jscpd-embedders-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        // SAFETY: set once, under the OnceLock, before any test of this
        // binary reads the environment.
        unsafe { std::env::set_var("JSCPD_CACHE_DIR", &root) };
        root
    })
}

fn local(model: &str) -> SemanticOptions {
    cache_root();
    SemanticOptions {
        model: model.into(),
        ..SemanticOptions::default()
    }
}

/// `model` as a download leaves it, with files of the pinned sizes (sparse,
/// and not the real weights) and the stamp a verified download writes.
fn pretend_downloaded(model: &LocalModel) -> PathBuf {
    let dir = model.dir(cache_root());
    std::fs::create_dir_all(&dir).unwrap();
    let mut stamp = format!("{} {}\n", model.id, model.revision);
    for file in model.files {
        let f = std::fs::File::create(dir.join(file.name)).unwrap();
        f.set_len(file.size).unwrap();
        stamp.push_str(&format!("{}  {}\n", file.sha256, file.name));
    }
    std::fs::write(dir.join(".verified"), stamp).unwrap();
    dir
}

#[test]
fn a_scan_with_a_local_model_not_downloaded_says_how_to_download_it() {
    let default = local("CodeRankEmbed");
    let err = embedder(&default, &[], true).err().unwrap();
    assert!(
        err.contains("nomic-ai/CodeRankEmbed is not downloaded yet"),
        "{err}"
    );
    assert!(
        err.contains("Run `jscpd --semantic-download` once (548 MB into"),
        "{err}"
    );
    let (id, size) = missing_model(&default).unwrap();
    assert_eq!(id, "nomic-ai/CodeRankEmbed");
    assert_eq!(size, 546_938_168 + 711_649 + 1_525);

    let api = SemanticOptions {
        provider: Provider::Http,
        ..default
    };
    assert_eq!(missing_model(&api), None, "an API needs no download");
    assert_eq!(
        missing_model(&local("qwen3-embedding:0.6b")),
        None,
        "nor does a model jscpd cannot run"
    );
}

#[test]
fn a_download_whose_checksum_does_not_match_leaves_the_model_missing() {
    // The first file has its pinned size and other bytes.
    let server = Server::start(vec![Reply::status(200, &"x".repeat(1_525))]);
    // SAFETY: no other test of this binary reads HF_ENDPOINT.
    unsafe { std::env::set_var("HF_ENDPOINT", &server.url) };
    let options = local("CodeRankEmbed");
    let err = download(&options, true).unwrap_err();
    let requests = server.requests();
    assert_eq!(requests.len(), 1, "it stops at the first bad file");
    assert_eq!(
        requests[0].path,
        "/nomic-ai/CodeRankEmbed/resolve/3c4b60807d71f79b43f3c4363786d9493691f8b1/config.json"
    );
    assert!(err.contains("got 1525 bytes with SHA-256"), "{err}");
    assert!(missing_model(&options).is_some(), "still missing");
    let err = embedder(&options, &[], true).err().unwrap();
    assert!(err.contains("not downloaded yet"), "{err}");
    let dir = cpd_semantic::embed::models::CODERANKEMBED.dir(cache_root());
    let left: Vec<_> = std::fs::read_dir(&dir).unwrap().collect();
    assert!(left.is_empty(), "no file kept: {left:?}");

    let err = download(&local("some/other-model"), true).unwrap_err();
    assert!(err.contains("not 'some/other-model'"), "{err}");
}

#[test]
fn a_model_in_place_is_used_without_downloading_and_listed_as_downloaded() {
    let dir = pretend_downloaded(&JINA_V2_BASE_CODE);
    let options = local("jina-embeddings-v2-base-code");
    assert_eq!(missing_model(&options), None);
    assert_eq!(download(&options, true).unwrap(), dir, "nothing to fetch");

    let listing = model_list();
    let row = |name: &str| {
        listing
            .lines()
            .find(|l| l.starts_with(name))
            .unwrap_or_else(|| panic!("no {name} in\n{listing}"))
            .to_string()
    };
    assert!(listing.starts_with("MODEL"), "{listing}");
    assert!(row("jina-embeddings-v2-base-code").ends_with("in jscpd, 324 MB, downloaded"));
    assert!(row("CodeRankEmbed (default)").ends_with("in jscpd, 548 MB"));
    assert!(listing.contains("API (Ollama:"), "{listing}");
    assert!(listing.contains("CROSS is the default --semantic-threshold"));

    // The files are not real weights: the embedder is made, and embedding
    // reports the broken model instead of panicking.
    let embedder = embedder(&options, &[], true).unwrap();
    let err = embedder.embed(&["fn a() {}"]).unwrap_err();
    assert!(
        err.contains("tokenizer.json") || err.contains("config.json"),
        "{err}"
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn an_embeddings_api_is_asked_once_and_its_vectors_cached_between_runs() {
    let project = cache_root().join("project");
    std::fs::create_dir_all(&project).unwrap();
    let answer = || {
        Reply::json(
            200,
            r#"{"data":[{"index":0,"embedding":[1.0,0.0]},{"index":1,"embedding":[0.0,1.0]}]}"#,
        )
    };
    let server = Server::start(vec![answer(), answer()]);
    let options = SemanticOptions {
        provider: Provider::Http,
        url: format!("{}/v1", server.url),
        model: "embedder".into(),
        on_command_line: true,
        ..SemanticOptions::default()
    };
    let scanned = [project];
    let texts = ["fn a() {}", "fn b() {}"];
    let first = embedder(&options, &scanned, true)
        .unwrap()
        .embed(&texts)
        .unwrap();
    assert_eq!(first, [vec![1.0, 0.0], vec![0.0, 1.0]]);
    // A later run over the same paths reads the cache.
    let again = embedder(&options, &scanned, true)
        .unwrap()
        .embed(&texts)
        .unwrap();
    assert_eq!(again, first);
    assert_eq!(server.requests_so_far().len(), 1);
    // Rebuilding the cache asks again.
    let rebuild = SemanticOptions {
        rebuild_cache: true,
        ..options.clone()
    };
    let rebuilt = embedder(&rebuild, &scanned, true)
        .unwrap()
        .embed(&texts)
        .unwrap();
    assert_eq!(rebuilt, first);
    let requests = server.requests();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[1].json()["input"], serde_json::json!(texts));
}
