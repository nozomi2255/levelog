use std::{
    collections::HashSet,
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};

use crate::dto::CodexPathCandidateDto;

const SYSTEM_CANDIDATES: [(&str, &str, bool); 3] = [
    (
        "/Applications/Codex.app/Contents/Resources/codex",
        "codex_app",
        true,
    ),
    ("/opt/homebrew/bin/codex", "homebrew", true),
    ("/usr/local/bin/codex", "usr_local", false),
];

/// Validates an explicitly supplied Codex path without invoking a process.
pub fn canonical_executable(path: &Path) -> Result<PathBuf, String> {
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
    Ok(canonical)
}

/// Searches only the fixed, documented paths. It never consults PATH, `which`, or a shell and
/// deliberately does not probe the discovered executable.
pub fn discover(configured: Option<&str>) -> Vec<CodexPathCandidateDto> {
    let mut paths: Vec<(PathBuf, String, bool)> = Vec::new();
    if let Some(path) = configured.filter(|path| !path.trim().is_empty()) {
        paths.push((PathBuf::from(path), "configured".into(), true));
    }
    paths.extend(
        SYSTEM_CANDIDATES
            .into_iter()
            .map(|(path, source, recommended)| (PathBuf::from(path), source.into(), recommended)),
    );
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        paths.extend([
            (home.join(".local/bin/codex"), "local_bin".into(), false),
            (home.join(".volta/bin/codex"), "volta".into(), false),
            (
                home.join(".codex/packages/standalone/current/bin/codex"),
                "codex_standalone".into(),
                true,
            ),
        ]);
    }

    let mut seen = HashSet::new();
    paths
        .into_iter()
        .filter_map(|(discovered, source, recommended)| {
            let canonical = canonical_executable(&discovered).ok()?;
            if !seen.insert(canonical.clone()) {
                return None;
            }
            Some(CodexPathCandidateDto {
                discovered_path: discovered.to_string_lossy().into_owned(),
                canonical_path: canonical.to_string_lossy().into_owned(),
                source,
                executable: true,
                recommended,
                connection: None,
            })
        })
        .collect()
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
        assert!(configured.connection.is_none());
    }
}
