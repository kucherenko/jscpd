//! The `basta` binary as a user runs it: what it prints, what it writes and
//! the exit code it ends with.

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

struct Dir(PathBuf);

impl Dir {
    fn new(files: &[(&str, &str)]) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let root = std::env::temp_dir().join(format!(
            "basta-binary-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::remove_dir_all(&root).ok();
        std::fs::create_dir_all(&root).unwrap();
        for (path, text) in files {
            let full = root.join(path);
            std::fs::create_dir_all(full.parent().unwrap()).unwrap();
            std::fs::write(full, text).unwrap();
        }
        Self(root)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).ok();
    }
}

fn basta(args: &[&str], stdin: &str) -> Output {
    use std::io::Write;
    let mut child = Command::new(env!("CARGO_BIN_EXE_basta"))
        .args(args)
        .env("NO_COLOR", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the basta binary runs");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(stdin.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

const DEAD: &[(&str, &str)] = &[
    ("src/index.ts", "import { used } from './lib';\nused();\n"),
    (
        "src/lib.ts",
        "export function used() {}\nexport function neverImported() {}\n",
    ),
];

#[test]
fn list_prints_the_supported_formats() {
    let out = basta(&["--list"], "");
    assert!(out.status.success());
    let stdout = text(&out.stdout);
    for format in ["typescript", "python", "vue"] {
        assert!(stdout.lines().any(|l| l == format), "{stdout}");
    }
}

#[test]
fn list_frameworks_and_debug_describe_without_scanning() {
    let project = Dir::new(DEAD);
    let root = project.path().to_str().unwrap();
    let out = basta(&[root, "--list-frameworks"], "");
    assert!(out.status.success(), "{}", text(&out.stderr));
    assert!(text(&out.stdout).contains("next"), "{}", text(&out.stdout));

    let out = basta(&[root, "--debug", "--categories", "exports"], "");
    assert!(out.status.success());
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).expect("JSON");
    assert_eq!(json["categories"], serde_json::json!(["unused-export"]));
}

#[test]
fn a_bad_argument_stops_the_run_with_exit_code_one() {
    let out = basta(&["/definitely/not/a/basta/path"], "");
    assert_eq!(out.status.code(), Some(1));
    assert!(
        text(&out.stderr).contains("Error:"),
        "{}",
        text(&out.stderr)
    );
}

#[test]
fn warnings_are_printed_and_the_run_goes_on() {
    let project = Dir::new(DEAD);
    let root = project.path().to_str().unwrap();
    let out = basta(
        &[root, "--threshold", "150", "--max-size", "lots", "--silent"],
        "",
    );
    let stderr = text(&out.stderr);
    assert!(stderr.contains("Warning: --threshold"), "{stderr}");
    assert!(stderr.contains("Warning: --max-size"), "{stderr}");
    assert_eq!(out.status.code(), Some(0), "{stderr}");
}

#[test]
fn a_run_reports_findings_writes_files_and_sets_the_exit_code() {
    let project = Dir::new(DEAD);
    let root = project.path().to_str().unwrap();
    let report_dir = project.path().join("out");
    let out = basta(
        &[
            root,
            "--ignore",
            "**/out/**",
            "-r",
            "console,json",
            "-o",
            report_dir.to_str().unwrap(),
            "--exit-code",
            "3",
        ],
        "",
    );
    assert_eq!(out.status.code(), Some(3), "{}", text(&out.stderr));
    assert!(
        text(&out.stdout).contains("neverImported"),
        "{}",
        text(&out.stdout)
    );
    let written: Vec<String> = std::fs::read_dir(&report_dir)
        .expect("the json reporter writes into -o")
        .map(|e| std::fs::read_to_string(e.unwrap().path()).unwrap())
        .collect();
    assert!(
        written.iter().any(|t| t.contains("neverImported")),
        "{written:?}"
    );

    let clean = Dir::new(&[("src/index.ts", "console.log(1);\n")]);
    let out = basta(&[clean.path().to_str().unwrap(), "--exit-code", "3"], "");
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stdout));
}

#[test]
fn rust_diagnostics_can_come_from_stdin() {
    let project = Dir::new(&[("src/lib.rs", "fn unused() {}\n")]);
    let root = project.path().to_str().unwrap();
    let message = serde_json::json!({
        "reason": "compiler-message",
        "message": {
            "code": { "code": "dead_code" },
            "level": "warning",
            "message": "function `unused` is never used",
            "spans": [{
                "file_name": "src/lib.rs",
                "line_start": 1, "line_end": 1,
                "column_start": 4, "column_end": 10,
                "is_primary": true
            }]
        }
    });
    let out = basta(
        &[root, "--rust-diagnostics", "-", "-r", "console"],
        &format!("{message}\n"),
    );
    assert!(
        text(&out.stdout).contains("unused"),
        "stdout: {}\nstderr: {}",
        text(&out.stdout),
        text(&out.stderr)
    );
}
