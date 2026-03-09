mod browser;
mod parsing;
mod permissions;
mod ui;

pub use browser::{
    extract_google_redirect_target, extract_query_param, frontmost_browser, google_lucky_url,
    google_search_url, is_google_search_goal, is_redirect_alert, prefer_lucky_only,
    wants_first_result,
};
pub use parsing::{
    calculator_has_input, compute_calc_result, extract_best_number, extract_note_title,
    extract_search_query, fetch_stock_price, goal_is_ui_task, goal_mentions_calculation,
    goal_mentions_desktop, goal_mentions_image, goal_mentions_notes, goal_mentions_stock_price,
    infer_stock_symbol, normalize_digits,
};
pub use permissions::{permission_help, preflight_permissions, verify_screen_capture};
pub use ui::{
    compute_plan_key, ensure_app_focus, focus_text_area, goal_primary_app, looks_like_dialog,
    looks_like_subject, resume_hint_for_goal, try_close_front_dialog,
};
