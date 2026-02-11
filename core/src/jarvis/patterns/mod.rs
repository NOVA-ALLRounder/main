// Pattern detection implementations

pub mod app_launch;
pub mod click_sequence;
pub mod file_open;
pub mod website_visit;

pub use app_launch::AppLaunchPattern;
pub use click_sequence::ClickSequencePattern;
pub use file_open::FileOpenPattern;
pub use website_visit::WebsiteVisitPattern;
