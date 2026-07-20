//! Local-only source intake. Paths are obtained by the native dialog, never from a webview.
use std::{
    fs,
    path::{Path, PathBuf},
};
pub const MAX_FILES: usize = 100;
pub const MAX_FILE_BYTES: u64 = 1024 * 1024;
pub const MAX_BATCH_BYTES: u64 = 10 * 1024 * 1024;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SafeSource {
    pub kind: String,
    pub display_name: String,
    pub original_path: PathBuf,
    pub content: String,
}
pub fn validate_file(path: &Path, batch_bytes: u64) -> Result<SafeSource, String> {
    let metadata = fs::symlink_metadata(path).map_err(|e| format!("読み込めません: {e}"))?;
    if metadata.file_type().is_symlink() {
        return Err("シンボリックリンクは取り込めません".into());
    }
    if !metadata.file_type().is_file() {
        return Err("通常ファイルだけを取り込めます".into());
    }
    let extension = path
        .extension()
        .and_then(|x| x.to_str())
        .map(str::to_ascii_lowercase);
    if !matches!(extension.as_deref(), Some("md" | "markdown" | "txt")) {
        return Err(".md、.markdown、.txt だけを取り込めます".into());
    }
    if metadata.len() > MAX_FILE_BYTES {
        return Err("ファイルは 1 MiB 以下にしてください".into());
    }
    if batch_bytes.saturating_add(metadata.len()) > MAX_BATCH_BYTES {
        return Err("合計 10 MiB を超えます".into());
    }
    let bytes = fs::read(path).map_err(|e| format!("読み込めません: {e}"))?;
    let after = fs::symlink_metadata(path).map_err(|e| format!("読み込めません: {e}"))?;
    if after.file_type().is_symlink()
        || !after.file_type().is_file()
        || after.len() != metadata.len()
        || bytes.len() as u64 != metadata.len()
    {
        return Err("読み込み中にファイルが変更されました。もう一度選択してください".into());
    }
    if bytes.contains(&0) {
        return Err("NUL を含むテキストは取り込めません".into());
    }
    let content =
        String::from_utf8(bytes).map_err(|_| "UTF-8 テキストだけを取り込めます".to_string())?;
    let display_name = path
        .file_name()
        .and_then(|x| x.to_str())
        .ok_or_else(|| "表示名を取得できません".to_string())?
        .to_owned();
    let kind = if matches!(extension.as_deref(), Some("md" | "markdown")) {
        "markdown"
    } else {
        "text"
    };
    Ok(SafeSource {
        kind: kind.into(),
        display_name,
        original_path: path.to_path_buf(),
        content,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_nul_and_wrong_extension() {
        let dir = tempfile::tempdir().unwrap();
        let wrong = dir.path().join("note.pdf");
        std::fs::write(&wrong, "x").unwrap();
        assert!(validate_file(&wrong, 0).is_err());
        let nul = dir.path().join("note.md");
        std::fs::write(&nul, b"a\0b").unwrap();
        assert!(validate_file(&nul, 0).is_err());
    }
}
