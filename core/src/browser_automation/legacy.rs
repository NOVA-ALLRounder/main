use super::*;

use anyhow::Context;

pub fn open_url_in_chrome(url: &str) -> Result<()> {
    get_browser_automation().navigate(url, Some("Google Chrome"))
}

pub fn scroll_page(pixels: i32) -> Result<()> {
    let direction = if pixels > 0 { "down" } else { "up" };
    let amount = pixels.abs();
    let script = format!(
        r#"tell application "System Events" to scroll {} by {}"#,
        direction, amount
    );

    std::process::Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .output()
        .context("Failed to scroll")?;

    Ok(())
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
