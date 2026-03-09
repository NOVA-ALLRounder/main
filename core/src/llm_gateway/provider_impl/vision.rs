use super::*;

pub(crate) async fn analyze_screen(
    client: &OpenAILLMClient,
    prompt: &str,
    image_b64: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let vision_model = client.vision_model();
    let body = json!({
        "model": vision_model,
        "messages": [
            {
                "role": "user",
                "content": [
                    { "type": "text", "text": prompt },
                    {
                        "type": "image_url",
                        "image_url": {
                            "url": format!("data:image/jpeg;base64,{}", image_b64)
                        }
                    }
                ]
            }
        ],
        "max_tokens": 500
    });

    let res_json = client
        .post_with_retry("https://api.openai.com/v1/chat/completions", &body)
        .await?;

    if let Some(err) = res_json.get("error") {
        return Err(anyhow::anyhow!("OpenAI API Error: {:?}", err).into());
    }

    let content = res_json["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("")
        .to_string();

    Ok(content)
}

pub(crate) async fn find_element_coordinates(
    client: &OpenAILLMClient,
    element_description: &str,
    image_b64: &str,
) -> Result<Option<(i32, i32)>> {
    let system_prompt = r#"
        You are a Screen Coordinate Locator.
        Analyze the screenshot and find the generic center coordinates (x, y) of the UI element described by the user.

        Output JSON ONLY:
        {
          "thinking": "Briefly describe the element's location (e.g. 'Found Blue button in top right')",
          "found": true,
          "x": 123,
          "y": 456
        }
        or
        { "found": false }

        DO NOT output markdown.
        "#;

    let user_msg = format!("Find this element: {}", element_description);

    let vision_model = client.vision_model();
    let body = json!({
        "model": vision_model,
        "messages": [
            { "role": "system", "content": system_prompt },
            {
                "role": "user",
                "content": [
                    { "type": "text", "text": user_msg },
                    {
                        "type": "image_url",
                        "image_url": {
                            "url": format!("data:image/jpeg;base64,{}", image_b64)
                        }
                    }
                ]
            }
        ],
        "max_tokens": 100,
        "response_format": { "type": "json_object" }
    });

    let res_json = client
        .post_with_retry("https://api.openai.com/v1/chat/completions", &body)
        .await?;
    let content = res_json["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("{}");

    let parsed: Value = serde_json::from_str(content)?;

    if parsed["found"].as_bool().unwrap_or(false) {
        let x = parsed["x"].as_i64().unwrap_or(0) as i32;
        let y = parsed["y"].as_i64().unwrap_or(0) as i32;
        Ok(Some((x, y)))
    } else {
        Ok(None)
    }
}

pub(crate) async fn score_quality(
    client: &OpenAILLMClient,
    system_prompt: &str,
    payload: &serde_json::Value,
) -> Result<String> {
    let body = json!({
        "model": client.model,
        "messages": [
            { "role": "system", "content": system_prompt },
            { "role": "user", "content": payload.to_string() }
        ],
        "temperature": 0.2,
        "response_format": { "type": "json_object" }
    });

    let body = client
        .post_with_retry("https://api.openai.com/v1/chat/completions", &body)
        .await?;
    let content = body["choices"][0]["message"]["content"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("No content in quality scoring response"))?;
    Ok(content.to_string())
}
