use std::{collections::VecDeque, path::PathBuf};
use tokio::sync::{broadcast, RwLock};
use uuid::Uuid;

use crate::models::{Claims, Notification, NotificationView};

const MAX_NOTIFICATIONS: usize = 2000;

pub struct NotificationStore {
    items: RwLock<VecDeque<Notification>>,
    data_file: PathBuf,
    tx: broadcast::Sender<Notification>,
}

impl NotificationStore {
    pub fn new(data_file: PathBuf) -> Self {
        let initial = Self::load_from_disk(&data_file);
        let (tx, _) = broadcast::channel(512);
        Self {
            items: RwLock::new(initial),
            data_file,
            tx,
        }
    }

    fn load_from_disk(path: &PathBuf) -> VecDeque<Notification> {
        if !path.exists() {
            return VecDeque::new();
        }
        match std::fs::read_to_string(path) {
            Ok(content) => serde_json::from_str::<VecDeque<Notification>>(&content)
                .unwrap_or_else(|e| {
                    tracing::warn!("Failed to parse {:?}: {}", path, e);
                    VecDeque::new()
                }),
            Err(e) => {
                tracing::warn!("Failed to read {:?}: {}", path, e);
                VecDeque::new()
            }
        }
    }

    fn persist_to_disk(&self, items: &VecDeque<Notification>) {
        if let Some(parent) = self.data_file.parent() {
            if !parent.as_os_str().is_empty() {
                let _ = std::fs::create_dir_all(parent);
            }
        }
        match serde_json::to_string_pretty(items) {
            Ok(json) => {
                let tmp = self.data_file.with_extension("json.tmp");
                if std::fs::write(&tmp, json).is_ok() {
                    let _ = std::fs::rename(&tmp, &self.data_file);
                }
            }
            Err(e) => tracing::error!("Failed to serialize notifications: {}", e),
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Notification> {
        self.tx.subscribe()
    }

    pub async fn push(&self, notification: Notification) -> Notification {
        {
            let mut guard = self.items.write().await;
            guard.push_front(notification.clone());
            while guard.len() > MAX_NOTIFICATIONS {
                guard.pop_back();
            }
            self.persist_to_disk(&guard);
        }
        let _ = self.tx.send(notification.clone());
        notification
    }

    pub async fn list_for_user(&self, claims: &Claims, limit: usize) -> (Vec<NotificationView>, usize) {
        let guard = self.items.read().await;
        let matching: Vec<NotificationView> = guard
            .iter()
            .filter(|n| n.matches_user(claims))
            .map(|n| n.to_view(claims.user_id))
            .collect();

        let unread_count = matching.iter().filter(|n| n.unread).count();
        let truncated = matching.into_iter().take(limit).collect();
        (truncated, unread_count)
    }

    pub async fn mark_read(&self, id: Uuid, claims: &Claims) -> bool {
        let mut guard = self.items.write().await;
        let mut changed = false;
        if let Some(item) = guard.iter_mut().find(|n| n.id == id && n.matches_user(claims)) {
            if !item.read_by.contains(&claims.user_id) {
                item.read_by.push(claims.user_id);
                changed = true;
            }
        }
        if changed {
            self.persist_to_disk(&guard);
        }
        changed
    }

    pub async fn mark_all_read(&self, claims: &Claims) -> usize {
        let mut guard = self.items.write().await;
        let mut count = 0;
        for item in guard.iter_mut() {
            if item.matches_user(claims) && !item.read_by.contains(&claims.user_id) {
                item.read_by.push(claims.user_id);
                count += 1;
            }
        }
        if count > 0 {
            self.persist_to_disk(&guard);
        }
        count
    }
}
