use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Claims {
    pub user_id: i64,
    #[serde(default)]
    pub email: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub role: String,
    #[serde(default)]
    pub branch: Option<String>,
    #[serde(default)]
    pub exp: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Notification {
    pub id: Uuid,
    pub event_type: String,
    pub title: String,
    pub message: String,
    pub severity: String,
    pub action_url: Option<String>,
    pub target_user_ids: Vec<i64>,
    pub target_branches: Vec<String>,
    pub target_roles: Vec<String>,
    pub actor_user_id: Option<i64>,
    pub exclude_actor: bool,
    pub read_by: Vec<i64>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationView {
    pub id: Uuid,
    pub event_type: String,
    pub title: String,
    pub message: String,
    pub severity: String,
    pub action_url: Option<String>,
    pub unread: bool,
    pub created_at: DateTime<Utc>,
}

impl Notification {
    pub fn to_view(&self, user_id: i64) -> NotificationView {
        NotificationView {
            id: self.id,
            event_type: self.event_type.clone(),
            title: self.title.clone(),
            message: self.message.clone(),
            severity: self.severity.clone(),
            action_url: self.action_url.clone(),
            unread: !self.read_by.contains(&user_id),
            created_at: self.created_at,
        }
    }

    pub fn matches_user(&self, claims: &Claims) -> bool {
        if self.exclude_actor {
            if let Some(actor_id) = self.actor_user_id {
                if actor_id == claims.user_id {
                    return false;
                }
            }
        }

        if !self.target_user_ids.is_empty() {
            return self.target_user_ids.contains(&claims.user_id);
        }

        let role_matched = if self.target_roles.is_empty() {
            false
        } else {
            self.target_roles
                .iter()
                .any(|r| r.eq_ignore_ascii_case(claims.role.trim()))
        };

        let branch_matched = if self.target_branches.is_empty() {
            false
        } else if claims.role.eq_ignore_ascii_case("admin") {
            true
        } else {
            claims
                .branch
                .as_deref()
                .map(|ub| {
                    let ub_norm = ub.trim().to_lowercase();
                    self.target_branches
                        .iter()
                        .any(|tb| tb.trim().to_lowercase() == ub_norm)
                })
                .unwrap_or(false)
        };

        if !self.target_roles.is_empty() && !self.target_branches.is_empty() {
            return role_matched || branch_matched;
        }
        if !self.target_roles.is_empty() {
            return role_matched;
        }
        if !self.target_branches.is_empty() {
            return branch_matched;
        }

        true
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateNotificationRequest {
    pub event_type: String,
    pub title: String,
    pub message: String,
    pub severity: Option<String>,
    pub action_url: Option<String>,
    #[serde(default)]
    pub target_user_ids: Vec<i64>,
    #[serde(default)]
    pub target_branches: Vec<String>,
    #[serde(default)]
    pub target_roles: Vec<String>,
    pub actor_user_id: Option<i64>,
    #[serde(default = "default_exclude_actor")]
    pub exclude_actor: bool,
}

fn default_exclude_actor() -> bool {
    true
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationListResponse {
    pub notifications: Vec<NotificationView>,
    pub unread_count: usize,
}
