// User context tracking

use serde::{Deserialize, Serialize};
use std::time::Instant;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserContext {
    pub active_app: Option<String>,
    pub active_window_title: Option<String>,
    #[serde(skip, default = "instant_now")]
    pub last_activity: Instant,
    pub activity_count: u32, // Events in last 1 minute
}

fn instant_now() -> Instant {
    Instant::now()
}

impl Default for UserContext {
    fn default() -> Self {
        Self {
            active_app: None,
            active_window_title: None,
            last_activity: Instant::now(),
            activity_count: 0,
        }
    }
}

impl UserContext {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn update_app(&mut self, app: String) {
        self.active_app = Some(app);
        self.last_activity = Instant::now();
        self.activity_count += 1;
    }

    pub fn update_window(&mut self, title: String) {
        self.active_window_title = Some(title);
        self.last_activity = Instant::now();
    }

    pub fn increment_activity(&mut self) {
        self.activity_count += 1;
        self.last_activity = Instant::now();
    }

    pub fn reset_activity_count(&mut self) {
        self.activity_count = 0;
    }

    pub fn is_idle(&self, idle_threshold_secs: u64) -> bool {
        self.last_activity.elapsed().as_secs() > idle_threshold_secs
    }
}
