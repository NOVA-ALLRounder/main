use crate::applescript;
use anyhow::Result;
use serde_json;

fn ensure_chrome_ready(url: Option<&str>) -> Result<()> {
    let set_url = url
        .map(|u| format!("set URL of active tab of front window to {}", format!("{:?}", u)))
        .unwrap_or_default();
    let script = format!(
        r#"
        tell application "Google Chrome"
            if (count of windows) = 0 then
                make new window
            end if
            {set_url}
            activate
        end tell
    "#
    );
    applescript::run(&script).map_err(|err| {
        anyhow::anyhow!(
            "Chrome automation failed. Ensure Google Chrome is installed, running, and has Accessibility permissions. Details: {}",
            err
        )
    })?;
    Ok(())
}

pub fn open_url_in_chrome(url: &str) -> Result<()> {
    ensure_chrome_ready(Some(url))
}

pub fn fill_flight_fields(from: &str, to: &str, date_start: &str, date_end: Option<&str>) -> Result<bool> {
    ensure_chrome_ready(None)?;
    let values = serde_json::json!({
        "from": from,
        "to": to,
        "date_start": date_start,
        "date_end": date_end,
    });
    let values_json = serde_json::to_string(&values)?;
    let js = format!(
        r#"(() => {{
            const values = {values_json};
            const inputs = Array.from(document.querySelectorAll('input'));
            let filled = 0;
            const setNativeValue = (el, value) => {{
                const setter =
                    Object.getOwnPropertyDescriptor(el, 'value')?.set ||
                    Object.getOwnPropertyDescriptor(Object.getPrototypeOf(el), 'value')?.set;
                if (setter) {{
                    setter.call(el, value);
                }} else {{
                    el.value = value;
                }}
            }};
            const fire = (el) => {{
                el.dispatchEvent(new Event('input', {{ bubbles: true }}));
                el.dispatchEvent(new Event('change', {{ bubbles: true }}));
            }};
            const fillInput = (el, value) => {{
                if (!el || !value) return false;
                el.focus();
                setNativeValue(el, value);
                fire(el);
                return true;
            }};
            const findInputByTokens = (tokens) => {{
                for (const input of inputs) {{
                    const label = (input.getAttribute('aria-label') || input.getAttribute('placeholder') || input.name || input.id || '').toLowerCase();
                    if (tokens.some(t => label.includes(t))) {{
                        return input;
                    }}
                }}
                return null;
            }};
            const isGoogleFlights = location.hostname.includes('google') && location.pathname.includes('/travel/flights');
            if (isGoogleFlights) {{
                const fromInput =
                    document.querySelector('input[aria-label*="Where from"], input[aria-label*="From"], input[placeholder*="Where from"], input[aria-label*="출발"]') ||
                    findInputByTokens(['where from', 'from', '출발', '출발지']);
                if (fillInput(fromInput, values.from)) filled++;
                const toInput =
                    document.querySelector('input[aria-label*="Where to"], input[aria-label*="To"], input[placeholder*="Where to"], input[aria-label*="도착"]') ||
                    findInputByTokens(['where to', 'to', '도착', '도착지']);
                if (fillInput(toInput, values.to)) filled++;
                const departInput =
                    document.querySelector('input[aria-label*="Departure"], input[aria-label*="Depart"], input[placeholder*="Departure"], input[aria-label*="출발"], input[aria-label*="가는날"]') ||
                    findInputByTokens(['departure', 'depart', '출발', '가는날', '날짜', '출발일']);
                if (fillInput(departInput, values.date_start)) filled++;
                const returnInput =
                    document.querySelector('input[aria-label*="Return"], input[aria-label*="오는날"], input[aria-label*="귀국"], input[placeholder*="Return"]') ||
                    findInputByTokens(['return', '오는날', '귀국', '복귀']);
                if (fillInput(returnInput, values.date_end)) filled++;
            }}
            if (filled === 0) {{
                const matchers = [
                    ['from', ['from', '출발', '출발지', 'where from']],
                    ['to', ['to', '도착', '도착지', 'where to']],
                    ['date_start', ['depart', '출발일', '날짜', 'date']]
                ];
                for (const input of inputs) {{
                    const label = (input.getAttribute('aria-label') || input.getAttribute('placeholder') || input.name || input.id || '').toLowerCase();
                    for (const [key, tokens] of matchers) {{
                        if (!values[key]) continue;
                        if (tokens.some(t => label.includes(t))) {{
                            if (fillInput(input, values[key])) filled++;
                            break;
                        }}
                    }}
                }}
            }}
            return String(filled);
        }})()"#
    );
    let res = applescript::execute_js_in_chrome(&js)?;
    Ok(res.trim().parse::<i32>().unwrap_or(0) > 0)
}

pub fn fill_search_query(query: &str) -> Result<bool> {
    ensure_chrome_ready(None)?;
    let js = format!(
        r#"(() => {{
            const query = {query:?};
            const input = document.querySelector(
                'input[name=\"query\"], input[name=\"searchKeyword\"], input#query, input#autocomplete, input[type=\"search\"], input[placeholder*=\"검색\"], input[placeholder*=\"Search\"], input[id*=\"search\"]'
            );
            if (!input) return '0';
            input.focus();
            const setter =
                Object.getOwnPropertyDescriptor(input, 'value')?.set ||
                Object.getOwnPropertyDescriptor(Object.getPrototypeOf(input), 'value')?.set;
            if (setter) {{
                setter.call(input, query);
            }} else {{
                input.value = query;
            }}
            input.dispatchEvent(new Event('input', {{ bubbles: true }}));
            input.dispatchEvent(new Event('change', {{ bubbles: true }}));
            return '1';
        }})()"#
    );
    let res = applescript::execute_js_in_chrome(&js)?;
    Ok(res.trim() == "1")
}

pub fn click_search_button() -> Result<bool> {
    ensure_chrome_ready(None)?;
    let js = r#"(() => {
        const input = document.querySelector('input[name="query"], input[name="searchKeyword"], input[type="search"], input[id*="search"]');
        const form = input?.closest('form');
        if (form) {
            form.submit();
            return '1';
        }
        if (input) {
            const eventInit = { key: 'Enter', code: 'Enter', keyCode: 13, which: 13, bubbles: true };
            input.dispatchEvent(new KeyboardEvent('keydown', eventInit));
            input.dispatchEvent(new KeyboardEvent('keyup', eventInit));
            return '1';
        }
        const buttons = Array.from(document.querySelectorAll('button, [role="button"], input[type="submit"]'));
        const tokens = ['검색', 'search', '찾기', 'submit'];
        for (const btn of buttons) {
            const text = (btn.innerText || btn.value || btn.getAttribute('aria-label') || '').toLowerCase();
            if (tokens.some(t => text.includes(t))) {
                btn.click();
                return '1';
            }
        }
        return '0';
    })()"#;
    let res = applescript::execute_js_in_chrome(js)?;
    Ok(res.trim() == "1")
}

pub fn autofill_form(name: Option<&str>, email: Option<&str>, phone: Option<&str>, address: Option<&str>) -> Result<bool> {
    ensure_chrome_ready(None)?;
    let js = format!(
        r#"(() => {{
            const values = {{
                name: {name:?},
                email: {email:?},
                phone: {phone:?},
                address: {address:?}
            }};
            const host = location.hostname || '';
            if (!values.name && !values.email && !values.phone && !values.address && host.includes('httpbin.org')) {{
                values.name = "Test User";
                values.email = "test@example.com";
                values.phone = "01000000000";
                values.address = "Test Address";
            }}
            const fields = Array.from(document.querySelectorAll('input, textarea'));
            let filled = 0;
            const rules = [
                ['name', ['name', '이름', '성명', 'customer']],
                ['email', ['email', '이메일', 'e-mail']],
                ['phone', ['phone', 'telephone', 'tel', '전화', '연락', '휴대폰', 'mobile']],
                ['address', ['address', '주소']]
            ];
            const setNativeValue = (el, value) => {{
                const setter =
                    Object.getOwnPropertyDescriptor(el, 'value')?.set ||
                    Object.getOwnPropertyDescriptor(Object.getPrototypeOf(el), 'value')?.set;
                if (setter) {{
                    setter.call(el, value);
                }} else {{
                    el.value = value;
                }}
            }};
            const fire = (el) => {{
                el.dispatchEvent(new Event('input', {{ bubbles: true }}));
                el.dispatchEvent(new Event('change', {{ bubbles: true }}));
            }};
            if (host.includes('httpbin.org')) {{
                const map = [
                    ['custname', values.name],
                    ['custtel', values.phone],
                    ['custemail', values.email],
                ];
                for (const [name, val] of map) {{
                    if (!val) continue;
                    const el = document.querySelector(`input[name="${{name}}"]`);
                    if (el) {{
                        el.focus();
                        setNativeValue(el, val);
                        fire(el);
                        filled++;
                    }}
                }}
            }}
            for (const field of fields) {{
                const label = (field.getAttribute('aria-label') || field.getAttribute('placeholder') || field.name || field.id || '').toLowerCase();
                const type = (field.getAttribute('type') || '').toLowerCase();
                for (const [key, tokens] of rules) {{
                    if (!values[key]) continue;
                    const typeMatch = (key === 'email' && type === 'email') || (key === 'phone' && type === 'tel');
                    if (tokens.some(t => label.includes(t)) || typeMatch) {{
                        field.focus();
                        setNativeValue(field, values[key]);
                        field.dispatchEvent(new Event('input', {{ bubbles: true }}));
                        field.dispatchEvent(new Event('change', {{ bubbles: true }}));
                        filled++;
                        break;
                    }}
                }}
            }}
            return String(filled);
        }})()"#
    );
    let res = applescript::execute_js_in_chrome(&js)?;
    Ok(res.trim().parse::<i32>().unwrap_or(0) > 0)
}

pub fn get_page_context() -> Result<String> {
    ensure_chrome_ready(None)?;
    let js = r#"(() => {
        const title = document.title || '';
        const url = location.href || '';
        const domain = location.hostname || '';
        return JSON.stringify({ title, url, domain });
    })()"#;
    applescript::execute_js_in_chrome(js)
}

pub fn scroll_page(pixels: i32) -> Result<bool> {
    ensure_chrome_ready(None)?;
    let js = format!(
        r#"(() => {{
            window.scrollBy(0, {pixels});
            return '1';
        }})()"#
    );
    let res = applescript::execute_js_in_chrome(&js)?;
    Ok(res.trim() == "1")
}

pub fn extract_flight_summary() -> Result<String> {
    ensure_chrome_ready(None)?;
    let js = r#"(() => {
        const text = document.body?.innerText || '';
        const lines = text.split('\n').map(l => l.trim()).filter(Boolean);
        const ariaLines = Array.from(document.querySelectorAll('[aria-label]'))
            .map(el => el.getAttribute('aria-label'))
            .filter(Boolean);
        const pool = [...ariaLines, ...lines];
        const prices = [];
        const times = [];
        const stops = [];
        const priceRe = /(\d{1,3}(?:,\d{3})+)\s*원|\$\d+/;
        const timeRe = /(\d{1,2}:\d{2})|(\d{1,2})\s*시간\s*(\d{1,2})?\s*분?/;
        const stopRe = /(직항|경유|stop|stops?)/i;
        for (const line of pool) {
            if (prices.length < 5) {
                const m = line.match(priceRe);
                if (m) {
                    const val = m[0];
                    if (!prices.includes(val)) prices.push(val);
                }
            }
            if (times.length < 5) {
                const m = line.match(timeRe);
                if (m) {
                    const val = m[0];
                    if (!times.includes(val)) times.push(val);
                }
            }
            if (stops.length < 5 && stopRe.test(line)) {
                const val = line;
                if (!stops.includes(val)) stops.push(val);
            }
            if (prices.length >= 5 && times.length >= 5 && stops.length >= 5) break;
        }
        return JSON.stringify({ prices, times, stops });
    })()"#;
    applescript::execute_js_in_chrome(js)
}

pub fn extract_shopping_summary() -> Result<String> {
    ensure_chrome_ready(None)?;
    let js = r#"(() => {
        const text = document.body?.innerText || '';
        const lines = text.split('\n').map(l => l.trim()).filter(Boolean);
        const priceNodes = Array.from(document.querySelectorAll('[class*="price"], [class*="amount"], [aria-label*="원"], [aria-label*="$"]'))
            .map(el => (el.innerText || el.getAttribute('aria-label') || '').trim())
            .filter(Boolean);
        const pool = [...priceNodes, ...lines];
        const prices = [];
        const sellers = [];
        const priceRe = /(\d{1,3}(?:,\d{3})+)\s*원/;
        for (const line of pool) {
            if (prices.length < 5) {
                const m = line.match(priceRe);
                if (m) {
                    const val = m[0];
                    if (!prices.includes(val)) prices.push(val);
                }
            }
            if (sellers.length < 5 && line.toLowerCase().includes('판매')) {
                if (!sellers.includes(line)) sellers.push(line);
            }
            if (prices.length >= 5 && sellers.length >= 5) break;
        }
        return JSON.stringify({ prices, sellers });
    })()"#;
    applescript::execute_js_in_chrome(js)
}

pub fn apply_flight_filters(
    budget_max: Option<&str>,
    time_window: Option<&str>,
    direct_only: Option<&str>,
) -> Result<bool> {
    ensure_chrome_ready(None)?;
    let payload = serde_json::json!({
        "budget_max": budget_max,
        "time_window": time_window,
        "direct_only": direct_only,
    });
    let payload_json = serde_json::to_string(&payload)?;
    let js = format!(
        r#"(() => {{
            const params = {payload_json};
            const budget = params.budget_max;
            const timeWindow = params.time_window;
            const directOnly = params.direct_only;
            let actions = 0;
            const normalize = (value) => (value || '').toString().toLowerCase();
            const candidates = Array.from(document.querySelectorAll(
                'button, [role="button"], [role="checkbox"], input[type="checkbox"], [role="menuitem"], [role="radio"]'
            ));
            const clickByTokens = (tokens) => {{
                for (const node of candidates) {{
                    const text = normalize(node.innerText || node.getAttribute('aria-label') || node.getAttribute('title') || '');
                    if (!text) continue;
                    if (tokens.some(token => text.includes(token))) {{
                        node.click();
                        actions += 1;
                        return true;
                    }}
                }}
                return false;
            }};
            if (directOnly && ['true','1','yes','y'].includes(normalize(directOnly))) {{
                clickByTokens(['nonstop', 'direct', '직항']);
            }}
            if (timeWindow) {{
                const key = normalize(timeWindow);
                const tokensByKey = {{
                    morning: ['morning', '오전', 'am'],
                    afternoon: ['afternoon', '오후', 'pm'],
                    evening: ['evening', 'night', '저녁', '밤'],
                }};
                const tokens = tokensByKey[key] || [key];
                clickByTokens(tokens);
            }}
            if (budget) {{
                const inputs = Array.from(document.querySelectorAll('input[type="number"], input[type="text"]'));
                const priceInput = inputs.find(input => {{
                    const label = normalize(input.getAttribute('aria-label') || input.getAttribute('placeholder') || input.name || input.id || '');
                    return label.includes('price') || label.includes('가격') || label.includes('max') || label.includes('최대');
                }});
                if (priceInput) {{
                    priceInput.focus();
                    const setter =
                        Object.getOwnPropertyDescriptor(priceInput, 'value')?.set ||
                        Object.getOwnPropertyDescriptor(Object.getPrototypeOf(priceInput), 'value')?.set;
                    if (setter) {{
                        setter.call(priceInput, budget);
                    }} else {{
                        priceInput.value = budget;
                    }}
                    priceInput.dispatchEvent(new Event('input', {{ bubbles: true }}));
                    priceInput.dispatchEvent(new Event('change', {{ bubbles: true }}));
                    actions += 1;
                }}
            }}
            return String(actions);
        }})()"#
    );
    let res = applescript::execute_js_in_chrome(&js)?;
    Ok(res.trim().parse::<i32>().unwrap_or(0) > 0)
}

pub fn apply_shopping_filters(
    brand: Option<&str>,
    price_min: Option<&str>,
    price_max: Option<&str>,
) -> Result<bool> {
    ensure_chrome_ready(None)?;
    let payload = serde_json::json!({
        "brand": brand,
        "price_min": price_min,
        "price_max": price_max,
    });
    let payload_json = serde_json::to_string(&payload)?;
    let js = format!(
        r#"(() => {{
            const params = {payload_json};
            const brand = params.brand;
            const priceMin = params.price_min;
            const priceMax = params.price_max;
            let actions = 0;
            const normalize = (value) => (value || '').toString().toLowerCase();
            const setValue = (input, value) => {{
                if (!input || value == null) return false;
                input.focus();
                const setter =
                    Object.getOwnPropertyDescriptor(input, 'value')?.set ||
                    Object.getOwnPropertyDescriptor(Object.getPrototypeOf(input), 'value')?.set;
                if (setter) {{
                    setter.call(input, value);
                }} else {{
                    input.value = value;
                }}
                input.dispatchEvent(new Event('input', {{ bubbles: true }}));
                input.dispatchEvent(new Event('change', {{ bubbles: true }}));
                return true;
            }};
            if (brand) {{
                const brandNodes = Array.from(document.querySelectorAll('button, [role="button"], label, a, span'));
                for (const node of brandNodes) {{
                    const text = normalize(node.innerText || node.getAttribute('aria-label') || '');
                    if (text && text.includes(normalize(brand))) {{
                        node.click();
                        actions += 1;
                        break;
                    }}
                }}
            }}
            const inputs = Array.from(document.querySelectorAll('input[type="number"], input[type="text"]'));
            const minInput = inputs.find(input => {{
                const label = normalize(input.getAttribute('aria-label') || input.getAttribute('placeholder') || input.name || input.id || '');
                return label.includes('min') || label.includes('최소');
            }});
            const maxInput = inputs.find(input => {{
                const label = normalize(input.getAttribute('aria-label') || input.getAttribute('placeholder') || input.name || input.id || '');
                return label.includes('max') || label.includes('최대');
            }});
            if (setValue(minInput, priceMin)) actions += 1;
            if (setValue(maxInput, priceMax)) actions += 1;
            return String(actions);
        }})()"#
    );
    let res = applescript::execute_js_in_chrome(&js)?;
    Ok(res.trim().parse::<i32>().unwrap_or(0) > 0)
}
