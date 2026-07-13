use crate::ai::config::AiConfigManager;
use crate::http_client::HttpClient;
use crate::vault::TokenVault;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::task::AbortHandle;

struct AiTaskEntry {
    generation: u64,
    abort_handle: AbortHandle,
}

pub struct AiTaskRegistry {
    tasks: Mutex<HashMap<String, AiTaskEntry>>,
    next_generation: AtomicU64,
}

impl AiTaskRegistry {
    fn new() -> Self {
        Self { tasks: Mutex::new(HashMap::new()), next_generation: AtomicU64::new(1) }
    }

    pub fn next_generation(&self) -> u64 {
        self.next_generation.fetch_add(1, Ordering::Relaxed)
    }

    pub async fn replace(&self, request_id: String, generation: u64, abort_handle: AbortHandle) {
        if let Some(previous) = self.tasks.lock().await.insert(request_id, AiTaskEntry { generation, abort_handle }) {
            previous.abort_handle.abort();
        }
    }

    pub async fn cancel(&self, request_id: &str) {
        if let Some(task) = self.tasks.lock().await.remove(request_id) {
            task.abort_handle.abort();
        }
    }

    pub async fn has_active_tasks(&self) -> bool {
        !self.tasks.lock().await.is_empty()
    }

    pub async fn remove_if_current(&self, request_id: &str, generation: u64) {
        let mut tasks = self.tasks.lock().await;
        if tasks.get(request_id).is_some_and(|entry| entry.generation == generation) {
            tasks.remove(request_id);
        }
    }
}

/// Application state shared across all Tauri commands
pub struct AppState {
    pub http_client: Arc<HttpClient>,
    pub token_vault: Arc<TokenVault>,
    pub ai_config: Arc<AiConfigManager>,
    pub ai_tasks: Arc<AiTaskRegistry>,
    pub update_operation_active: AtomicBool,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            http_client: Arc::new(HttpClient::new()),
            token_vault: Arc::new(TokenVault::new()),
            ai_config: Arc::new(AiConfigManager::new()),
            ai_tasks: Arc::new(AiTaskRegistry::new()),
            update_operation_active: AtomicBool::new(false),
        }
    }
}
