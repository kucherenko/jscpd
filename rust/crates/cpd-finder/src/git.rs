//! Running the `git` binary against a repository chosen by path.
//!
//! Git exports repository-local variables (`GIT_DIR`, `GIT_INDEX_FILE`, ...)
//! to the hooks it runs. A child `git` started from inside a hook would
//! inherit them and act on the hook's repository and index instead of the one
//! named by `-C`: `git worktree add` would overwrite the index of the commit
//! in progress. Every git child process goes through [`command`], which drops
//! those variables, the same set git itself clears before entering a submodule
//! (`git rev-parse --local-env-vars`).

use std::path::Path;
use std::process::Command;

/// Variables that point git at a repository, index or configuration other
/// than the one at the working directory.
const LOCAL_ENV_VARS: &[&str] = &[
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    "GIT_CONFIG",
    "GIT_CONFIG_PARAMETERS",
    "GIT_CONFIG_COUNT",
    "GIT_OBJECT_DIRECTORY",
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_IMPLICIT_WORK_TREE",
    "GIT_GRAFT_FILE",
    "GIT_INDEX_FILE",
    "GIT_NO_REPLACE_OBJECTS",
    "GIT_REPLACE_REF_BASE",
    "GIT_PREFIX",
    "GIT_INTERNAL_SUPER_PREFIX",
    "GIT_SHALLOW_FILE",
    "GIT_COMMON_DIR",
];

/// A `git -C <repo_root>` command that ignores repository-local variables
/// inherited from the environment.
pub fn command(repo_root: &Path) -> Command {
    let mut cmd = Command::new("git");
    for var in LOCAL_ENV_VARS {
        cmd.env_remove(var);
    }
    cmd.arg("-C").arg(repo_root);
    cmd
}

/// Whether `value` would be read by git as an option when passed as a
/// revision, range or path argument.
pub fn looks_like_option(value: &str) -> bool {
    value.starts_with('-')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_command_does_not_pass_on_a_hooks_repository_variables() {
        let cmd = command(Path::new("/repo"));
        let removed: Vec<_> = cmd
            .get_envs()
            .filter(|(_, value)| value.is_none())
            .map(|(name, _)| name.to_string_lossy().into_owned())
            .collect();
        for var in [
            "GIT_DIR",
            "GIT_INDEX_FILE",
            "GIT_WORK_TREE",
            "GIT_COMMON_DIR",
        ] {
            assert!(removed.iter().any(|r| r == var), "{var} must be cleared");
        }
        let args: Vec<_> = cmd
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert_eq!(args, ["-C", "/repo"]);
    }

    #[test]
    fn only_leading_dashes_look_like_options() {
        assert!(looks_like_option("--output=/tmp/x"));
        assert!(looks_like_option("-p"));
        assert!(!looks_like_option("main"));
        assert!(!looks_like_option("v1.0..HEAD"));
        assert!(!looks_like_option("feature/-x"));
    }
}
