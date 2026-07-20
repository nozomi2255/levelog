use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error, Serialize)]
#[serde(tag = "code", content = "message", rename_all = "snake_case")]
pub enum AppError {
    #[error("入力内容が正しくありません: {0}")]
    Validation(String),
    #[error("対象のデータが見つかりません: {0}")]
    NotFound(String),
    #[error("現在の状態では操作できません: {0}")]
    InvalidState(String),
    #[error("データが別の操作で更新されました: {0}")]
    Conflict(String),
    #[error("データベース処理に失敗しました: {0}")]
    Database(String),
    #[error("Codexを利用できません: {0}")]
    Codex(String),
    #[error("ファイル処理に失敗しました: {0}")]
    Io(String),
    #[error("内部処理に失敗しました: {0}")]
    Internal(String),
}

impl From<sqlx::Error> for AppError {
    fn from(value: sqlx::Error) -> Self {
        Self::Database(value.to_string())
    }
}

impl From<std::io::Error> for AppError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value.to_string())
    }
}

impl From<crate::application::ServiceError> for AppError {
    fn from(value: crate::application::ServiceError) -> Self {
        match value {
            crate::application::ServiceError::NotFound(kind) => Self::NotFound(kind.into()),
            crate::application::ServiceError::AnalysisNotConfirmable => {
                Self::InvalidState("解析が確認可能な状態ではありません".into())
            }
            crate::application::ServiceError::AnalysisNotRunning => {
                Self::InvalidState("解析はすでに終了またはキャンセルされています".into())
            }
            crate::application::ServiceError::AnalysisAlreadyRunning => {
                Self::InvalidState("この活動の解析はすでに実行中です".into())
            }
            crate::application::ServiceError::InterviewQuestionPending => {
                Self::InvalidState("先に未回答の確認質問へ回答してください".into())
            }
            crate::application::ServiceError::InvalidCandidate(id) => {
                Self::Validation(format!("解析に属さない候補です: {id}"))
            }
            crate::application::ServiceError::InvalidCandidateEdit(message) => {
                Self::Validation(message)
            }
            crate::application::ServiceError::IncompleteCandidateDecisions => {
                Self::Validation("すべての分析候補に採用・編集・却下を一つ選んでください".into())
            }
            crate::application::ServiceError::UnknownSkill(id) => {
                Self::Validation(format!("固定カタログにないスキルです: {id}"))
            }
            crate::application::ServiceError::InvalidQuestTransition { from, to } => {
                Self::InvalidState(format!("クエストを {from} から {to} へ変更できません"))
            }
            crate::application::ServiceError::QuestNotReflectable => {
                Self::InvalidState("完了したクエストだけ振り返りを保存できます".into())
            }
            crate::application::ServiceError::QuestGenerationNotRunning => {
                Self::InvalidState("クエスト生成はすでに終了しています".into())
            }
            crate::application::ServiceError::Database(error) => Self::Database(error.to_string()),
        }
    }
}

impl From<crate::infrastructure::codex::CodexError> for AppError {
    fn from(value: crate::infrastructure::codex::CodexError) -> Self {
        Self::Codex(value.to_string())
    }
}
