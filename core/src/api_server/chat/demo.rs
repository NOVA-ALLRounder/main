use axum::Json;

use crate::api_server::chat_support::{respond_chat, ChatOpsFlags};
use crate::api_server::AppState;

use super::{ChatRequest, ChatResponse};

pub(crate) async fn handle_demo_vision_command(
    state: &AppState,
    req: &ChatRequest,
    memory_scope_ref: Option<&str>,
    message: &str,
) -> Option<Json<ChatResponse>> {
    if message != "demo_vision" {
        return None;
    }

    let llm = state.llm_client.as_ref()?;
    let llm_clone = llm.clone();

    tokio::spawn(async move {
        let mut driver = crate::visual_driver::VisualDriver::new();
        use crate::visual_driver::{SmartStep, UiAction};

        driver.add_step(
            SmartStep::new(
                UiAction::OpenUrl("https://www.google.com".to_string()),
                "Open Google",
            )
            .with_post_check("Is the Google search homepage visible?"),
        );
        driver.add_step(SmartStep::new(UiAction::Wait(3), "Wait for Load"));
        driver.add_step(
            SmartStep::new(
                UiAction::Type("Hello World".to_string()),
                "Type Search Query",
            )
            .with_pre_check("Is there a search input field visible?")
            .with_post_check("Is the text 'Hello World' visible in the search bar?"),
        );

        if let Err(e) = driver.execute(Some(llm_clone.as_ref())).await {
            eprintln!("❌ Vision Demo Failed: {}", e);
        } else {
            println!("✅ Vision Demo Completed Successfully.");
        }
    });

    Some(respond_chat(
        req,
        memory_scope_ref,
        message,
        ChatResponse {
            response: "🚀 Vision 검증 데모(Smart Mode)를 시작합니다.\n(Google 접속 -> [검증] -> 키 입력 -> [검증])".to_string(),
            command: None,
            route_meta: None,
        },
        "vision_demo",
        "success",
        Some(1.0),
        ChatOpsFlags::default(),
        None,
    ))
}
