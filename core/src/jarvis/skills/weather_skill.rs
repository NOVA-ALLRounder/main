// WeatherSkill - ?醫롫뎁 鈺곌퀬??(API ???븍뜇釉??
// 筌〓㈇?? skills-main steipete/weather
// wttr.in (??용뮞?? + Open-Meteo (JSON, ?怨멸쉭 ??덈궖)

use super::{Skill, SkillContext, SkillResult};
use crate::jarvis::skills::metadata::*;
use async_trait::async_trait;
use serde_json::json;

pub struct WeatherSkill {
    client: reqwest::Client,
}

impl WeatherSkill {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .unwrap_or_default();
        Self { client }
    }
}

impl Default for WeatherSkill {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Skill for WeatherSkill {
    fn metadata(&self) -> SkillMetadata {
        SkillMetadata {
            name: "weather".to_string(),
            description: "?醫롫뎁 鈺곌퀬?? ?袁⑹삺 ?醫롫뎁, ??덈궖 (API ???븍뜇釉??".to_string(),
            version: "1.0.0".to_string(),
            actions: vec![
                "current".to_string(),
                "forecast".to_string(),
            ],
            requirements: SkillRequirements::default(),
            tags: vec!["weather".to_string(), "daily".to_string()],
        }
    }

    fn check_eligibility(&self) -> EligibilityResult {
        EligibilityResult::eligible()
    }

    async fn execute(&self, ctx: SkillContext) -> SkillResult {
        log::info!("WeatherSkill executing action: {}", ctx.action);

        match ctx.action.as_str() {
            "current" => self.current_weather(ctx).await,
            "forecast" => self.forecast(ctx).await,
            _ => SkillResult::error(format!("Unknown weather action: {}", ctx.action)),
        }
    }
}

impl WeatherSkill {
    /// ?袁⑹삺 ?醫롫뎁 (Open-Meteo API - ?얜?利? ???븍뜇釉??
    async fn current_weather(&self, ctx: SkillContext) -> SkillResult {
        let location = ctx
            .params
            .get("location")
            .and_then(|v| v.as_str())
            .unwrap_or("Seoul");

        // 1??ｍ? Geocoding (?袁⑸뻻筌????袁㏐펾??
        let (lat, lon, resolved_name) = match self.geocode(location).await {
            Ok(coords) => coords,
            Err(e) => return SkillResult::error(format!("?袁⑺뒄 野꺜????쎈솭: {}", e)),
        };

        // 2??ｍ? ?袁⑹삺 ?醫롫뎁 鈺곌퀬??
        let url = format!(
            "https://api.open-meteo.com/v1/forecast?latitude={}&longitude={}&current=temperature_2m,relative_humidity_2m,apparent_temperature,weather_code,wind_speed_10m,wind_direction_10m&timezone=auto",
            lat, lon
        );

        let response = match self.client.get(&url).send().await {
            Ok(r) => r,
            Err(e) => return SkillResult::error(format!("?醫롫뎁 API ?紐꾪뀱 ??쎈솭: {}", e)),
        };

        let data: serde_json::Value = match response.json().await {
            Ok(d) => d,
            Err(e) => return SkillResult::error(format!("?臾먮뼗 ???뼓 ??쎈솭: {}", e)),
        };

        let current = &data["current"];
        let temp = current["temperature_2m"].as_f64().unwrap_or(0.0);
        let feels_like = current["apparent_temperature"].as_f64().unwrap_or(0.0);
        let humidity = current["relative_humidity_2m"].as_f64().unwrap_or(0.0);
        let wind_speed = current["wind_speed_10m"].as_f64().unwrap_or(0.0);
        let weather_code = current["weather_code"].as_i64().unwrap_or(0);

        let weather_desc = weather_code_to_description(weather_code);
        let weather_emoji = weather_code_to_emoji(weather_code);

        SkillResult::success_with_data(
            format!(
                "{} {} {}: {:.1}吏퇒 (筌ｋ떯而?{:.1}吏퇒), ??щ즲 {:.0}%, ??용꺗 {:.1}km/h",
                weather_emoji, resolved_name, weather_desc, temp, feels_like, humidity, wind_speed
            ),
            json!({
                "location": resolved_name,
                "latitude": lat,
                "longitude": lon,
                "temperature": temp,
                "feels_like": feels_like,
                "humidity": humidity,
                "wind_speed": wind_speed,
                "weather_code": weather_code,
                "weather_description": weather_desc,
            }),
        )
    }

    /// 3????덈궖
    async fn forecast(&self, ctx: SkillContext) -> SkillResult {
        let location = ctx
            .params
            .get("location")
            .and_then(|v| v.as_str())
            .unwrap_or("Seoul");
        let days = ctx
            .params
            .get("days")
            .and_then(|v| v.as_u64())
            .unwrap_or(3)
            .min(7) as usize;

        let (lat, lon, resolved_name) = match self.geocode(location).await {
            Ok(coords) => coords,
            Err(e) => return SkillResult::error(format!("?袁⑺뒄 野꺜????쎈솭: {}", e)),
        };

        let url = format!(
            "https://api.open-meteo.com/v1/forecast?latitude={}&longitude={}&daily=weather_code,temperature_2m_max,temperature_2m_min,precipitation_probability_max&timezone=auto&forecast_days={}",
            lat, lon, days
        );

        let response = match self.client.get(&url).send().await {
            Ok(r) => r,
            Err(e) => return SkillResult::error(format!("??덈궖 API ?紐꾪뀱 ??쎈솭: {}", e)),
        };

        let data: serde_json::Value = match response.json().await {
            Ok(d) => d,
            Err(e) => return SkillResult::error(format!("?臾먮뼗 ???뼓 ??쎈솭: {}", e)),
        };

        let daily = &data["daily"];
        let dates = daily["time"].as_array();
        let max_temps = daily["temperature_2m_max"].as_array();
        let min_temps = daily["temperature_2m_min"].as_array();
        let weather_codes = daily["weather_code"].as_array();
        let precip_probs = daily["precipitation_probability_max"].as_array();

        let mut forecasts: Vec<serde_json::Value> = Vec::new();
        let mut summary_lines: Vec<String> = Vec::new();

        if let (Some(dates), Some(maxs), Some(mins), Some(codes)) =
            (dates, max_temps, min_temps, weather_codes)
        {
            for i in 0..dates.len().min(days) {
                let date = dates[i].as_str().unwrap_or("");
                let max_t = maxs[i].as_f64().unwrap_or(0.0);
                let min_t = mins[i].as_f64().unwrap_or(0.0);
                let code = codes[i].as_i64().unwrap_or(0);
                let precip = precip_probs
                    .and_then(|p| p.get(i))
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0);

                let emoji = weather_code_to_emoji(code);
                let desc = weather_code_to_description(code);

                summary_lines.push(format!(
                    "{} {} {} {:.0}吏?{:.0}吏?揶쏅벡??類ｌぇ {:.0}%",
                    date, emoji, desc, min_t, max_t, precip
                ));

                forecasts.push(json!({
                    "date": date,
                    "max_temp": max_t,
                    "min_temp": min_t,
                    "weather_code": code,
                    "weather": desc,
                    "precipitation_probability": precip,
                }));
            }
        }

        SkillResult::success_with_data(
            format!("{} {}????덈궖:\n{}", resolved_name, days, summary_lines.join("\n")),
            json!({
                "location": resolved_name,
                "forecasts": forecasts,
                "days": days,
            }),
        )
    }

    /// Geocoding: ?袁⑸뻻筌???(?袁⑤즲, 野껋럥猷? ??已?
    async fn geocode(&self, location: &str) -> Result<(f64, f64, String), String> {
        // ?癒?폒 ?怨뺣뮉 ?袁⑸뻻 ??롫굡?꾨뗀逾?(API ?紐꾪뀱 ??됰튋)
        let known = match location.to_lowercase().as_str() {
            "??뽰뒻" | "seoul" => Some((37.5665, 126.978, "??뽰뒻")),
            "?봔?? | "busan" => Some((35.1796, 129.0756, "?봔??)),
            "?紐꾩퓝" | "incheon" => Some((37.4563, 126.7052, "?紐꾩퓝")),
            "???? | "daejeon" => Some((36.3504, 127.3845, "????)),
            "???? | "daegu" => Some((35.8714, 128.6014, "????)),
            "?용쵐竊? | "gwangju" => Some((35.1595, 126.8526, "?용쵐竊?)),
            "??뽳폒" | "jeju" => Some((33.4996, 126.5312, "??뽳폒")),
            "tokyo" | "?袁⑺뱳" => Some((35.6762, 139.6503, "?袁⑺뱳")),
            "new york" | "??곸뒅" => Some((40.7128, -74.006, "??곸뒅")),
            "london" | "?怨뺣쐲" => Some((51.5074, -0.1278, "?怨뺣쐲")),
            "san francisco" | "sf" => Some((37.7749, -122.4194, "??곕늄????뽯뮞??)),
            _ => None,
        };

        if let Some((lat, lon, name)) = known {
            return Ok((lat, lon, name.to_string()));
        }

        // Open-Meteo Geocoding API
        let url = format!(
            "https://geocoding-api.open-meteo.com/v1/search?name={}&count=1&language=ko",
            urlencoding::encode(location)
        );

        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("Geocoding API ??쎈솭: {}", e))?;

        let data: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("Geocoding ???뼓 ??쎈솭: {}", e))?;

        let results = data["results"]
            .as_array()
            .ok_or_else(|| format!("'{}' ?袁⑺뒄??筌≪뼚??????곷뮸??덈뼄", location))?;

        if results.is_empty() {
            return Err(format!("'{}' ?袁⑺뒄??筌≪뼚??????곷뮸??덈뼄", location));
        }

        let first = &results[0];
        let lat = first["latitude"].as_f64().ok_or("?袁⑤즲 ??곸벉")?;
        let lon = first["longitude"].as_f64().ok_or("野껋럥猷???곸벉")?;
        let name = first["name"]
            .as_str()
            .unwrap_or(location)
            .to_string();

        Ok((lat, lon, name))
    }
}

/// WMO Weather Code ????볥럢????살구
fn weather_code_to_description(code: i64) -> &'static str {
    match code {
        0 => "筌띾쵐??,
        1 => "??筌ｋ?以?筌띾쵐??,
        2 => "?닌됱カ 鈺곌퀗??,
        3 => "?癒?뵝",
        45 | 48 => "??뉗뻣",
        51 | 53 | 55 => "??곷뮩??,
        56 | 57 => "??????곷뮩??,
        61 | 63 | 65 => "??,
        66 | 67 => "??????,
        71 | 73 | 75 => "??,
        77 => "?紐껋뵭??,
        80 | 81 | 82 => "???돌疫?,
        85 | 86 => "?????돌疫?,
        95 => "???뒭",
        96 | 99 => "?怨뺤뺏 ???뒭",
        _ => "??????곸벉",
    }
}

/// WMO Weather Code ?????덌쭪?
fn weather_code_to_emoji(code: i64) -> &'static str {
    match code {
        0 => "????,
        1 | 2 => "??,
        3 => "?怨삵닔",
        45 | 48 => "??㎪닼?,
        51 | 53 | 55 | 56 | 57 => "??→닼?,
        61 | 63 | 65 | 66 | 67 => "??€닼?,
        71 | 73 | 75 | 77 => "??ｆ닼?,
        80 | 81 | 82 => "??€닼?,
        85 | 86 => "?袁ы닔",
        95 | 96 | 99 => "??뚰닔",
        _ => "??쒏닼?,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metadata() {
        let skill = WeatherSkill::new();
        let meta = skill.metadata();
        assert_eq!(meta.name, "weather");
        assert_eq!(meta.actions.len(), 2);
    }

    #[test]
    fn test_always_eligible() {
        let skill = WeatherSkill::new();
        assert!(skill.check_eligibility().eligible);
    }

    #[test]
    fn test_weather_code_descriptions() {
        assert_eq!(weather_code_to_description(0), "筌띾쵐??);
        assert_eq!(weather_code_to_description(61), "??);
        assert_eq!(weather_code_to_description(71), "??);
        assert_eq!(weather_code_to_description(95), "???뒭");
    }

    #[test]
    fn test_weather_code_emoji() {
        assert_eq!(weather_code_to_emoji(0), "????);
        assert_eq!(weather_code_to_emoji(3), "?怨삵닔");
        assert_eq!(weather_code_to_emoji(61), "??€닼?);
    }
}
