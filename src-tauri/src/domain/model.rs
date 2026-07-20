use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestStatus {
    Proposed,
    Accepted,
    InProgress,
    Completed,
    Rescheduled,
    Adjusted,
    Cancelled,
}

impl QuestStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Proposed => "proposed",
            Self::Accepted => "accepted",
            Self::InProgress => "in_progress",
            Self::Completed => "completed",
            Self::Rescheduled => "rescheduled",
            Self::Adjusted => "adjusted",
            Self::Cancelled => "cancelled",
        }
    }

    pub fn transition_to(self, next: Self) -> Result<Self, DomainError> {
        let allowed = match self {
            Self::Proposed => matches!(
                next,
                Self::Accepted | Self::Rescheduled | Self::Adjusted | Self::Cancelled
            ),
            Self::Accepted => matches!(
                next,
                Self::InProgress | Self::Rescheduled | Self::Adjusted | Self::Cancelled
            ),
            Self::InProgress => matches!(
                next,
                Self::Completed | Self::Rescheduled | Self::Adjusted | Self::Cancelled
            ),
            Self::Rescheduled | Self::Adjusted => {
                matches!(next, Self::Accepted | Self::InProgress | Self::Cancelled)
            }
            Self::Completed | Self::Cancelled => false,
        };
        allowed
            .then_some(next)
            .ok_or(DomainError::InvalidQuestTransition {
                from: self,
                to: next,
            })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReflectionResult {
    Completed,
    PartiallyCompleted,
    NotCompleted,
    Rested,
}
impl ReflectionResult {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::PartiallyCompleted => "partially_completed",
            Self::NotCompleted => "not_completed",
            Self::Rested => "rested",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XpReason {
    ActivitySaved,
    AnalysisConfirmed,
    QuestReflectionSaved,
}
impl XpReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ActivitySaved => "activity_saved",
            Self::AnalysisConfirmed => "analysis_confirmed",
            Self::QuestReflectionSaved => "quest_reflection_saved",
        }
    }
}

#[derive(Debug, Error)]
pub enum DomainError {
    #[error("invalid quest transition from {from:?} to {to:?}")]
    InvalidQuestTransition { from: QuestStatus, to: QuestStatus },
}

/// Cumulative XP required to reach `level`; level 1 begins at zero XP.
pub fn xp_required_for_level(level: i64) -> i64 {
    50 * (level.saturating_sub(1)) * level
}

pub fn level_for_total_xp(total_xp: i64) -> i64 {
    let mut level = 1;
    while xp_required_for_level(level + 1) <= total_xp {
        level += 1;
    }
    level
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn levels_follow_the_fixed_curve() {
        assert_eq!(level_for_total_xp(0), 1);
        assert_eq!(level_for_total_xp(100), 2);
        assert_eq!(level_for_total_xp(299), 2);
        assert_eq!(level_for_total_xp(300), 3);
    }
    #[test]
    fn quest_state_machine_rejects_terminal_transitions() {
        assert!(
            QuestStatus::Proposed
                .transition_to(QuestStatus::Accepted)
                .is_ok()
        );
        assert!(
            QuestStatus::Completed
                .transition_to(QuestStatus::Accepted)
                .is_err()
        );
    }
}
