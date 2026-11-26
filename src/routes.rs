use std::{convert::Infallible, time::Duration};

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse,
    },
    Extension, Json,
};
use chrono::Utc;
use serde::Deserialize;
use serde_json::json;
use tokio_stream::{wrappers::BroadcastStream, Stream, StreamExt};
use uuid::Uuid;

use crate::{
    models::{Claims, CreateNotificationRequest, Notification, NotificationListResponse},
    AppState,
};

#[derive(Debug, Deserialize)]
pub struct ListQuery {
    pub limit: Option<usize>,
}

pub async fn health() -> impl IntoResponse {
    Json(json!({
        "status": "ok",
        "service": "trungnamhub_notify_server"
    }))
}

pub async fn list_notifications(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Query(query): Query<ListQuery>,
) -> impl IntoResponse {
    let limit = query.limit.unwrap_or(50).min(200);
    let (notifications, unread_count) = state.store.list_for_user(&claims, limit).await;
    Json(NotificationListResponse {
        notifications,
        unread_count,
    })
}

pub async fn stream_notifications(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let rx = state.store.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(move |msg| {
        let claims = claims.clone();
        match msg {
            Ok(notification) if notification.matches_user(&claims) => {
                let view = notification.to_view(claims.user_id);
                match serde_json::to_string(&view) {
                    Ok(json) => Some(Ok(Event::default().event("notification").data(json))),
                    Err(_) => None,
                }
            }
            _ => None,
        }
    });

    Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(25))
            .text("ping"),
    )
}

pub async fn mark_read(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    let updated = state.store.mark_read(id, &claims).await;
    Json(json!({ "success": true, "updated": updated }))
}

pub async fn mark_all_read(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> impl IntoResponse {
    let updated_count = state.store.mark_all_read(&claims).await;
    Json(json!({ "success": true, "updatedCount": updated_count }))
}

pub async fn internal_notify(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateNotificationRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    let provided_key = headers
        .get("x-internal-key")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if provided_key != state.internal_key {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let notification = Notification {
        id: Uuid::new_v4(),
        event_type: payload.event_type,
        title: payload.title,
        message: payload.message,
        severity: payload.severity.unwrap_or_else(|| "info".to_string()),
        action_url: payload.action_url,
        target_user_ids: payload.target_user_ids,
        target_branches: payload.target_branches,
        target_roles: payload.target_roles,
        actor_user_id: payload.actor_user_id,
        exclude_actor: payload.exclude_actor,
        read_by: Vec::new(),
        created_at: Utc::now(),
    };

    let saved = state.store.push(notification).await;
    Ok((StatusCode::CREATED, Json(json!({ "success": true, "id": saved.id }))))
}
