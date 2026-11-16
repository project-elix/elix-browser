use serde::Serialize;
use std::{collections::VecDeque, sync::Mutex};
#[derive(Clone, Serialize)]
pub struct Log {
    pub time: String,
    pub level: String,
    pub code: String,
}
#[derive(Default)]
pub struct Logs(pub Mutex<VecDeque<Log>>);
impl Logs {
    // Only fixed application event codes are accepted at callsites. Never URLs, paths, tokens or page errors.
    pub fn add(&self, level: &str, code: &str) {
        if let Ok(mut q) = self.0.lock() {
            q.push_back(Log {
                time: chrono::Utc::now().to_rfc3339(),
                level: level.into(),
                code: code.into(),
            });
            while q.len() > 250 {
                q.pop_front();
            }
        }
    }
    pub fn list(&self) -> Vec<Log> {
        self.0
            .lock()
            .map(|q| q.iter().cloned().collect())
            .unwrap_or_default()
    }
}
