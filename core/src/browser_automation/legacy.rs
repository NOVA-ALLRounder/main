use super::*;

use crate::platform::{app_role_primary_name, current_platform, AppRole};

pub fn open_url_in_chrome(url: &str) -> Result<()> {
    get_browser_automation().navigate(
        url,
        Some(app_role_primary_name(
            current_platform().kind(),
            AppRole::Browser,
        )),
    )
}

pub fn scroll_page(pixels: i32) -> Result<()> {
    current_platform().browser_scroll(pixels)
}

pub fn apply_flight_filters(
    _budget: Option<&str>,
    _time_window: Option<&str>,
    _direct_only: Option<&str>,
) -> Result<bool> {
    println!("⚠️ [Browser] apply_flight_filters: Use new ref-based API instead");
    Ok(false)
}

pub fn apply_shopping_filters(
    _brand: Option<&str>,
    _price_min: Option<&str>,
    _price_max: Option<&str>,
) -> Result<bool> {
    println!("⚠️ [Browser] apply_shopping_filters: Use new ref-based API instead");
    Ok(false)
}

pub fn click_search_button() -> Result<bool> {
    println!("⚠️ [Browser] click_search_button: Use new click_by_ref API instead");
    Ok(false)
}

pub fn get_page_context() -> Result<String> {
    Ok("Page context: Use take_snapshot() for detailed element refs".to_string())
}

pub fn fill_flight_fields(
    _from: &str,
    _to: &str,
    _date_start: &str,
    _date_end: Option<&str>,
) -> Result<bool> {
    println!("⚠️ [Browser] fill_flight_fields: Use new type_text API instead");
    Ok(true)
}

pub fn fill_search_query(query: &str) -> Result<bool> {
    get_browser_automation().type_text(query, 0)?;
    Ok(true)
}

pub fn autofill_form(
    _name: Option<&str>,
    _email: Option<&str>,
    _phone: Option<&str>,
    _address: Option<&str>,
) -> Result<bool> {
    println!("⚠️ [Browser] autofill_form: Use new type_text API instead");
    Ok(true)
}

pub fn extract_flight_summary() -> Result<String> {
    Ok("Flight summary extraction: Use take_snapshot() + find_by_name()".to_string())
}

pub fn extract_shopping_summary() -> Result<String> {
    Ok("Shopping summary extraction: Use take_snapshot() + find_by_name()".to_string())
}
