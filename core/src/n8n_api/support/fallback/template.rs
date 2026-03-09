use serde_json::{json, Value};

use super::scripts::{
    classify_code, extract_code, idempotency_code, markdown_code, notion_result_code,
    summarize_code, telegram_result_code, validate_code,
};
use super::strings::{compact_prompt_seed, slugify_for_path};

fn env_or_empty(keys: &[&str]) -> String {
    keys.iter()
        .filter_map(|key| std::env::var(key).ok())
        .map(|v| v.trim().to_string())
        .find(|v| !v.is_empty())
        .unwrap_or_default()
}

pub fn build_orchestrator_fallback_workflow(
    name: &str,
    prompt: Option<&str>,
    reason: &str,
) -> Value {
    let webhook_path = format!("steer-{}", slugify_for_path(name));
    let prompt_seed = compact_prompt_seed(prompt);
    let notion_parent_default = env_or_empty(&["NOTION_PAGE_ID"]);
    let notion_token_default = env_or_empty(&["NOTION_TOKEN", "NOTION_API_KEY"]);
    let telegram_chat_default = env_or_empty(&["TELEGRAM_CHAT_ID"]);
    let telegram_bot_default = env_or_empty(&["TELEGRAM_BOT_TOKEN", "TELEGRAM_ACCESS_TOKEN"]);
    let notion_parent_default_json =
        serde_json::to_string(&notion_parent_default).unwrap_or_else(|_| "\"\"".to_string());
    let notion_token_default_json =
        serde_json::to_string(&notion_token_default).unwrap_or_else(|_| "\"\"".to_string());
    let telegram_chat_default_json =
        serde_json::to_string(&telegram_chat_default).unwrap_or_else(|_| "\"\"".to_string());
    let telegram_bot_default_json =
        serde_json::to_string(&telegram_bot_default).unwrap_or_else(|_| "\"\"".to_string());
    let notion_parent_expr =
        "={{$json.body && $json.body.notion_parent_page_id ? $json.body.notion_parent_page_id : ($json.notion_parent_page_id || __DEFAULT__)}}"
            .replace("__DEFAULT__", &notion_parent_default_json);
    let notion_token_expr =
        "={{$json.body && $json.body.notion_token ? $json.body.notion_token : ($json.notion_token || __DEFAULT__)}}"
            .replace("__DEFAULT__", &notion_token_default_json);
    let telegram_chat_expr =
        "={{$json.body && $json.body.telegram_chat_id ? $json.body.telegram_chat_id : ($json.telegram_chat_id || __DEFAULT__)}}"
            .replace("__DEFAULT__", &telegram_chat_default_json);
    let telegram_bot_expr =
        "={{$json.body && $json.body.telegram_bot_token ? $json.body.telegram_bot_token : ($json.telegram_bot_token || __DEFAULT__)}}"
            .replace("__DEFAULT__", &telegram_bot_default_json);

    json!({
        "name": name,
        "nodes": [
            {
                "id": uuid::Uuid::new_v4().to_string(),
                "name": "Manual Trigger",
                "type": "n8n-nodes-base.manualTrigger",
                "typeVersion": 1,
                "position": [-420, -120],
                "parameters": {}
            },
            {
                "id": uuid::Uuid::new_v4().to_string(),
                "name": "ProgramWebhook",
                "type": "n8n-nodes-base.webhook",
                "typeVersion": 2,
                "position": [-420, 120],
                "parameters": {
                    "httpMethod": "POST",
                    "path": webhook_path,
                    "responseMode": "lastNode"
                }
            },
            {
                "id": uuid::Uuid::new_v4().to_string(),
                "name": "Input Envelope",
                "type": "n8n-nodes-base.set",
                "typeVersion": 2,
                "position": [-150, 0],
                "parameters": {
                    "keepOnlySet": false,
                    "values": {
                        "string": [
                            { "name": "request_title", "value": name },
                            { "name": "request_seed", "value": prompt_seed },
                            { "name": "request_text", "value": "={{$json.body && $json.body.prompt ? $json.body.prompt : ($json.body && $json.body.request_text ? $json.body.request_text : ($json.body && $json.body.text ? $json.body.text : ($json.body && $json.body.query ? $json.body.query : ($json.request_text || $json.request_seed || ''))))}}" },
                            { "name": "topic", "value": "={{$json.body && $json.body.topic ? $json.body.topic : ($json.topic || 'AI')}}" },
                            { "name": "timeframe", "value": "={{$json.body && $json.body.timeframe ? $json.body.timeframe : ($json.timeframe || '7d')}}" },
                            { "name": "language", "value": "={{$json.body && $json.body.language ? $json.body.language : ($json.language || 'en')}}" },
                            { "name": "top_n", "value": "={{$json.body && $json.body.top_n ? String($json.body.top_n) : ($json.body && $json.body.limit ? String($json.body.limit) : ($json.top_n || '5'))}}" },
                            { "name": "notion_parent_page_id", "value": notion_parent_expr },
                            { "name": "notion_token", "value": notion_token_expr },
                            { "name": "telegram_chat_id", "value": telegram_chat_expr },
                            { "name": "telegram_bot_token", "value": telegram_bot_expr },
                            { "name": "source_trigger", "value": "={{$json.headers ? 'webhook' : 'manual'}}" },
                            { "name": "orchestrator_version", "value": "steer_fallback_v4" },
                            { "name": "fallback_reason", "value": reason },
                            { "name": "started_at", "value": "={{$now}}" }
                        ]
                    }
                }
            },
            {
                "id": uuid::Uuid::new_v4().to_string(),
                "name": "Idempotency Guard",
                "type": "n8n-nodes-base.code",
                "typeVersion": 2,
                "position": [130, 0],
                "parameters": {
                    "mode": "runOnceForAllItems",
                    "jsCode": idempotency_code()
                }
            },
            {
                "id": uuid::Uuid::new_v4().to_string(),
                "name": "Fetch Trend Feed",
                "type": "n8n-nodes-base.httpRequest",
                "typeVersion": 4.2,
                "position": [390, 0],
                "continueOnFail": true,
                "parameters": {
                    "method": "GET",
                    "url": "={{'https://hn.algolia.com/api/v1/search_by_date?tags=story&query=' + encodeURIComponent($json.topic || 'AI')}}",
                    "options": {
                        "timeout": 12000,
                        "retry": {
                            "maxTries": 3,
                            "waitBetweenTries": 1000
                        }
                    }
                }
            },
            {
                "id": uuid::Uuid::new_v4().to_string(),
                "name": "Extract Top 5 Articles",
                "type": "n8n-nodes-base.code",
                "typeVersion": 2,
                "position": [650, 0],
                "parameters": {
                    "mode": "runOnceForAllItems",
                    "jsCode": extract_code()
                }
            },
            {
                "id": uuid::Uuid::new_v4().to_string(),
                "name": "Classify Trend Alignment",
                "type": "n8n-nodes-base.code",
                "typeVersion": 2,
                "position": [910, 0],
                "parameters": {
                    "mode": "runOnceForAllItems",
                    "jsCode": classify_code()
                }
            },
            {
                "id": uuid::Uuid::new_v4().to_string(),
                "name": "Build Comparative Summary",
                "type": "n8n-nodes-base.code",
                "typeVersion": 2,
                "position": [1170, 0],
                "parameters": {
                    "mode": "runOnceForAllItems",
                    "jsCode": summarize_code()
                }
            },
            {
                "id": uuid::Uuid::new_v4().to_string(),
                "name": "Validate Output Schema",
                "type": "n8n-nodes-base.code",
                "typeVersion": 2,
                "position": [1430, 0],
                "parameters": {
                    "mode": "runOnceForAllItems",
                    "jsCode": validate_code()
                }
            },
            {
                "id": uuid::Uuid::new_v4().to_string(),
                "name": "If Validation OK",
                "type": "n8n-nodes-base.if",
                "typeVersion": 2,
                "position": [1680, 0],
                "parameters": {
                    "conditions": {
                        "string": [
                            {
                                "value1": "={{$json.validation_ok}}",
                                "operation": "equal",
                                "value2": "true"
                            }
                        ]
                    }
                }
            },
            {
                "id": uuid::Uuid::new_v4().to_string(),
                "name": "Build Markdown Report",
                "type": "n8n-nodes-base.code",
                "typeVersion": 2,
                "position": [1930, -120],
                "parameters": {
                    "mode": "runOnceForAllItems",
                    "jsCode": markdown_code()
                }
            },
            {
                "id": uuid::Uuid::new_v4().to_string(),
                "name": "If Has Notion Config",
                "type": "n8n-nodes-base.if",
                "typeVersion": 2,
                "position": [2180, -120],
                "parameters": {
                    "conditions": {
                        "string": [
                            {
                                "value1": "={{$json.has_notion_config || 'false'}}",
                                "operation": "equal",
                                "value2": "true"
                            }
                        ]
                    }
                }
            },
            {
                "id": uuid::Uuid::new_v4().to_string(),
                "name": "Notion Create Page",
                "type": "n8n-nodes-base.httpRequest",
                "typeVersion": 4.2,
                "position": [2430, -240],
                "continueOnFail": true,
                "parameters": {
                    "method": "POST",
                    "url": "https://api.notion.com/v1/pages",
                    "sendHeaders": true,
                    "headerParameters": {
                        "parameters": [
                            { "name": "Authorization", "value": "={{'Bearer ' + ($json.notion_token || '')}}" },
                            { "name": "Notion-Version", "value": "2022-06-28" },
                            { "name": "Content-Type", "value": "application/json" }
                        ]
                    },
                    "sendBody": true,
                    "specifyBody": "json",
                    "jsonBody": "={{ { \"parent\": { \"type\": \"page_id\", \"page_id\": $json.notion_parent_page_id }, \"properties\": { \"title\": { \"title\": [ { \"text\": { \"content\": $json.notion_title || 'AllvIa AI Trend Digest' } } ] } }, \"children\": ((Array.isArray($json.notion_children) && $json.notion_children.length > 0) ? $json.notion_children.slice(0, 95) : [ { \"object\": \"block\", \"type\": \"paragraph\", \"paragraph\": { \"rich_text\": [ { \"type\": \"text\", \"text\": { \"content\": $json.notion_excerpt || '' } } ] } } ]) } }}",
                    "options": {
                        "timeout": 15000,
                        "retry": {
                            "maxTries": 3,
                            "waitBetweenTries": 1500
                        }
                    }
                }
            },
            {
                "id": uuid::Uuid::new_v4().to_string(),
                "name": "Normalize Notion Result",
                "type": "n8n-nodes-base.code",
                "typeVersion": 2,
                "position": [2670, -240],
                "parameters": {
                    "mode": "runOnceForAllItems",
                    "jsCode": notion_result_code()
                }
            },
            {
                "id": uuid::Uuid::new_v4().to_string(),
                "name": "If Has Telegram Config",
                "type": "n8n-nodes-base.if",
                "typeVersion": 2,
                "position": [2920, -120],
                "parameters": {
                    "conditions": {
                        "string": [
                            {
                                "value1": "={{$json.has_telegram_config || 'false'}}",
                                "operation": "equal",
                                "value2": "true"
                            }
                        ]
                    }
                }
            },
            {
                "id": uuid::Uuid::new_v4().to_string(),
                "name": "Telegram Notify",
                "type": "n8n-nodes-base.httpRequest",
                "typeVersion": 4.2,
                "position": [3160, -240],
                "continueOnFail": true,
                "parameters": {
                    "method": "POST",
                    "url": "={{'https://api.telegram.org/bot' + ($json.telegram_bot_token || '') + '/sendMessage'}}",
                    "sendBody": true,
                    "specifyBody": "json",
                    "jsonBody": "={{ { \"chat_id\": $json.telegram_chat_id, \"text\": ($json.telegram_summary || 'Workflow completed'), \"disable_web_page_preview\": true } }}",
                    "options": {
                        "timeout": 12000,
                        "retry": {
                            "maxTries": 3,
                            "waitBetweenTries": 1200
                        }
                    }
                }
            },
            {
                "id": uuid::Uuid::new_v4().to_string(),
                "name": "Normalize Telegram Result",
                "type": "n8n-nodes-base.code",
                "typeVersion": 2,
                "position": [3400, -240],
                "parameters": {
                    "mode": "runOnceForAllItems",
                    "jsCode": telegram_result_code()
                }
            },
            {
                "id": uuid::Uuid::new_v4().to_string(),
                "name": "Validation Error Payload",
                "type": "n8n-nodes-base.set",
                "typeVersion": 2,
                "position": [1930, 120],
                "parameters": {
                    "keepOnlySet": false,
                    "values": {
                        "string": [
                            { "name": "status", "value": "failed_validation" },
                            { "name": "error_summary", "value": "={{Array.isArray($json.validation_errors) ? $json.validation_errors.join('; ') : 'validation failed'}}" }
                        ]
                    }
                }
            },
            {
                "id": uuid::Uuid::new_v4().to_string(),
                "name": "Observability Log",
                "type": "n8n-nodes-base.set",
                "typeVersion": 2,
                "position": [3640, -120],
                "parameters": {
                    "keepOnlySet": false,
                    "values": {
                        "string": [
                            { "name": "status", "value": "={{$json.validation_ok === 'true' ? (($json.notion_status === 'failed' || $json.telegram_status === 'failed') ? 'completed_with_warnings' : 'completed') : ($json.status || 'failed')}}" },
                            { "name": "pipeline", "value": "allvia_fallback_orchestrator_v4" },
                            { "name": "idempotency_key", "value": "={{$json.idempotency_key || ''}}" },
                            { "name": "article_count", "value": "={{String($json.article_count || 0)}}" },
                            { "name": "fetch_ok", "value": "={{String($json.fetch_ok === true)}}" },
                            { "name": "notion_status", "value": "={{$json.notion_status || ($json.has_notion_config === 'true' ? 'unknown' : 'skipped')}}" },
                            { "name": "telegram_status", "value": "={{$json.telegram_status || ($json.has_telegram_config === 'true' ? 'unknown' : 'skipped')}}" },
                            { "name": "notion_error", "value": "={{$json.notion_error || ''}}" },
                            { "name": "telegram_error", "value": "={{$json.telegram_error || ''}}" },
                            { "name": "fallback_reason", "value": "={{$json.fallback_reason || ''}}" },
                            { "name": "completed_at", "value": "={{$now}}" }
                        ]
                    }
                }
            }
        ],
        "connections": {
            "Manual Trigger": {
                "main": [[{ "node": "Input Envelope", "type": "main", "index": 0 }]]
            },
            "ProgramWebhook": {
                "main": [[{ "node": "Input Envelope", "type": "main", "index": 0 }]]
            },
            "Input Envelope": {
                "main": [[{ "node": "Idempotency Guard", "type": "main", "index": 0 }]]
            },
            "Idempotency Guard": {
                "main": [[{ "node": "Fetch Trend Feed", "type": "main", "index": 0 }]]
            },
            "Fetch Trend Feed": {
                "main": [[{ "node": "Extract Top 5 Articles", "type": "main", "index": 0 }]]
            },
            "Extract Top 5 Articles": {
                "main": [[{ "node": "Classify Trend Alignment", "type": "main", "index": 0 }]]
            },
            "Classify Trend Alignment": {
                "main": [[{ "node": "Build Comparative Summary", "type": "main", "index": 0 }]]
            },
            "Build Comparative Summary": {
                "main": [[{ "node": "Validate Output Schema", "type": "main", "index": 0 }]]
            },
            "Validate Output Schema": {
                "main": [[{ "node": "If Validation OK", "type": "main", "index": 0 }]]
            },
            "If Validation OK": {
                "main": [
                    [{ "node": "Build Markdown Report", "type": "main", "index": 0 }],
                    [{ "node": "Validation Error Payload", "type": "main", "index": 0 }]
                ]
            },
            "Build Markdown Report": {
                "main": [[{ "node": "If Has Notion Config", "type": "main", "index": 0 }]]
            },
            "If Has Notion Config": {
                "main": [
                    [{ "node": "Notion Create Page", "type": "main", "index": 0 }],
                    [{ "node": "If Has Telegram Config", "type": "main", "index": 0 }]
                ]
            },
            "Notion Create Page": {
                "main": [[{ "node": "Normalize Notion Result", "type": "main", "index": 0 }]]
            },
            "Normalize Notion Result": {
                "main": [[{ "node": "If Has Telegram Config", "type": "main", "index": 0 }]]
            },
            "If Has Telegram Config": {
                "main": [
                    [{ "node": "Telegram Notify", "type": "main", "index": 0 }],
                    [{ "node": "Observability Log", "type": "main", "index": 0 }]
                ]
            },
            "Telegram Notify": {
                "main": [[{ "node": "Normalize Telegram Result", "type": "main", "index": 0 }]]
            },
            "Normalize Telegram Result": {
                "main": [[{ "node": "Observability Log", "type": "main", "index": 0 }]]
            },
            "Validation Error Payload": {
                "main": [[{ "node": "Observability Log", "type": "main", "index": 0 }]]
            }
        },
        "settings": {
            "saveManualExecutions": true,
            "executionTimeout": 300
        },
        "active": false,
        "meta": {
            "source": "steer-fallback-orchestrator-v4"
        }
    })
}
