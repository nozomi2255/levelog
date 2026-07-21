use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};

use crate::dto::CodexPathCandidateDto;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedExecutable {
    /// The user-selected absolute path. Keep this for execution because CLI shims may depend on
    /// their invocation name (`argv[0]`).
    pub launch_path: PathBuf,
    /// The resolved regular executable, used only to validate and deduplicate candidates.
    pub canonical_path: PathBuf,
}

const SYSTEM_CANDIDATES: [(&str, &str, bool); 3] = [
    ("/opt/homebrew/bin/codex", "homebrew", true),
    (
        "/Applications/Codex.app/Contents/Resources/codex",
        "codex_app",
        false,
    ),
    ("/usr/local/bin/codex", "usr_local", false),
];

/// Validates an explicitly supplied Codex path without invoking a process. The launch path is
/// deliberately not canonicalized: replacing a shim path with its target can change CLI behavior.
pub fn validate_executable(path: &Path) -> Result<ValidatedExecutable, String> {
    if !path.is_absolute() {
        return Err("Codex CLIのパスは絶対パスで指定してください".into());
    }
    let canonical = fs::canonicalize(path)
        .map_err(|error| format!("Codex CLIのパスを解決できません: {error}"))?;
    let metadata =
        fs::metadata(&canonical).map_err(|error| format!("Codex CLIを確認できません: {error}"))?;
    if !metadata.is_file() {
        return Err("Codex CLIのパスは通常ファイルではありません".into());
    }
    if metadata.permissions().mode() & 0o111 == 0 {
        return Err("Codex CLIに実行権限がありません".into());
    }
    Ok(ValidatedExecutable {
        launch_path: path.to_path_buf(),
        canonical_path: canonical,
    })
}

/// Returns the resolved target for callers that need an identity for comparison only. Never use
/// this result to launch the CLI; use [`validate_executable`]'s `launch_path` instead.
pub fn canonical_executable(path: &Path) -> Result<PathBuf, String> {
    Ok(validate_executable(path)?.canonical_path)
}

/// Searches only the fixed, documented paths. It never consults PATH, `which`, or a shell and
/// deliberately does not probe the discovered executable.
pub fn discover(configured: Option<&str>) -> Vec<CodexPathCandidateDto> {
    let mut paths: Vec<(PathBuf, String, bool)> = Vec::new();
    if let Some(path) = configured.filter(|path| !path.trim().is_empty()) {
        // Keep a known-good saved connection selected. An older release could have saved a
        // resolved shim target; a later launch alias for the same target replaces it below.
        paths.push((PathBuf::from(path), "configured".into(), true));
    }
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        paths.extend([
            (
                home.join(".codex/packages/standalone/current/bin/codex"),
                "codex_standalone".into(),
                true,
            ),
            (home.join(".local/bin/codex"), "local_bin".into(), false),
            (home.join(".volta/bin/codex"), "volta".into(), true),
        ]);
    }
    paths.extend(
        SYSTEM_CANDIDATES
            .into_iter()
            .map(|(path, source, recommended)| (PathBuf::from(path), source.into(), recommended)),
    );
    candidates_from_paths(paths)
}

fn candidates_from_paths(paths: Vec<(PathBuf, String, bool)>) -> Vec<CodexPathCandidateDto> {
    let mut candidates = Vec::new();
    for (discovered, source, recommended) in paths {
        let Ok(executable) = validate_executable(&discovered) else {
            continue;
        };
        let candidate = CodexPathCandidateDto {
            discovered_path: discovered.to_string_lossy().into_owned(),
            launch_path: executable.launch_path.to_string_lossy().into_owned(),
            canonical_path: executable.canonical_path.to_string_lossy().into_owned(),
            source,
            executable: true,
            recommended,
            connection: None,
        };
        if let Some(index) = candidates
            .iter()
            .position(|existing: &CodexPathCandidateDto| {
                existing.canonical_path == candidate.canonical_path
            })
        {
            // Prefer a launch alias over its resolved target. Volta and similar shims can
            // inspect argv[0], so launching the target is not behavior-preserving.
            if path_is_symlink(&candidate.launch_path)
                && !path_is_symlink(&candidates[index].launch_path)
            {
                candidates[index] = candidate;
            }
            continue;
        }
        candidates.push(candidate);
    }
    candidates
}

fn path_is_symlink(path: &str) -> bool {
    fs::symlink_metadata(path)
        .map(|metadata| metadata.file_type().is_symlink())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{os::unix::fs::PermissionsExt, os::unix::fs::symlink};

    #[test]
    fn canonicalizes_executable_symlinks_and_rejects_non_executable_files() {
        let directory = tempfile::tempdir().unwrap();
        let executable = directory.path().join("codex-real");
        fs::write(&executable, "test").unwrap();
        let mut permissions = fs::metadata(&executable).unwrap().permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&executable, permissions).unwrap();
        let link = directory.path().join("codex");
        symlink(&executable, &link).unwrap();
        assert_eq!(
            canonical_executable(&link).unwrap(),
            fs::canonicalize(&executable).unwrap()
        );

        let plain = directory.path().join("plain");
        fs::write(&plain, "test").unwrap();
        let mut permissions = fs::metadata(&plain).unwrap().permissions();
        permissions.set_mode(0o600);
        fs::set_permissions(&plain, permissions).unwrap();
        assert!(canonical_executable(&plain).is_err());
        assert!(canonical_executable(directory.path()).is_err());
    }

    #[test]
    fn configured_candidate_is_deduplicated_after_canonicalization() {
        let directory = tempfile::tempdir().unwrap();
        let executable = directory.path().join("codex");
        fs::write(&executable, "test").unwrap();
        let mut permissions = fs::metadata(&executable).unwrap().permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&executable, permissions).unwrap();
        let candidates = discover(Some(executable.to_str().unwrap()));
        let configured = candidates
            .iter()
            .find(|candidate| candidate.source == "configured")
            .unwrap();
        assert_eq!(
            configured.canonical_path,
            fs::canonicalize(&executable).unwrap().to_string_lossy()
        );
        assert_eq!(configured.launch_path, executable.to_string_lossy());
        assert!(configured.connection.is_none());
    }

    #[test]
    fn configured_resolved_target_does_not_hide_a_safe_shim_launch_alias() {
        let directory = tempfile::tempdir().unwrap();
        let target = directory.path().join("volta-shim");
        fs::write(&target, "test").unwrap();
        let mut permissions = fs::metadata(&target).unwrap().permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&target, permissions).unwrap();
        let launch = directory.path().join("bin/codex");
        fs::create_dir_all(launch.parent().unwrap()).unwrap();
        symlink(&target, &launch).unwrap();

        assert_eq!(validate_executable(&launch).unwrap().launch_path, launch);

        let candidates = candidates_from_paths(vec![
            (target.clone(), "configured".into(), false),
            (launch.clone(), "volta".into(), true),
        ]);
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].launch_path, launch.to_string_lossy());
        assert_eq!(candidates[0].source, "volta");
    }

    #[tokio::test]
    async fn preserves_symlink_launch_path_for_argv_zero_sensitive_shims() {
        use crate::infrastructure::codex::{ProcessRequest, ProcessRunner, TokioProcessRunner};
        use tokio::sync::watch;

        let directory = tempfile::tempdir().unwrap();
        let target = directory.path().join("volta-shim");
        fs::write(
            &target,
            "#!/bin/sh\ncase \"$0\" in *volta-shim) exit 126 ;; *) exit 0 ;; esac\n",
        )
        .unwrap();
        let mut permissions = fs::metadata(&target).unwrap().permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&target, permissions).unwrap();
        let launch = directory.path().join("bin/codex");
        fs::create_dir_all(launch.parent().unwrap()).unwrap();
        symlink(&target, &launch).unwrap();

        let validated = validate_executable(&launch).unwrap();
        assert_eq!(validated.launch_path, launch);
        assert_eq!(validated.canonical_path, fs::canonicalize(&target).unwrap());
        let (_sender, receiver) = watch::channel(false);
        let runner = TokioProcessRunner;
        let output = runner
            .run(
                ProcessRequest {
                    program: validated.launch_path,
                    args: vec![],
                    cwd: directory.path().to_path_buf(),
                    stdin: None,
                },
                receiver,
            )
            .await
            .unwrap();
        assert_eq!(output.status, 0);
    }
}
