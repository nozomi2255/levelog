use chrono::{Duration, Local, NaiveDate, TimeZone, Utc};
use sqlx::{Row, sqlite::SqliteRow};
use tauri::State;

use crate::{
    domain::{level_for_total_xp, xp_required_for_level},
    dto::{
        ActivityDto, AppSettingsDto, BootState, CategoryObservation, CodexConnectionInput,
        CodexConnectionStatus, DashboardSnapshot, OnboardingInput, QuestDto, SkillDto,
        WeeklyXpPoint,
    },
    error::AppError,
    state::AppState,
};

#[tauri::command]
pub async fn get_boot_state(state: State<'_, AppState>) -> Result<BootState, AppError> {
    let profile = load_profile(&state).await?;
    Ok(BootState {
        onboarding_complete: profile.is_some(),
        codex: profile.map(|profile| CodexConnectionStatus {
            available: false,
            authenticated: false,
            path: profile.codex_path,
            version: None,
            message: "接続テスト未実行".into(),
        }),
    })
}

#[tauri::command]
pub async fn update_codex_path(
    state: State<'_, AppState>,
    input: CodexConnectionInput,
) -> Result<AppSettingsDto, AppError> {
    if !std::path::Path::new(&input.codex_path).is_absolute() {
        return Err(AppError::Validation(
            "Codex CLIは絶対パスで指定してください".into(),
        ));
    }
    let mut profile = load_profile(&state)
        .await?
        .ok_or_else(|| AppError::InvalidState("初期設定を完了してください".into()))?;
    profile.codex_path = input.codex_path;
    let updated_at = now();
    let value =
        serde_json::to_string(&profile).map_err(|error| AppError::Internal(error.to_string()))?;
    sqlx::query("UPDATE app_settings SET value_json = ?, updated_at = ? WHERE key = 'profile'")
        .bind(value)
        .bind(&updated_at)
        .execute(state.db.pool())
        .await?;
    Ok(AppSettingsDto {
        role: profile.role,
        focus_skill_ids: profile.focus_skill_ids,
        weekly_minutes: profile.weekly_minutes,
        excluded_quest_patterns: profile.excluded_quest_patterns,
        codex_path: profile.codex_path,
        onboarding_complete: true,
        updated_at,
    })
}

#[tauri::command]
pub async fn save_onboarding(
    state: State<'_, AppState>,
    input: OnboardingInput,
) -> Result<AppSettingsDto, AppError> {
    let role = input.role.trim();
    if role.is_empty() {
        return Err(AppError::Validation("現在の役割を入力してください".into()));
    }
    if !(1..=3).contains(&input.focus_skill_ids.len()) {
        return Err(AppError::Validation(
            "重点スキルは1〜3個選択してください".into(),
        ));
    }
    if input.weekly_minutes <= 0 || input.weekly_minutes > 10_080 {
        return Err(AppError::Validation(
            "週の利用時間を正しく入力してください".into(),
        ));
    }
    if !std::path::Path::new(&input.codex_path).is_absolute() {
        return Err(AppError::Validation(
            "Codex CLIは絶対パスで指定してください".into(),
        ));
    }
    for skill_id in &input.focus_skill_ids {
        let exists: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM skills WHERE id = ?")
            .bind(skill_id)
            .fetch_one(state.db.pool())
            .await?;
        if exists != 1 {
            return Err(AppError::Validation(format!(
                "固定カタログにないスキルです: {skill_id}"
            )));
        }
    }
    let normalized = OnboardingInput {
        role: role.into(),
        focus_skill_ids: input.focus_skill_ids,
        weekly_minutes: input.weekly_minutes,
        excluded_quest_patterns: input.excluded_quest_patterns.trim().into(),
        codex_path: input.codex_path,
    };
    let now = now();
    let value = serde_json::to_string(&normalized)
        .map_err(|error| AppError::Internal(error.to_string()))?;
    sqlx::query("INSERT INTO app_settings (key, value_json, updated_at) VALUES ('profile', ?, ?) ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json, updated_at = excluded.updated_at")
        .bind(value)
        .bind(&now)
        .execute(state.db.pool())
        .await?;
    Ok(AppSettingsDto {
        role: normalized.role,
        focus_skill_ids: normalized.focus_skill_ids,
        weekly_minutes: normalized.weekly_minutes,
        excluded_quest_patterns: normalized.excluded_quest_patterns,
        codex_path: normalized.codex_path,
        onboarding_complete: true,
        updated_at: now,
    })
}

#[tauri::command]
pub async fn get_dashboard(state: State<'_, AppState>) -> Result<DashboardSnapshot, AppError> {
    let total_xp: i64 = sqlx::query_scalar("SELECT COALESCE(SUM(amount), 0) FROM xp_events")
        .fetch_one(state.db.pool())
        .await?;
    let level = level_for_total_xp(total_xp);
    let today_date = Local::now().date_naive();
    let today = today_date.to_string();
    let (today_start, today_end) = utc_bounds_for_local_date(today_date)?;
    let today_xp: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(amount), 0) FROM xp_events WHERE created_at >= ? AND created_at < ?",
    )
    .bind(&today_start)
    .bind(&today_end)
    .fetch_one(state.db.pool())
    .await?;
    let today_activities: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM activities WHERE occurred_on = ?")
            .bind(&today)
            .fetch_one(state.db.pool())
            .await?;
    let today_observations: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM skill_observations o JOIN activities a ON a.id = o.activity_id WHERE a.occurred_on = ?",
    )
    .bind(&today)
    .fetch_one(state.db.pool())
    .await?;

    let active_quest = sqlx::query("SELECT * FROM quests WHERE status IN ('accepted', 'in_progress', 'rescheduled', 'adjusted') ORDER BY updated_at DESC LIMIT 1")
        .fetch_optional(state.db.pool())
        .await?
        .map(|row| quest_from_row(&row));

    let recent_activities = sqlx::query(
        "SELECT a.*, (SELECT status FROM ai_analyses x WHERE x.activity_id = a.id ORDER BY x.created_at DESC LIMIT 1) analysis_status FROM activities a ORDER BY occurred_on DESC, created_at DESC LIMIT 5",
    )
    .fetch_all(state.db.pool())
    .await?
    .iter()
    .map(activity_from_row)
    .collect();

    let start = Local::now().date_naive() - Duration::days(6);
    let mut weekly_xp = Vec::with_capacity(7);
    for offset in 0..7 {
        let local_date = start + Duration::days(offset);
        let date = local_date.to_string();
        let (day_start, day_end) = utc_bounds_for_local_date(local_date)?;
        let xp: i64 = sqlx::query_scalar(
            "SELECT COALESCE(SUM(amount), 0) FROM xp_events WHERE created_at >= ? AND created_at < ?",
        )
        .bind(day_start)
        .bind(day_end)
        .fetch_one(state.db.pool())
        .await?;
        weekly_xp.push(WeeklyXpPoint { date, xp });
    }

    let category_observations = sqlx::query("SELECT s.category, COUNT(*) count FROM skill_observations o JOIN skills s ON s.id = o.skill_id JOIN activities a ON a.id = o.activity_id WHERE a.occurred_on >= ? GROUP BY s.category ORDER BY s.category")
        .bind(start.to_string())
        .fetch_all(state.db.pool())
        .await?
        .iter()
        .map(|row| CategoryObservation {
            category: row.get("category"),
            count: row.get("count"),
        })
        .collect();

    Ok(DashboardSnapshot {
        level,
        total_xp,
        xp_to_next_level: xp_required_for_level(level + 1) - total_xp,
        today_xp,
        today_activities,
        today_observations,
        active_quest,
        recent_activities,
        weekly_xp,
        category_observations,
    })
}

#[tauri::command]
pub async fn list_skills(state: State<'_, AppState>) -> Result<Vec<SkillDto>, AppError> {
    let rows = sqlx::query("SELECT s.id, s.name, s.category, COUNT(o.id) evidence_count FROM skills s LEFT JOIN skill_observations o ON o.skill_id = s.id WHERE s.is_active = 1 GROUP BY s.id ORDER BY s.category, s.id")
        .fetch_all(state.db.pool())
        .await?;
    Ok(rows
        .iter()
        .map(|row| SkillDto {
            id: row.get("id"),
            code: row.get("id"),
            name: row.get("name"),
            category: row.get("category"),
            evidence_count: row.get("evidence_count"),
            state: "observing".into(),
        })
        .collect())
}

pub(crate) async fn load_profile(state: &AppState) -> Result<Option<OnboardingInput>, AppError> {
    let value: Option<String> =
        sqlx::query_scalar("SELECT value_json FROM app_settings WHERE key = 'profile'")
            .fetch_optional(state.db.pool())
            .await?;
    value
        .map(|json| {
            serde_json::from_str(&json).map_err(|error| AppError::Internal(error.to_string()))
        })
        .transpose()
}

pub(crate) fn activity_from_row(row: &SqliteRow) -> ActivityDto {
    ActivityDto {
        id: row.get("id"),
        occurred_on: row.get("occurred_on"),
        action_text: row.get("action_text"),
        challenge_text: row.get("challenge_text"),
        outcome_text: row.get("outcome_text"),
        created_at: row.get("created_at"),
        analysis_status: row.try_get("analysis_status").ok(),
    }
}

pub(crate) fn quest_from_row(row: &SqliteRow) -> QuestDto {
    let criteria: String = row.get("success_criteria_json");
    QuestDto {
        id: row.get("id"),
        template_id: row.get("template_id"),
        title: row.get("title"),
        description: row.get("description"),
        target_skill_id: row.try_get("target_skill_id").unwrap_or_default(),
        estimated_minutes: row.get("estimated_minutes"),
        difficulty: row.get("difficulty"),
        success_criteria: serde_json::from_str(&criteria).unwrap_or_default(),
        evidence_prompt: row.get("evidence_prompt"),
        status: row.get("status"),
        scheduled_on: row.get("scheduled_on"),
    }
}

pub(crate) fn now() -> String {
    Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

fn utc_bounds_for_local_date(date: NaiveDate) -> Result<(String, String), AppError> {
    let start_naive = date
        .and_hms_opt(0, 0, 0)
        .ok_or_else(|| AppError::Internal("ローカル日付を変換できませんでした".into()))?;
    let end_naive = (date + Duration::days(1))
        .and_hms_opt(0, 0, 0)
        .ok_or_else(|| AppError::Internal("ローカル日付を変換できませんでした".into()))?;
    let start = Local
        .from_local_datetime(&start_naive)
        .earliest()
        .ok_or_else(|| AppError::Internal("ローカル時刻をUTCへ変換できませんでした".into()))?
        .with_timezone(&Utc)
        .to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let end = Local
        .from_local_datetime(&end_naive)
        .earliest()
        .ok_or_else(|| AppError::Internal("ローカル時刻をUTCへ変換できませんでした".into()))?
        .with_timezone(&Utc)
        .to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    Ok((start, end))
}
