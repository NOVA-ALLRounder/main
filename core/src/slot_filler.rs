use crate::nl_automation::{IntentType, SlotFillResult, SlotMap};

pub fn fill_slots(intent: &IntentType, mut slots: SlotMap) -> SlotFillResult {
    let required = required_slots(intent);
    let missing: Vec<String> = required
        .into_iter()
        .filter(|key| {
            let k = *key;
            !slots.contains_key(k) || slots.get(k).map(|v| v.is_empty()).unwrap_or(true)
        })
        .map(|s| s.to_string())
        .collect();

    let follow_up = if missing.is_empty() {
        None
    } else {
        Some(build_follow_up(intent, &missing))
    };

    // Normalize: trim slot values
    for value in slots.values_mut() {
        *value = value.trim().to_string();
    }

    SlotFillResult {
        slots,
        missing,
        follow_up,
    }
}

fn required_slots(intent: &IntentType) -> Vec<&'static str> {
    match intent {
        IntentType::FlightSearch => vec!["from", "to", "date_start"],
        IntentType::ShoppingCompare => vec!["product_name"],
        IntentType::FormFill => vec!["target_url", "form_purpose"],
        IntentType::GenericTask => vec![],
    }
}

fn build_follow_up(intent: &IntentType, missing: &[String]) -> String {
    match intent {
        IntentType::FlightSearch => {
            let mut parts = Vec::new();
            if missing.contains(&"from".to_string()) {
                parts.push("출발지를 알려줘");
            }
            if missing.contains(&"to".to_string()) {
                parts.push("도착지를 알려줘");
            }
            if missing.contains(&"date_start".to_string()) {
                parts.push("출발 날짜를 알려줘");
            }
            format!("항공권 검색을 위해 {}", parts.join(", "))
        }
        IntentType::ShoppingCompare => "찾을 제품명을 알려줘".to_string(),
        IntentType::FormFill => {
            let mut parts = Vec::new();
            if missing.contains(&"target_url".to_string()) {
                parts.push("작성할 폼 URL을 알려줘");
            }
            if missing.contains(&"form_purpose".to_string()) {
                parts.push("작성 목적을 알려줘");
            }
            format!("웹폼 자동작성을 위해 {}", parts.join(", "))
        }
        IntentType::GenericTask => "추가 정보를 알려줘".to_string(),
    }
}
