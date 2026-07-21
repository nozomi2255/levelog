//! The fail-closed boundary around the local Codex CLI.
//!
//! Nothing outside this module receives arbitrary command execution.  A caller can only ask for
//! a typed analysis or proposal after the installed CLI proves that every required safety switch
//! is available.

pub mod discovery;

use async_trait::async_trait;
use serde::de::DeserializeOwned;
use std::{collections::BTreeSet, path::PathBuf, time::Duration};
use tempfile::TempDir;
use thiserror::Error;
use tokio::{io::AsyncWriteExt, process::Command, sync::watch, time::timeout};

use crate::dto::{ActivityAnalysisOutput, EvidenceExtractionOutput, QuestProposalOutput};

pub const TIMEOUT: Duration = Duration::from_secs(180);
pub const REQUIRED_FEATURES: [&str; 5] = [
    "shell_tool",
    "unified_exec",
    "browser_use",
    "computer_use",
    "in_app_browser",
];
pub const ACTIVITY_SCHEMA_VERSION: &str = "activity-analysis.v2";
pub const QUEST_SCHEMA_VERSION: &str = "quest-proposal.v1";
pub const EVIDENCE_SCHEMA_VERSION: &str = "evidence-extraction.v1";
const MAX_CODEX_PAYLOAD_BYTES: usize = 512 * 1024;
const MAX_CODEX_OUTPUT_BYTES: u64 = 1024 * 1024;

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

#[derive(Debug, Clone)]
pub struct CodexJsonOutput<T> {
    pub raw_json: String,
    pub parsed: T,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum CodexError {
    #[error("指定した場所にCodex CLIが見つかりません: {0}")]
    NotFound(String),
    #[error("Codex CLIは絶対パスで指定してください: {0}")]
    RelativePath(String),
    #[error("Codex CLIにログインしていません。ターミナルで `codex login` を実行してください")]
    NotLoggedIn,
    #[error(
        "Codex CLIのログイン状態を確認できませんでした（終了コード: {status}）。接続テストをもう一度実行してください"
    )]
    LoginStatusProbeFailed { status: i32 },
    #[error("インストール済みCodex CLIは必要な安全機能に対応していません: {0}")]
    Incompatible(String),
    #[error("Codex処理が180秒でタイムアウトしました")]
    TimedOut,
    #[error("Codex処理をキャンセルしました")]
    Cancelled,
    #[error("Codex処理に失敗しました: {0}")]
    Process(String),
    #[error("Codexから正しいJSONが返りませんでした: {0}")]
    InvalidJson(String),
    #[error("Codex出力が{0}の形式に適合しません: {1}")]
    SchemaViolation(&'static str, String),
    #[error("Codexから正しいJSONが返りませんでした: {message}")]
    InvalidJsonOutput { message: String, raw_json: String },
    #[error("Codex出力が{schema}の形式に適合しません: {message}")]
    SchemaViolationOutput {
        schema: &'static str,
        message: String,
        raw_json: String,
    },
}

impl CodexError {
    pub fn is_schema_retryable(&self) -> bool {
        matches!(
            self,
            Self::InvalidJson(_)
                | Self::SchemaViolation(_, _)
                | Self::InvalidJsonOutput { .. }
                | Self::SchemaViolationOutput { .. }
        )
    }

    pub fn raw_output(&self) -> Option<&str> {
        match self {
            Self::InvalidJsonOutput { raw_json, .. }
            | Self::SchemaViolationOutput { raw_json, .. } => Some(raw_json),
            _ => None,
        }
    }
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
        require_authenticated_login_status(&login)?;
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
    ) -> Result<CodexJsonOutput<ActivityAnalysisOutput>, CodexError> {
        let raw_json = self
            .exec_json(
                ACTIVITY_SCHEMA_VERSION,
                activity_schema(),
                include_str!("../../../prompts/activity-analysis.v2.md"),
                payload,
                cancel,
            )
            .await?;
        let output: ActivityAnalysisOutput = decode(raw_json.clone(), ACTIVITY_SCHEMA_VERSION)?;
        if output.summary.trim().is_empty()
            || output.summary.chars().count() > 1_000
            || output.outcomes.len() > 20
            || output
                .outcomes
                .iter()
                .any(|outcome| outcome.trim().is_empty() || outcome.chars().count() > 500)
            || output.confirmed_facts.len() > 20
            || output
                .confirmed_facts
                .iter()
                .any(|fact| fact.trim().is_empty() || fact.chars().count() > 500)
            || output.unconfirmed_facts.len() > 20
            || output
                .unconfirmed_facts
                .iter()
                .any(|fact| fact.trim().is_empty() || fact.chars().count() > 500)
            || output.skill_candidates.len() > 15
            || output.skill_candidates.iter().any(|candidate| {
                !SKILL_IDS.contains(&candidate.skill_id.as_str())
                    || !(0.0..=1.0).contains(&candidate.confidence)
                    || candidate.reason.trim().is_empty()
                    || candidate.reason.chars().count() > 1_000
                    || candidate.evidence.trim().is_empty()
                    || candidate.evidence.chars().count() > 1_000
                    || candidate
                        .specialized_skill_name
                        .as_ref()
                        .is_some_and(|name| name.trim().is_empty() || name.chars().count() > 80)
            })
            || output.next_question.as_ref().is_some_and(|question| {
                !matches!(
                    question.target.as_str(),
                    "context"
                        | "autonomy"
                        | "outcome"
                        | "difficulty"
                        | "scope"
                        | "support"
                        | "repeatability"
                        | "measurement"
                ) || !matches!(
                    question.answer_type.as_str(),
                    "single_choice" | "text" | "number"
                ) || !valid_question_id(&question.question_id)
                    || question.text.trim().is_empty()
                    || question.text.chars().count() > 500
                    || question.why_it_matters.trim().is_empty()
                    || question.why_it_matters.chars().count() > 500
                    || question.choices.len() > 5
                    || (question.answer_type == "single_choice" && question.choices.is_empty())
                    || (question.answer_type != "single_choice" && !question.choices.is_empty())
                    || question.choices.iter().any(|choice| {
                        choice.value.trim().is_empty()
                            || choice.value.chars().count() > 80
                            || choice.label.trim().is_empty()
                            || choice.label.chars().count() > 120
                    })
                    || question
                        .choices
                        .iter()
                        .map(|choice| choice.value.as_str())
                        .collect::<BTreeSet<_>>()
                        .len()
                        != question.choices.len()
            })
        {
            return Err(CodexError::SchemaViolationOutput {
                schema: ACTIVITY_SCHEMA_VERSION,
                message: "unknown skill or invalid candidate".into(),
                raw_json,
            });
        }
        Ok(CodexJsonOutput {
            raw_json,
            parsed: output,
        })
    }

    pub async fn propose_quest(
        &self,
        payload: String,
        cancel: watch::Receiver<bool>,
    ) -> Result<CodexJsonOutput<QuestProposalOutput>, CodexError> {
        let raw_json = self
            .exec_json(
                QUEST_SCHEMA_VERSION,
                quest_schema(),
                include_str!("../../../prompts/quest-proposal.v1.md"),
                payload,
                cancel,
            )
            .await?;
        let output: QuestProposalOutput = decode(raw_json.clone(), QUEST_SCHEMA_VERSION)?;
        const TEMPLATES: [&str; 5] = [
            "clarify_once",
            "summarize_decision",
            "explain_simply",
            "plan_next_step",
            "review_evidence",
        ];
        if !TEMPLATES.contains(&output.template_id.as_str())
            || !SKILL_IDS.contains(&output.target_skill_id.as_str())
            || output.title.trim().is_empty()
            || output.title.chars().count() > 120
            || output.description.trim().is_empty()
            || output.description.chars().count() > 1_000
            || !(5..=30).contains(&output.estimated_minutes)
            || !(1..=5).contains(&output.difficulty)
            || output.success_criteria.is_empty()
            || output.success_criteria.len() > 5
            || output
                .success_criteria
                .iter()
                .any(|x| x.trim().is_empty() || x.chars().count() > 240)
            || output.evidence_prompt.trim().is_empty()
            || output.evidence_prompt.chars().count() > 500
        {
            return Err(CodexError::SchemaViolationOutput {
                schema: QUEST_SCHEMA_VERSION,
                message: "invalid safe quest fields".into(),
                raw_json,
            });
        }
        Ok(CodexJsonOutput {
            raw_json,
            parsed: output,
        })
    }

    pub async fn analyze_evidence(
        &self,
        payload: String,
        cancel: watch::Receiver<bool>,
    ) -> Result<CodexJsonOutput<EvidenceExtractionOutput>, CodexError> {
        let raw_json = self
            .exec_json(
                EVIDENCE_SCHEMA_VERSION,
                evidence_schema(),
                include_str!("../../../prompts/evidence-extraction.v1.md"),
                payload,
                cancel,
            )
            .await?;
        let output: EvidenceExtractionOutput = decode(raw_json.clone(), EVIDENCE_SCHEMA_VERSION)?;
        if output.candidates.len() > 50
            || output.candidates.iter().any(|c| {
                !matches!(
                    c.kind.as_str(),
                    "fact"
                        | "experience"
                        | "achievement"
                        | "outcome"
                        | "decision"
                        | "lesson"
                        | "knowledge"
                        | "idea"
                        | "project"
                        | "interest"
                        | "personality_signal"
                        | "inference"
                ) || !matches!(c.provenance.as_str(), "import_extracted" | "ai_inference")
                    || !(0.0..=1.0).contains(&c.confidence)
                    || c.statement.trim().is_empty()
                    || c.statement.chars().count() > 1_000
                    || c.source_excerpt.trim().is_empty()
                    || c.source_excerpt.chars().count() > 2_000
                    || matches!(
                        (&c.start_byte, &c.end_byte),
                        (Some(_), None) | (None, Some(_))
                    )
                    || c.canonical_skill_id
                        .as_ref()
                        .is_some_and(|id| !SKILL_IDS.contains(&id.as_str()))
                    || c.project_hint
                        .as_ref()
                        .is_some_and(|hint| hint.trim().is_empty() || hint.chars().count() > 160)
            })
        {
            return Err(CodexError::SchemaViolationOutput {
                schema: EVIDENCE_SCHEMA_VERSION,
                message: "invalid evidence candidate".into(),
                raw_json,
            });
        }
        Ok(CodexJsonOutput {
            raw_json,
            parsed: output,
        })
    }

    async fn exec_json(
        &self,
        schema_version: &'static str,
        schema_document: &'static str,
        prompt: &str,
        payload: String,
        cancel: watch::Receiver<bool>,
    ) -> Result<String, CodexError> {
        if payload.len() > MAX_CODEX_PAYLOAD_BYTES {
            return Err(CodexError::Process(format!(
                "送信内容が大きすぎます（上限 {} KiB）",
                MAX_CODEX_PAYLOAD_BYTES / 1024
            )));
        }
        self.probe().await?;
        let dir = empty_dir()?;
        let schema = dir.path().join(format!("{schema_version}.schema.json"));
        tokio::fs::write(&schema, schema_document)
            .await
            .map_err(|err| CodexError::Process(format!("schemaを書き出せません: {err}")))?;
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
        let metadata = tokio::fs::metadata(&output)
            .await
            .map_err(|err| CodexError::InvalidJson(format!("{schema_version}: {err}")))?;
        if metadata.len() > MAX_CODEX_OUTPUT_BYTES {
            return Err(CodexError::InvalidJson(format!(
                "{schema_version}: 出力が大きすぎます"
            )));
        }
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

fn valid_question_id(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    value.len() <= 64
        && (first.is_ascii_lowercase() || first.is_ascii_digit())
        && chars.all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || character == '_'
                || character == '-'
        })
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

/// `codex login status` is a CLI status command, so exit code zero is its authentication-success
/// contract. Human-readable output differs between packaged builds and may be written to stderr.
/// Known explicit logout messages override a zero exit; every other nonzero status is a diagnostic
/// probe failure rather than a claim that the user is logged out. Do not surface the output here:
/// stderr can contain environment details that must not reach the UI.
fn require_authenticated_login_status(output: &ProcessOutput) -> Result<(), CodexError> {
    let status_text = format!("{}\n{}", output.stdout, output.stderr).to_ascii_lowercase();
    let negative_markers = [
        "not logged in",
        "not authenticated",
        "logged out",
        "unauthenticated",
    ];
    if negative_markers
        .iter()
        .any(|marker| status_text.contains(marker))
    {
        return Err(CodexError::NotLoggedIn);
    }
    if output.status == 0 {
        Ok(())
    } else {
        Err(CodexError::LoginStatusProbeFailed {
            status: output.status,
        })
    }
}
fn decode<T: DeserializeOwned>(json: String, schema: &'static str) -> Result<T, CodexError> {
    let value: serde_json::Value =
        serde_json::from_str(&json).map_err(|err| CodexError::InvalidJsonOutput {
            message: format!("{schema}: {err}"),
            raw_json: json.clone(),
        })?;
    serde_json::from_value(value).map_err(|err| CodexError::SchemaViolationOutput {
        schema,
        message: err.to_string(),
        raw_json: json,
    })
}
fn activity_schema() -> &'static str {
    include_str!("../../../schemas/activity-analysis.v2.schema.json")
}
fn quest_schema() -> &'static str {
    include_str!("../../../schemas/quest-proposal.v1.schema.json")
}
fn evidence_schema() -> &'static str {
    include_str!("../../../schemas/evidence-extraction.v1.schema.json")
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
    fn process_output(
        status: i32,
        stdout: &str,
        stderr: &str,
    ) -> Result<ProcessOutput, CodexError> {
        Ok(ProcessOutput {
            status,
            stdout: stdout.into(),
            stderr: stderr.into(),
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
    async fn probe_accepts_authenticated_status_emitted_to_stderr_by_macos_app_cli() {
        let mut replies = compatible_replies();
        replies[1] = process_output(
            0,
            "",
            "WARNING: proceeding, even though we could not create PATH aliases: Operation not permitted\nLogged in using ChatGPT\n",
        );
        let client = CodexClient::new(fake_path(), FakeRunner::new(replies)).unwrap();

        assert!(client.probe().await.is_ok());
    }
    #[test]
    fn login_status_uses_exit_status_without_exposing_stderr() {
        assert!(
            require_authenticated_login_status(
                &process_output(0, "", "unrecognized but successful output").unwrap()
            )
            .is_ok()
        );
        assert_eq!(
            require_authenticated_login_status(
                &process_output(1, "", "database at /private/path failed").unwrap()
            )
            .unwrap_err(),
            CodexError::LoginStatusProbeFailed { status: 1 }
        );
        assert_eq!(
            require_authenticated_login_status(&process_output(0, "not logged in", "").unwrap())
                .unwrap_err(),
            CodexError::NotLoggedIn
        );
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
        let invalid = decode::<ActivityAnalysisOutput>("not json".into(), ACTIVITY_SCHEMA_VERSION)
            .unwrap_err();
        assert!(matches!(&invalid, CodexError::InvalidJsonOutput { .. }));
        assert_eq!(invalid.raw_output(), Some("not json"));
        assert!(matches!(
            decode::<ActivityAnalysisOutput>("{}".into(), ACTIVITY_SCHEMA_VERSION),
            Err(CodexError::SchemaViolationOutput { .. })
        ));
        let missing_nullable = serde_json::json!({
            "summary": "整理した",
            "outcomes": [],
            "confirmedFacts": [],
            "unconfirmedFacts": [],
            "skillCandidates": []
        })
        .to_string();
        assert!(matches!(
            decode::<ActivityAnalysisOutput>(missing_nullable, ACTIVITY_SCHEMA_VERSION),
            Err(CodexError::SchemaViolationOutput { .. })
        ));
    }

    #[test]
    fn activity_schema_uses_supported_nullable_object_type() {
        let schema: serde_json::Value = serde_json::from_str(activity_schema()).unwrap();
        let next_question = &schema["properties"]["nextQuestion"];
        assert!(next_question.get("oneOf").is_none());
        assert_eq!(next_question["type"], serde_json::json!(["object", "null"]));
        assert_eq!(next_question["additionalProperties"], false);
        assert!(
            next_question["required"]
                .as_array()
                .is_some_and(|required| required.iter().any(|field| field == "questionId"))
        );
    }

    #[test]
    fn activity_v2_decodes_structured_question_and_specialized_skill() {
        let output = decode::<ActivityAnalysisOutput>(
            serde_json::json!({
                "summary": "一覧APIの遅延原因を調べた",
                "outcomes": ["SQLが原因候補だと分かった"],
                "confirmedFacts": ["一覧APIを調査した"],
                "unconfirmedFacts": ["改善後の速度は未測定"],
                "skillCandidates": [{
                    "skillId": "thinking.hypothesis_testing",
                    "specializedSkillName": "SQL性能調査",
                    "confidence": 0.72,
                    "reason": "原因候補を検証している",
                    "evidence": "SQLを原因候補として調べた"
                }],
                "nextQuestion": {
                    "questionId": "outcome_measurement",
                    "target": "measurement",
                    "text": "改善前後の速度を確認できましたか？",
                    "answerType": "single_choice",
                    "choices": [
                        { "value": "measured", "label": "数値で確認した" },
                        { "value": "felt", "label": "体感では改善した" }
                    ],
                    "whyItMatters": "成果を事実と推測に分けるため"
                }
            })
            .to_string(),
            ACTIVITY_SCHEMA_VERSION,
        )
        .unwrap();
        assert_eq!(output.confirmed_facts.len(), 1);
        assert_eq!(
            output.skill_candidates[0].specialized_skill_name.as_deref(),
            Some("SQL性能調査")
        );
        assert_eq!(
            output
                .next_question
                .as_ref()
                .map(|question| question.target.as_str()),
            Some("measurement")
        );
    }

    #[tokio::test]
    #[ignore = "probes the explicitly configured real Codex CLI; run only with an explicit command"]
    async fn real_codex_connection_probe_test() {
        let path = std::env::var("LEVELOG_CODEX_PATH")
            .expect("set LEVELOG_CODEX_PATH to the absolute Codex CLI path");
        let connection = CodexClient::new(PathBuf::from(path), TokioProcessRunner)
            .unwrap()
            .probe()
            .await
            .unwrap();
        assert!(!connection.version.trim().is_empty());
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
        assert!(!output.parsed.summary.trim().is_empty());
    }
}
