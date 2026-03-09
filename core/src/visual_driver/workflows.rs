use crate::visual_driver::{SmartStep, UiAction, VisualDriver};

pub fn n8n_fallback_create_workflow() -> VisualDriver {
    let mut driver = VisualDriver::new();
    driver.steps.extend([
        SmartStep::new(
            UiAction::OpenUrl("https://app.n8n.cloud".to_string()),
            "Open n8n cloud",
        ),
        SmartStep::new(UiAction::Wait(5), "Wait for n8n to load"),
        SmartStep::new(
            UiAction::Click("Create Workflow".to_string()),
            "Click Create Workflow",
        ),
    ]);
    driver
}
