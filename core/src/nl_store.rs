use crate::nl_automation::{IntentResult, Plan, SlotMap};
use lazy_static::lazy_static;
use std::collections::HashMap;
use std::sync::Mutex;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct SessionState {
    pub session_id: String,
    pub intent: IntentResult,
    pub slots: SlotMap,
    pub plan_id: Option<String>,
    pub prompt: String,
}

lazy_static! {
    static ref SESSION_STORE: Mutex<HashMap<String, SessionState>> = Mutex::new(HashMap::new());
    static ref PLAN_STORE: Mutex<HashMap<String, Plan>> = Mutex::new(HashMap::new());
}

pub fn create_session(intent: IntentResult, slots: SlotMap, prompt: String) -> SessionState {
    let session_id = Uuid::new_v4().to_string();
    let state = SessionState {
        session_id: session_id.clone(),
        intent,
        slots,
        plan_id: None,
        prompt,
    };
    if let Ok(mut store) = SESSION_STORE.lock() {
        store.insert(session_id.clone(), state.clone());
    }
    state
}

pub fn get_session(session_id: &str) -> Option<SessionState> {
    SESSION_STORE
        .lock()
        .ok()
        .and_then(|store| store.get(session_id).cloned())
}

pub fn update_session_slots(session_id: &str, updates: &SlotMap) -> Option<SessionState> {
    let mut store = SESSION_STORE.lock().ok()?;
    let state = store.get_mut(session_id)?;
    for (k, v) in updates {
        state.slots.insert(k.clone(), v.clone());
    }
    Some(state.clone())
}

pub fn set_session_plan(session_id: &str, plan: Plan) -> Option<SessionState> {
    let mut store = SESSION_STORE.lock().ok()?;
    let state = store.get_mut(session_id)?;
    state.plan_id = Some(plan.plan_id.clone());
    if let Ok(mut plans) = PLAN_STORE.lock() {
        plans.insert(plan.plan_id.clone(), plan);
    }
    Some(state.clone())
}

pub fn get_plan(plan_id: &str) -> Option<Plan> {
    PLAN_STORE
        .lock()
        .ok()
        .and_then(|store| store.get(plan_id).cloned())
}

pub fn find_session_by_plan(plan_id: &str) -> Option<SessionState> {
    let store = SESSION_STORE.lock().ok()?;
    store
        .values()
        .find(|state| state.plan_id.as_deref() == Some(plan_id))
        .cloned()
}
