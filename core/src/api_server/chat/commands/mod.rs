mod local;
mod system;
mod vision;

use crate::api_server::chat_support::ChatOpsFlags;

pub(crate) use self::local::handle_local_chat_command;
pub(crate) use self::system::handle_system_chat_command;
pub(crate) use self::vision::handle_vision_chat_command;

fn local_only_flags() -> ChatOpsFlags {
    ChatOpsFlags {
        local_only: true,
        ..Default::default()
    }
}
