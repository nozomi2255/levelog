use chrono::{Duration, Local, NaiveDate, TimeZone, Utc};
use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use sqlx::{Row, Sqlite, Transaction, sqlite::SqliteRow};
use tauri::State;
use uuid::Uuid;

use crate::{
    domain::{level_for_total_xp, xp_required_for_level},
    dto::{
        ActivityDto, AppSettingsDto, BootState, CategoryObservation, CodexConnectionInput,
        CodexConnectionStatus, DashboardSnapshot, FocusThemeDto, FocusThemeInput, OnboardingInput,
        QuestDto, SaveFocusThemesInput, SkillDto, SpecializedSkillSummaryDto,
        USER_PROFILE_SCHEMA_VERSION, UpdateUserProfileInput, UserProfileDto, WeeklyXpPoint,
    },
    error::AppError,
    infrastructure::codex::TIMEOUT,
    state::AppState,
};

#[tauri::command]
pub async fn get_boot_state(state: State<'_, AppState>) -> Result<BootState, AppError> {
    let profile = load_profile(&state).await?;
    let codex = load_codex_connection(&state).await?.or_else(|| {
        profile.as_ref().map(|profile| CodexConnectionStatus {
            available: false,
            authenticated: false,
            path: profile.codex_path.clone(),
            version: None,
            message: "接続テスト未実行".into(),
        })
    });
    Ok(BootState {
        onboarding_complete: profile.is_some(),
        codex,
    })
}

#[tauri::command]
pub async fn update_codex_path(
    state: State<'_, AppState>,
    input: CodexConnectionInput,
) -> Result<AppSettingsDto, AppError> {
    let canonical = crate::infrastructure::codex::discovery::canonical_executable(
        std::path::Path::new(input.codex_path.trim()),
    )
    .map_err(AppError::Validation)?;
    let _permit = state
        .codex_semaphore
        .clone()
        .acquire_owned()
        .await
        .map_err(|_| AppError::InvalidState("Codex実行キューを開始できませんでした".into()))?;
    let connection = tokio::time::timeout(
        TIMEOUT,
        super::codex::test_connection(canonical.to_string_lossy().into_owned()),
    )
    .await
    .map_err(|_| AppError::Codex("Codex接続確認が180秒でタイムアウトしました".into()))?;
    if !connection.available || !connection.authenticated {
        return Err(AppError::Codex(connection.message));
    }
    let profile = load_profile(&state)
        .await?
        .ok_or_else(|| AppError::InvalidState("初期設定を完了してください".into()))?;
    let updated_at = now();
    let value = serde_json::to_string(&connection)
        .map_err(|error| AppError::Internal(error.to_string()))?;
    sqlx::query("INSERT INTO app_settings (key, value_json, updated_at) VALUES ('codex_connection', ?, ?) ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json, updated_at = excluded.updated_at")
        .bind(value)
        .bind(&updated_at)
        .execute(state.db.pool())
        .await?;
    Ok(AppSettingsDto {
        role: profile.role,
        focus_skill_ids: profile.focus_skill_ids,
        weekly_minutes: profile.weekly_minutes,
        excluded_quest_patterns: profile.excluded_quest_patterns,
        codex_path: connection.path,
        onboarding_complete: true,
        updated_at,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredProfileV2 {
    role: String,
    background: String,
    current_responsibilities: String,
    domains_and_technologies: Vec<String>,
    growth_goal: String,
    motivation: String,
    current_challenges: String,
    recent_success: String,
    focus_skill_ids: Vec<String>,
    weekly_minutes: i64,
    preferred_quest_minutes: i64,
    preferred_quest_style: String,
    constraints: String,
    excluded_quest_patterns: String,
}

#[tauri::command]
pub async fn get_user_profile(state: State<'_, AppState>) -> Result<UserProfileDto, AppError> {
    get_user_profile_inner(&state).await
}

#[tauri::command]
pub async fn update_user_profile(
    state: State<'_, AppState>,
    input: UpdateUserProfileInput,
) -> Result<UserProfileDto, AppError> {
    update_user_profile_inner(&state, input).await
}

async fn update_user_profile_inner(
    state: &AppState,
    input: UpdateUserProfileInput,
) -> Result<UserProfileDto, AppError> {
    ensure_profile_revision(state).await?;
    let normalized = validate_profile(input, state.db.pool()).await?;
    let mut tx = state.db.pool().begin().await?;
    let current = sqlx::query(
        "SELECT id, revision FROM user_profile_revisions ORDER BY revision DESC LIMIT 1",
    )
    .fetch_optional(&mut *tx)
    .await?;
    let (supersedes_id, current_revision) = current
        .map(|row| {
            (
                Some(row.get::<String, _>("id")),
                row.get::<i64, _>("revision"),
            )
        })
        .unwrap_or((None, 0));
    if current_revision > 0 && normalized.expected_revision.is_none() {
        return Err(AppError::Conflict(
            "プロフィールrevisionを指定して再度保存してください".into(),
        ));
    }
    if let Some(expected) = normalized.expected_revision
        && expected != current_revision
    {
        return Err(AppError::Conflict(format!(
            "プロフィールrevision {current_revision}を再読み込みしてください"
        )));
    }
    let stored = stored_profile(&normalized);
    let id = Uuid::new_v4().to_string();
    let created_at = now();
    let insert = sqlx::query("INSERT INTO user_profile_revisions (id, schema_version, revision, profile_json, created_at, supersedes_id) VALUES (?, ?, ?, ?, ?, ?)")
        .bind(id)
        .bind(USER_PROFILE_SCHEMA_VERSION)
        .bind(current_revision + 1)
        .bind(serde_json::to_string(&stored).map_err(|error| AppError::Internal(error.to_string()))?)
        .bind(&created_at)
        .bind(supersedes_id)
        .execute(&mut *tx)
        .await;
    if let Err(error) = insert {
        if matches!(&error, sqlx::Error::Database(database) if database.is_unique_violation()) {
            return Err(AppError::Conflict(
                "プロフィールが同時に更新されました。再読み込みしてください".into(),
            ));
        }
        return Err(error.into());
    }
    tx.commit().await?;
    get_user_profile_inner(state).await
}

#[tauri::command]
pub async fn list_focus_themes(state: State<'_, AppState>) -> Result<Vec<FocusThemeDto>, AppError> {
    ensure_profile_revision(&state).await?;
    load_focus_themes(state.db.pool()).await
}

#[tauri::command]
pub async fn save_focus_themes(
    state: State<'_, AppState>,
    input: SaveFocusThemesInput,
) -> Result<Vec<FocusThemeDto>, AppError> {
    if input.themes.len() > 10 {
        return Err(AppError::Validation(
            "フォーカステーマは10件まで保存できます".into(),
        ));
    }
    let mut seen_ids = HashSet::new();
    let mut tx = state.db.pool().begin().await?;
    for theme in input.themes {
        validate_theme(&theme, &mut tx).await?;
        let id = match theme.id {
            Some(id) => {
                if !seen_ids.insert(id.clone()) {
                    return Err(AppError::Validation("テーマIDが重複しています".into()));
                }
                let exists: i64 =
                    sqlx::query_scalar("SELECT COUNT(*) FROM focus_themes WHERE id = ?")
                        .bind(&id)
                        .fetch_one(&mut *tx)
                        .await?;
                if exists != 1 {
                    return Err(AppError::NotFound("フォーカステーマ".into()));
                }
                id
            }
            None => Uuid::new_v4().to_string(),
        };
        let timestamp = now();
        sqlx::query("INSERT INTO focus_themes (id, title, desired_outcome, why_now, horizon, status, sort_order, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?) ON CONFLICT(id) DO UPDATE SET title = excluded.title, desired_outcome = excluded.desired_outcome, why_now = excluded.why_now, horizon = excluded.horizon, status = excluded.status, sort_order = excluded.sort_order, updated_at = excluded.updated_at")
            .bind(&id).bind(theme.title.trim()).bind(theme.desired_outcome.trim()).bind(theme.why_now.trim()).bind(theme.horizon.trim()).bind(theme.status.trim()).bind(theme.sort_order).bind(&timestamp).bind(&timestamp).execute(&mut *tx).await?;
        sqlx::query("DELETE FROM focus_theme_skill_links WHERE theme_id = ?")
            .bind(&id)
            .execute(&mut *tx)
            .await?;
        for skill_id in dedupe_strings(theme.linked_skill_ids) {
            sqlx::query("INSERT INTO focus_theme_skill_links (theme_id, skill_id, relevance, created_at) VALUES (?, ?, 1, ?)")
                .bind(&id).bind(skill_id).bind(&timestamp).execute(&mut *tx).await?;
        }
    }
    tx.commit().await?;
    load_focus_themes(state.db.pool()).await
}

#[tauri::command]
pub async fn save_onboarding(
    state: State<'_, AppState>,
    input: OnboardingInput,
) -> Result<AppSettingsDto, AppError> {
    ensure_profile_revision(&state).await?;
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
    let mut tx = state.db.pool().begin().await?;
    sqlx::query("INSERT INTO app_settings (key, value_json, updated_at) VALUES ('profile', ?, ?) ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json, updated_at = excluded.updated_at")
        .bind(value)
        .bind(&now)
        .execute(&mut *tx)
        .await?;
    // Keep compatibility with older callers without letting them silently update
    // only the legacy settings row. Every accepted profile change is a revision.
    let current = sqlx::query("SELECT id, revision, profile_json FROM user_profile_revisions ORDER BY revision DESC LIMIT 1")
        .fetch_optional(&mut *tx)
        .await?;
    let (supersedes_id, revision, mut stored) = if let Some(row) = current {
        let stored = serde_json::from_str::<StoredProfileV2>(&row.get::<String, _>("profile_json"))
            .map_err(|error| AppError::Internal(error.to_string()))?;
        (
            Some(row.get::<String, _>("id")),
            row.get::<i64, _>("revision") + 1,
            stored,
        )
    } else {
        (
            None,
            1,
            StoredProfileV2 {
                role: String::new(),
                background: String::new(),
                current_responsibilities: String::new(),
                domains_and_technologies: vec![],
                growth_goal: String::new(),
                motivation: String::new(),
                current_challenges: String::new(),
                recent_success: String::new(),
                focus_skill_ids: vec![],
                weekly_minutes: 60,
                preferred_quest_minutes: 15,
                preferred_quest_style: "balanced".into(),
                constraints: String::new(),
                excluded_quest_patterns: String::new(),
            },
        )
    };
    stored.role = normalized.role.clone();
    stored.focus_skill_ids = normalized.focus_skill_ids.clone();
    stored.weekly_minutes = normalized.weekly_minutes;
    stored.excluded_quest_patterns = normalized.excluded_quest_patterns.clone();
    sqlx::query("INSERT INTO user_profile_revisions (id, schema_version, revision, profile_json, created_at, supersedes_id) VALUES (?, ?, ?, ?, ?, ?)")
        .bind(Uuid::new_v4().to_string())
        .bind(USER_PROFILE_SCHEMA_VERSION)
        .bind(revision)
        .bind(serde_json::to_string(&stored).map_err(|error| AppError::Internal(error.to_string()))?)
        .bind(&now)
        .bind(supersedes_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
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
    let specialized_rows = sqlx::query("SELECT o.skill_id, (SELECT o2.specialized_skill_name FROM skill_observations o2 WHERE o2.skill_id = o.skill_id AND o2.normalized_specialized_skill_name = o.normalized_specialized_skill_name ORDER BY o2.created_at DESC, o2.rowid DESC LIMIT 1) specialized_skill_name, COUNT(*) evidence_count, MAX(o.created_at) last_observed_at FROM skill_observations o WHERE o.normalized_specialized_skill_name IS NOT NULL AND trim(o.normalized_specialized_skill_name) <> '' GROUP BY o.skill_id, o.normalized_specialized_skill_name ORDER BY o.skill_id, evidence_count DESC, specialized_skill_name")
        .fetch_all(state.db.pool()).await?;
    let mut specialized: HashMap<String, Vec<SpecializedSkillSummaryDto>> = HashMap::new();
    for row in specialized_rows {
        specialized
            .entry(row.get("skill_id"))
            .or_default()
            .push(SpecializedSkillSummaryDto {
                name: row.get("specialized_skill_name"),
                evidence_count: row.get("evidence_count"),
                last_observed_at: row.try_get("last_observed_at").ok(),
            });
    }
    Ok(rows
        .iter()
        .map(|row| SkillDto {
            id: row.get("id"),
            code: row.get("id"),
            name: row.get("name"),
            category: row.get("category"),
            evidence_count: row.get("evidence_count"),
            state: "observing".into(),
            specialized_skills: specialized
                .remove(row.get::<String, _>("id").as_str())
                .unwrap_or_default(),
        })
        .collect())
}

pub(crate) async fn load_profile(state: &AppState) -> Result<Option<OnboardingInput>, AppError> {
    ensure_profile_revision(state).await?;
    let latest = sqlx::query(
        "SELECT profile_json FROM user_profile_revisions ORDER BY revision DESC LIMIT 1",
    )
    .fetch_optional(state.db.pool())
    .await?;
    let legacy: Option<String> =
        sqlx::query_scalar("SELECT value_json FROM app_settings WHERE key = 'profile'")
            .fetch_optional(state.db.pool())
            .await?;
    let codex_path = load_codex_connection(state)
        .await?
        .map(|value| value.path)
        .or_else(|| {
            legacy
                .as_deref()
                .and_then(|json| serde_json::from_str::<OnboardingInput>(json).ok())
                .map(|profile| profile.codex_path)
        })
        .unwrap_or_default();
    if let Some(row) = latest {
        let stored: StoredProfileV2 = serde_json::from_str(&row.get::<String, _>("profile_json"))
            .map_err(|error| AppError::Internal(error.to_string()))?;
        return Ok(Some(OnboardingInput {
            role: stored.role,
            focus_skill_ids: stored.focus_skill_ids,
            weekly_minutes: stored.weekly_minutes,
            excluded_quest_patterns: stored.excluded_quest_patterns,
            codex_path,
        }));
    }
    legacy
        .map(|json| {
            serde_json::from_str(&json).map_err(|error| AppError::Internal(error.to_string()))
        })
        .transpose()
}

async fn load_codex_connection(
    state: &AppState,
) -> Result<Option<CodexConnectionStatus>, AppError> {
    let value: Option<String> =
        sqlx::query_scalar("SELECT value_json FROM app_settings WHERE key = 'codex_connection'")
            .fetch_optional(state.db.pool())
            .await?;
    value
        .map(|json| {
            serde_json::from_str(&json).map_err(|error| AppError::Internal(error.to_string()))
        })
        .transpose()
}

async fn ensure_profile_revision(state: &AppState) -> Result<(), AppError> {
    let exists: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM user_profile_revisions")
        .fetch_one(state.db.pool())
        .await?;
    if exists > 0 {
        return Ok(());
    }
    let legacy_json: Option<String> =
        sqlx::query_scalar("SELECT value_json FROM app_settings WHERE key = 'profile'")
            .fetch_optional(state.db.pool())
            .await?;
    let Some(legacy_json) = legacy_json else {
        return Ok(());
    };
    let legacy: OnboardingInput = serde_json::from_str(&legacy_json)
        .map_err(|error| AppError::Internal(error.to_string()))?;
    let mut tx = state.db.pool().begin().await?;
    let exists: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM user_profile_revisions")
        .fetch_one(&mut *tx)
        .await?;
    if exists == 0 {
        let stored = StoredProfileV2 {
            role: legacy.role.trim().into(),
            background: String::new(),
            current_responsibilities: String::new(),
            domains_and_technologies: vec![],
            growth_goal: String::new(),
            motivation: String::new(),
            current_challenges: String::new(),
            recent_success: String::new(),
            focus_skill_ids: dedupe_strings(legacy.focus_skill_ids.clone()),
            weekly_minutes: legacy.weekly_minutes,
            preferred_quest_minutes: 15,
            preferred_quest_style: "balanced".into(),
            constraints: String::new(),
            excluded_quest_patterns: legacy.excluded_quest_patterns.trim().into(),
        };
        let created_at = now();
        sqlx::query("INSERT INTO user_profile_revisions (id, schema_version, revision, profile_json, created_at, supersedes_id) VALUES (?, ?, 1, ?, ?, NULL)")
            .bind(Uuid::new_v4().to_string()).bind(USER_PROFILE_SCHEMA_VERSION).bind(serde_json::to_string(&stored).map_err(|error| AppError::Internal(error.to_string()))?).bind(&created_at).execute(&mut *tx).await?;
        if !stored.focus_skill_ids.is_empty() {
            let theme_id = Uuid::new_v4().to_string();
            sqlx::query("INSERT INTO focus_themes (id, title, desired_outcome, why_now, horizon, status, sort_order, created_at, updated_at) VALUES (?, '初期設定の重点領域', '', '', 'ongoing', 'active', 0, ?, ?)")
                .bind(&theme_id).bind(&created_at).bind(&created_at).execute(&mut *tx).await?;
            for skill_id in &stored.focus_skill_ids {
                sqlx::query("INSERT INTO focus_theme_skill_links (theme_id, skill_id, relevance, created_at) VALUES (?, ?, 1, ?)")
                    .bind(&theme_id).bind(skill_id).bind(&created_at).execute(&mut *tx).await?;
            }
        }
    }
    tx.commit().await?;
    Ok(())
}

async fn get_user_profile_inner(state: &AppState) -> Result<UserProfileDto, AppError> {
    ensure_profile_revision(state).await?;
    let row = sqlx::query("SELECT schema_version, revision, profile_json, created_at FROM user_profile_revisions ORDER BY revision DESC LIMIT 1")
        .fetch_optional(state.db.pool()).await?.ok_or_else(|| AppError::NotFound("ユーザープロフィール".into()))?;
    let schema_version: i64 = row.get("schema_version");
    if schema_version != USER_PROFILE_SCHEMA_VERSION {
        return Err(AppError::Internal(format!(
            "未対応のプロフィールschema versionです: {schema_version}"
        )));
    }
    let stored: StoredProfileV2 = serde_json::from_str(&row.get::<String, _>("profile_json"))
        .map_err(|error| AppError::Internal(error.to_string()))?;
    Ok(UserProfileDto {
        schema_version,
        revision: row.get("revision"),
        role: stored.role,
        background: stored.background,
        current_responsibilities: stored.current_responsibilities,
        domains_and_technologies: stored.domains_and_technologies,
        growth_goal: stored.growth_goal,
        motivation: stored.motivation,
        current_challenges: stored.current_challenges,
        recent_success: stored.recent_success,
        focus_skill_ids: stored.focus_skill_ids,
        weekly_minutes: stored.weekly_minutes,
        preferred_quest_minutes: stored.preferred_quest_minutes,
        preferred_quest_style: stored.preferred_quest_style,
        constraints: stored.constraints,
        excluded_quest_patterns: stored.excluded_quest_patterns,
        focus_themes: load_focus_themes(state.db.pool()).await?,
        updated_at: row.get("created_at"),
    })
}

fn stored_profile(input: &UpdateUserProfileInput) -> StoredProfileV2 {
    StoredProfileV2 {
        role: input.role.clone(),
        background: input.background.clone(),
        current_responsibilities: input.current_responsibilities.clone(),
        domains_and_technologies: input.domains_and_technologies.clone(),
        growth_goal: input.growth_goal.clone(),
        motivation: input.motivation.clone(),
        current_challenges: input.current_challenges.clone(),
        recent_success: input.recent_success.clone(),
        focus_skill_ids: input.focus_skill_ids.clone(),
        weekly_minutes: input.weekly_minutes,
        preferred_quest_minutes: input.preferred_quest_minutes,
        preferred_quest_style: input.preferred_quest_style.clone(),
        constraints: input.constraints.clone(),
        excluded_quest_patterns: input.excluded_quest_patterns.clone(),
    }
}

async fn validate_profile(
    mut input: UpdateUserProfileInput,
    pool: &sqlx::SqlitePool,
) -> Result<UpdateUserProfileInput, AppError> {
    input.role = required_text("現在の役割", input.role, 120)?;
    input.background = limited_text("背景", input.background, 4_000)?;
    input.current_responsibilities =
        limited_text("現在の責任", input.current_responsibilities, 4_000)?;
    input.growth_goal = limited_text("成長目標", input.growth_goal, 2_000)?;
    input.motivation = limited_text("動機", input.motivation, 2_000)?;
    input.current_challenges = limited_text("現在の課題", input.current_challenges, 4_000)?;
    input.recent_success = limited_text("最近の成功", input.recent_success, 4_000)?;
    input.preferred_quest_style =
        required_text("クエストスタイル", input.preferred_quest_style, 80)?;
    input.constraints = limited_text("制約", input.constraints, 4_000)?;
    input.excluded_quest_patterns =
        limited_text("避けたいクエスト", input.excluded_quest_patterns, 4_000)?;
    input.domains_and_technologies = dedupe_strings(input.domains_and_technologies)
        .into_iter()
        .map(|value| required_text("領域・技術", value, 120))
        .collect::<Result<Vec<_>, _>>()?;
    if input.domains_and_technologies.len() > 20 {
        return Err(AppError::Validation("領域・技術は20件までです".into()));
    }
    input.focus_skill_ids = dedupe_strings(input.focus_skill_ids);
    if !(1..=3).contains(&input.focus_skill_ids.len()) {
        return Err(AppError::Validation(
            "重点スキルは1〜3個選択してください".into(),
        ));
    }
    validate_skill_ids_pool(pool, &input.focus_skill_ids).await?;
    if !(1..=10_080).contains(&input.weekly_minutes) {
        return Err(AppError::Validation(
            "週の利用時間を正しく入力してください".into(),
        ));
    }
    if !(5..=120).contains(&input.preferred_quest_minutes) {
        return Err(AppError::Validation(
            "希望クエスト時間は5〜120分で指定してください".into(),
        ));
    }
    Ok(input)
}

async fn validate_theme(
    theme: &FocusThemeInput,
    tx: &mut Transaction<'_, Sqlite>,
) -> Result<(), AppError> {
    required_text("テーマ名", theme.title.clone(), 120)?;
    limited_text("望む変化", theme.desired_outcome.clone(), 2_000)?;
    limited_text("今取り組む理由", theme.why_now.clone(), 2_000)?;
    if !["now", "quarter", "year", "ongoing"].contains(&theme.horizon.trim()) {
        return Err(AppError::Validation("テーマ期間が正しくありません".into()));
    }
    if !["active", "paused", "completed"].contains(&theme.status.trim()) {
        return Err(AppError::Validation("テーマ状態が正しくありません".into()));
    }
    if !(-10_000..=10_000).contains(&theme.sort_order) {
        return Err(AppError::Validation("テーマの表示順が範囲外です".into()));
    }
    let ids = dedupe_strings(theme.linked_skill_ids.clone());
    if ids.len() != theme.linked_skill_ids.len() {
        return Err(AppError::Validation(
            "テーマ内のスキルが重複しています".into(),
        ));
    }
    for id in ids {
        let exists: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM skills WHERE id = ? AND is_active = 1")
                .bind(&id)
                .fetch_one(&mut **tx)
                .await?;
        if exists != 1 {
            return Err(AppError::Validation(format!(
                "固定カタログにないスキルです: {id}"
            )));
        }
    }
    Ok(())
}

async fn validate_skill_ids_pool(pool: &sqlx::SqlitePool, ids: &[String]) -> Result<(), AppError> {
    for id in ids {
        let exists: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM skills WHERE id = ? AND is_active = 1")
                .bind(id)
                .fetch_one(pool)
                .await?;
        if exists != 1 {
            return Err(AppError::Validation(format!(
                "固定カタログにないスキルです: {id}"
            )));
        }
    }
    Ok(())
}

async fn load_focus_themes(pool: &sqlx::SqlitePool) -> Result<Vec<FocusThemeDto>, AppError> {
    let theme_rows = sqlx::query("SELECT id, title, desired_outcome, why_now, horizon, status, sort_order, updated_at FROM focus_themes ORDER BY sort_order, created_at, id")
        .fetch_all(pool).await?;
    let link_rows = sqlx::query(
        "SELECT theme_id, skill_id FROM focus_theme_skill_links ORDER BY theme_id, skill_id",
    )
    .fetch_all(pool)
    .await?;
    let mut links: HashMap<String, Vec<String>> = HashMap::new();
    for row in link_rows {
        links
            .entry(row.get("theme_id"))
            .or_default()
            .push(row.get("skill_id"));
    }
    Ok(theme_rows
        .into_iter()
        .map(|row| {
            let id: String = row.get("id");
            FocusThemeDto {
                linked_skill_ids: links.remove(&id).unwrap_or_default(),
                id,
                title: row.get("title"),
                desired_outcome: row.get("desired_outcome"),
                why_now: row.get("why_now"),
                horizon: row.get("horizon"),
                status: row.get("status"),
                sort_order: row.get("sort_order"),
                updated_at: row.get("updated_at"),
            }
        })
        .collect())
}

fn required_text(label: &str, value: String, max: usize) -> Result<String, AppError> {
    let value = value.trim().to_owned();
    if value.is_empty() {
        return Err(AppError::Validation(format!("{label}を入力してください")));
    }
    if value.chars().count() > max {
        return Err(AppError::Validation(format!("{label}は{max}文字以内です")));
    }
    Ok(value)
}
fn limited_text(label: &str, value: String, max: usize) -> Result<String, AppError> {
    let value = value.trim().to_owned();
    if value.chars().count() > max {
        return Err(AppError::Validation(format!("{label}は{max}文字以内です")));
    }
    Ok(value)
}
fn dedupe_strings(values: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    values
        .into_iter()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty() && seen.insert(value.clone()))
        .collect()
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

#[cfg(test)]
mod tests {
    use super::*;

    async fn state() -> (tempfile::TempDir, AppState) {
        let directory = tempfile::tempdir().unwrap();
        let state = AppState::initialize(directory.path().to_path_buf())
            .await
            .unwrap();
        (directory, state)
    }

    fn update(expected_revision: Option<i64>) -> UpdateUserProfileInput {
        UpdateUserProfileInput {
            expected_revision,
            role: " プロダクトエンジニア ".into(),
            background: " 開発経験 ".into(),
            current_responsibilities: " 設計と実装 ".into(),
            domains_and_technologies: vec![" Rust ".into(), "Rust".into()],
            growth_goal: "設計判断を改善する".into(),
            motivation: "仕事の質を高める".into(),
            current_challenges: "曖昧な要件".into(),
            recent_success: "仕様を整理した".into(),
            focus_skill_ids: vec!["thinking.information_structuring".into()],
            weekly_minutes: 120,
            preferred_quest_minutes: 15,
            preferred_quest_style: "balanced".into(),
            constraints: "".into(),
            excluded_quest_patterns: "".into(),
        }
    }

    #[tokio::test]
    async fn legacy_profile_is_lazily_migrated_without_deleting_the_setting() {
        let (_directory, state) = state().await;
        let legacy = OnboardingInput {
            role: "developer".into(),
            focus_skill_ids: vec!["thinking.information_structuring".into()],
            weekly_minutes: 90,
            excluded_quest_patterns: "calls".into(),
            codex_path: "/tmp/codex".into(),
        };
        sqlx::query(
            "INSERT INTO app_settings (key, value_json, updated_at) VALUES ('profile', ?, ?)",
        )
        .bind(serde_json::to_string(&legacy).unwrap())
        .bind(now())
        .execute(state.db.pool())
        .await
        .unwrap();
        let profile = get_user_profile_inner(&state).await.unwrap();
        assert_eq!(profile.revision, 1);
        assert_eq!(profile.role, "developer");
        assert_eq!(profile.focus_themes.len(), 1);
        let legacy_still_exists: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM app_settings WHERE key = 'profile'")
                .fetch_one(state.db.pool())
                .await
                .unwrap();
        assert_eq!(legacy_still_exists, 1);
    }

    #[tokio::test]
    async fn profile_update_appends_and_rejects_stale_revision() {
        let (_directory, state) = state().await;
        let first = update_user_profile_inner(&state, update(None))
            .await
            .unwrap();
        assert_eq!(first.revision, 1);
        assert_eq!(first.domains_and_technologies, vec!["Rust"]);
        let second = update_user_profile_inner(&state, update(Some(1)))
            .await
            .unwrap();
        assert_eq!(second.revision, 2);
        assert!(matches!(
            update_user_profile_inner(&state, update(Some(1))).await,
            Err(AppError::Conflict(_))
        ));
        let revisions: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM user_profile_revisions")
            .fetch_one(state.db.pool())
            .await
            .unwrap();
        assert_eq!(revisions, 2);
    }

    #[tokio::test]
    async fn invalid_focus_skill_is_rejected() {
        let (_directory, state) = state().await;
        let mut input = update(None);
        input.focus_skill_ids = vec!["invented.skill".into()];
        assert!(matches!(
            validate_profile(input, state.db.pool()).await,
            Err(AppError::Validation(_))
        ));
    }
}
