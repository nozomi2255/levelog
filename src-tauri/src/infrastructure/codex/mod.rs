//! The fail-closed boundary around the local Codex CLI.
//!
//! Nothing outside this module receives arbitrary command execution.  A caller can only ask for
//! a typed analysis or proposal after the installed CLI proves that every required safety switch
//! is available.

use async_trait::async_trait;
use serde::de::DeserializeOwned;
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    time::Duration,
};
use tempfile::TempDir;
use thiserror::Error;
use tokio::{io::AsyncWriteExt, process::Command, sync::watch, time::timeout};

use crate::dto::{ActivityAnalysisOutput, QuestProposalOutput};

pub const TIMEOUT: Duration = Duration::from_secs(180);
pub const REQUIRED_FEATURES: [&str; 5] = [
    "shell_tool",
    "unified_exec",
    "browser_use",
    "computer_use",
    "in_app_browser",
];
pub const ACTIVITY_SCHEMA_VERSION: &str = "activity-analysis.v1";
pub const QUEST_SCHEMA_VERSION: &str = "quest-proposal.v1";

const SKILL_IDS: [&str; 15] = [
    "thinking.information_structuring",
    "thinking.problem_decomposition",
    "thinking.hypothesis_testing",
    "technical.technical_learning",
    "technical.system_design",
    "technical.validation",
    "communication.clarification",
    "communication.explanation",
    "communication.documentation",
    "execution.prioritization",
    "execution.planning",
    "execution.follow_through",
    "interpersonal.listening",
    "interpersonal.alignment",
    "interpersonal.feedback",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessRequest {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub stdin: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessOutput {
    pub status: i32,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum CodexError {
    #[error("Codex CLI was not found at {0}")]
    NotFound(String),
    #[error("Codex CLI path must be absolute: {0}")]
    RelativePath(String),
    #[error("Codex CLI is not logged in")]
    NotLoggedIn,
    #[error("installed Codex CLI lacks required safety control: {0}")]
    Incompatible(String),
    #[error("Codex process timed out after 180 seconds")]
    TimedOut,
    #[error("Codex process was cancelled")]
    Cancelled,
    #[error("Codex process failed: {0}")]
    Process(String),
    #[error("Codex returned invalid JSON: {0}")]
    InvalidJson(String),
    #[error("Codex returned output that violates {0}: {1}")]
    SchemaViolation(&'static str, String),
}

#[async_trait]
pub trait ProcessRunner: Send + Sync {
    async fn run(
        &self,
        request: ProcessRequest,
        cancel: watch::Receiver<bool>,
    ) -> Result<ProcessOutput, CodexError>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct TokioProcessRunner;

#[async_trait]
impl ProcessRunner for TokioProcessRunner {
    async fn run(
        &self,
        request: ProcessRequest,
        mut cancel: watch::Receiver<bool>,
    ) -> Result<ProcessOutput, CodexError> {
        if *cancel.borrow() {
            return Err(CodexError::Cancelled);
        }
        let mut command = Command::new(&request.program);
        command
            .args(&request.args)
            .current_dir(&request.cwd)
            .kill_on_drop(true)
            .stdin(if request.stdin.is_some() {
                std::process::Stdio::piped()
            } else {
                std::process::Stdio::null()
            })
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        let mut child = command
            .spawn()
            .map_err(|err| CodexError::Process(err.to_string()))?;
        if let Some(input) = request.stdin {
            let mut stdin = child
                .stdin
                .take()
                .ok_or_else(|| CodexError::Process("unable to open stdin".into()))?;
            stdin
                .write_all(input.as_bytes())
                .await
                .map_err(|err| CodexError::Process(err.to_string()))?;
        }
        let wait = child.wait_with_output();
        tokio::pin!(wait);
        let output = tokio::select! {
            result = timeout(TIMEOUT, &mut wait) => result
                .map_err(|_| CodexError::TimedOut)?
                .map_err(|err| CodexError::Process(err.to_string()))?,
            changed = cancel.changed() => { changed.map_err(|_| CodexError::Cancelled)?; return Err(CodexError::Cancelled); }
        };
        Ok(ProcessOutput {
            status: output.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodexConnection {
    pub path: PathBuf,
    pub version: String,
}

pub struct CodexClient<R> {
    runner: R,
    path: PathBuf,
}
impl<R: ProcessRunner> CodexClient<R> {
    pub fn new(path: PathBuf, runner: R) -> Result<Self, CodexError> {
        if !path.is_absolute() {
            return Err(CodexError::RelativePath(path.display().to_string()));
        }
        Ok(Self { runner, path })
    }

    pub async fn probe(&self) -> Result<CodexConnection, CodexError> {
        if !self.path.is_file() {
            return Err(CodexError::NotFound(self.path.display().to_string()));
        }
        let dir = empty_dir()?;
        let version = self.run_args(vec!["--version".into()], &dir, None).await?;
        require_success(&version)?;
        let login = self
            .run_args(vec!["login".into(), "status".into()], &dir, None)
            .await?;
        if login.status != 0 || !login.stdout.to_lowercase().contains("logged in") {
            return Err(CodexError::NotLoggedIn);
        }
        let help = self
            .run_args(vec!["exec".into(), "--help".into()], &dir, None)
            .await?;
        require_success(&help)?;
        for flag in [
            "--ephemeral",
            "--sandbox",
            "--ignore-user-config",
            "--ignore-rules",
            "--skip-git-repo-check",
            "--disable",
            "--output-schema",
            "--output-last-message",
        ] {
            if !help.stdout.contains(flag) {
                return Err(CodexError::Incompatible(flag.into()));
            }
        }
        let features = self
            .run_args(vec!["features".into(), "list".into()], &dir, None)
            .await?;
        require_success(&features)?;
        let available: BTreeSet<&str> = features
            .stdout
            .lines()
            .filter_map(|line| line.split_whitespace().next())
            .collect();
        for feature in REQUIRED_FEATURES {
            if !available.contains(feature) {
                return Err(CodexError::Incompatible(feature.into()));
            }
        }
        Ok(CodexConnection {
            path: self.path.clone(),
            version: version.stdout.trim().to_owned(),
        })
    }

    pub async fn analyze_activity(
        &self,
        payload: String,
        cancel: watch::Receiver<bool>,
    ) -> Result<ActivityAnalysisOutput, CodexError> {
        let result = self
            .exec_json(
                ACTIVITY_SCHEMA_VERSION,
                activity_schema(),
                include_str!("../../../prompts/activity-analysis.v1.md"),
                payload,
                cancel,
            )
            .await?;
        let output: ActivityAnalysisOutput = decode(result, ACTIVITY_SCHEMA_VERSION)?;
        if output.summary.trim().is_empty()
            || output.skill_candidates.iter().any(|candidate| {
                !SKILL_IDS.contains(&candidate.skill_id.as_str())
                    || !(0.0..=1.0).contains(&candidate.confidence)
                    || candidate.reason.trim().is_empty()
                    || candidate.evidence.trim().is_empty()
            })
        {
            return Err(CodexError::SchemaViolation(
                ACTIVITY_SCHEMA_VERSION,
                "unknown skill or invalid candidate".into(),
            ));
        }
        Ok(output)
    }

    pub async fn propose_quest(
        &self,
        payload: String,
        cancel: watch::Receiver<bool>,
    ) -> Result<QuestProposalOutput, CodexError> {
        let result = self
            .exec_json(
                QUEST_SCHEMA_VERSION,
                quest_schema(),
                include_str!("../../../prompts/quest-proposal.v1.md"),
                payload,
                cancel,
            )
            .await?;
        let output: QuestProposalOutput = decode(result, QUEST_SCHEMA_VERSION)?;
        const TEMPLATES: [&str; 5] = [
            "clarify_once",
            "summarize_decision",
            "explain_simply",
            "plan_next_step",
            "review_evidence",
        ];
        if !TEMPLATES.contains(&output.template_id.as_str())
            || !SKILL_IDS.contains(&output.target_skill_id.as_str())
            || !(5..=30).contains(&output.estimated_minutes)
            || !(1..=5).contains(&output.difficulty)
            || output.success_criteria.is_empty()
            || output.success_criteria.iter().any(|x| x.trim().is_empty())
        {
            return Err(CodexError::SchemaViolation(
                QUEST_SCHEMA_VERSION,
                "invalid safe quest fields".into(),
            ));
        }
        Ok(output)
    }

    async fn exec_json(
        &self,
        schema_version: &'static str,
        schema: PathBuf,
        prompt: &str,
        payload: String,
        cancel: watch::Receiver<bool>,
    ) -> Result<String, CodexError> {
        self.probe().await?;
        let dir = empty_dir()?;
        let output = dir.path().join("result.json");
        let mut args = vec![
            "exec".into(),
            "--ephemeral".into(),
            "--sandbox".into(),
            "read-only".into(),
            "--ignore-user-config".into(),
            "--ignore-rules".into(),
            "--skip-git-repo-check".into(),
            "-c".into(),
            "web_search=\"disabled\"".into(),
        ];
        for feature in REQUIRED_FEATURES {
            args.extend(["--disable".into(), feature.into()]);
        }
        args.extend([
            "--output-schema".into(),
            schema.display().to_string(),
            "--output-last-message".into(),
            output.display().to_string(),
            prompt.into(),
        ]);
        let response = self
            .runner
            .run(
                ProcessRequest {
                    program: self.path.clone(),
                    args,
                    cwd: dir.path().to_path_buf(),
                    stdin: Some(payload),
                },
                cancel,
            )
            .await?;
        require_success(&response)?;
        tokio::fs::read_to_string(&output)
            .await
            .map_err(|err| CodexError::InvalidJson(format!("{schema_version}: {err}")))
    }

    async fn run_args(
        &self,
        args: Vec<String>,
        dir: &TempDir,
        stdin: Option<String>,
    ) -> Result<ProcessOutput, CodexError> {
        let (_cancel_sender, cancel_receiver) = watch::channel(false);
        self.runner
            .run(
                ProcessRequest {
                    program: self.path.clone(),
                    args,
                    cwd: dir.path().to_path_buf(),
                    stdin,
                },
                cancel_receiver,
            )
            .await
    }
}

fn empty_dir() -> Result<TempDir, CodexError> {
    tempfile::Builder::new()
        .prefix("levelog-codex-")
        .tempdir()
        .map_err(|err| CodexError::Process(err.to_string()))
}
fn require_success(output: &ProcessOutput) -> Result<(), CodexError> {
    if output.status == 0 {
        Ok(())
    } else {
        Err(CodexError::Process(if output.stderr.trim().is_empty() {
            output.stdout.clone()
        } else {
            output.stderr.clone()
        }))
    }
}
fn decode<T: DeserializeOwned>(json: String, schema: &'static str) -> Result<T, CodexError> {
    let value: serde_json::Value = serde_json::from_str(&json)
        .map_err(|err| CodexError::InvalidJson(format!("{schema}: {err}")))?;
    serde_json::from_value(value)
        .map_err(|err| CodexError::SchemaViolation(schema, err.to_string()))
}
fn activity_schema() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("schemas/activity-analysis.v1.schema.json")
}
fn quest_schema() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("schemas/quest-proposal.v1.schema.json")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        collections::VecDeque,
        sync::{Arc, Mutex},
    };
    #[derive(Clone)]
    struct FakeRunner {
        replies: Arc<Mutex<VecDeque<Result<ProcessOutput, CodexError>>>>,
        calls: Arc<Mutex<Vec<ProcessRequest>>>,
    }
    impl FakeRunner {
        fn new(replies: Vec<Result<ProcessOutput, CodexError>>) -> Self {
            Self {
                replies: Arc::new(Mutex::new(replies.into())),
                calls: Arc::new(Mutex::new(vec![])),
            }
        }
    }
    #[async_trait]
    impl ProcessRunner for FakeRunner {
        async fn run(
            &self,
            request: ProcessRequest,
            _: watch::Receiver<bool>,
        ) -> Result<ProcessOutput, CodexError> {
            self.calls.lock().unwrap().push(request);
            self.replies.lock().unwrap().pop_front().unwrap()
        }
    }
    fn ok(text: &str) -> Result<ProcessOutput, CodexError> {
        Ok(ProcessOutput {
            status: 0,
            stdout: text.into(),
            stderr: String::new(),
        })
    }
    fn fake_path() -> PathBuf {
        std::env::current_exe().unwrap()
    }
    fn compatible_replies() -> Vec<Result<ProcessOutput, CodexError>> {
        vec![
            ok("codex-cli 0.144.1"),
            ok("Logged in using ChatGPT"),
            ok(
                "--ephemeral --sandbox --ignore-user-config --ignore-rules --skip-git-repo-check --disable --output-schema --output-last-message",
            ),
            ok(
                "shell_tool stable true\nunified_exec stable true\nbrowser_use stable true\ncomputer_use stable true\nin_app_browser stable true",
            ),
        ]
    }
    #[tokio::test]
    async fn probe_rejects_missing_cli() {
        let client =
            CodexClient::new(PathBuf::from("/not/a/codex"), FakeRunner::new(vec![])).unwrap();
        assert!(matches!(client.probe().await, Err(CodexError::NotFound(_))));
    }
    #[tokio::test]
    async fn probe_rejects_logged_out_cli() {
        let client = CodexClient::new(
            fake_path(),
            FakeRunner::new(vec![ok("codex-cli"), ok("not authenticated")]),
        )
        .unwrap();
        assert_eq!(client.probe().await.unwrap_err(), CodexError::NotLoggedIn);
    }
    #[tokio::test]
    async fn probe_rejects_missing_safety_feature() {
        let mut replies = compatible_replies();
        replies[3] = ok("shell_tool stable true");
        let client = CodexClient::new(fake_path(), FakeRunner::new(replies)).unwrap();
        assert!(
            matches!(client.probe().await, Err(CodexError::Incompatible(feature)) if feature == "unified_exec")
        );
    }
    #[tokio::test]
    async fn fake_runner_surfaces_timeout_and_cancel() {
        let client = CodexClient::new(
            fake_path(),
            FakeRunner::new(vec![Err(CodexError::TimedOut)]),
        )
        .unwrap();
        let dir = empty_dir().unwrap();
        assert_eq!(
            client
                .run_args(vec!["--version".into()], &dir, None)
                .await
                .unwrap_err(),
            CodexError::TimedOut
        );
        let (tx, rx) = watch::channel(false);
        tx.send(true).unwrap();
        let runner = TokioProcessRunner;
        let request = ProcessRequest {
            program: fake_path(),
            args: vec!["--version".into()],
            cwd: dir.path().to_path_buf(),
            stdin: None,
        };
        assert_eq!(
            runner.run(request, rx).await.unwrap_err(),
            CodexError::Cancelled
        );
    }
    #[test]
    fn schema_decode_rejects_non_json_and_schema_mismatch() {
        assert!(matches!(
            decode::<ActivityAnalysisOutput>("not json".into(), ACTIVITY_SCHEMA_VERSION),
            Err(CodexError::InvalidJson(_))
        ));
        assert!(matches!(
            decode::<ActivityAnalysisOutput>("{}".into(), ACTIVITY_SCHEMA_VERSION),
            Err(CodexError::SchemaViolation(_, _))
        ));
    }

    #[tokio::test]
    #[ignore = "contacts the configured real Codex service; run only with an explicit command"]
    async fn real_codex_smoke_test() {
        let path = std::env::var("LEVELOG_CODEX_PATH")
            .expect("set LEVELOG_CODEX_PATH to the absolute Codex CLI path");
        let client = CodexClient::new(PathBuf::from(path), TokioProcessRunner).unwrap();
        let (_cancel_sender, cancel_receiver) = watch::channel(false);
        let output = client
            .analyze_activity(
                serde_json::json!({
                    "activity": {
                        "occurredOn": "2026-07-20",
                        "whatIDid": "公開ドキュメントの要点を三つに整理した",
                        "whatWasDifficult": "重複する説明をまとめる必要があった",
                        "whatChanged": "次に読む人が判断しやすくなった"
                    },
                    "profile": { "role": "テスト", "focusSkillIds": ["thinking.information_structuring"] }
                })
                .to_string(),
                cancel_receiver,
            )
            .await
            .unwrap();
        assert!(!output.summary.trim().is_empty());
    }
}
