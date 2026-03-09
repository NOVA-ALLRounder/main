use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct Update {
    pub(crate) update_id: u64,
    pub(crate) message: Option<Message>,
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct Message {
    pub(crate) message_id: u64,
    pub(crate) from: Option<User>,
    pub(crate) chat: Chat,
    pub(crate) text: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct User {
    pub(crate) id: u64,
    pub(crate) username: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct Chat {
    pub(crate) id: i64,
    #[serde(rename = "type")]
    pub(crate) kind: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct GetUpdatesResponse {
    pub(crate) ok: bool,
    pub(crate) result: Option<Vec<Update>>,
}

#[derive(Deserialize, Debug)]
pub(crate) struct TelegramApiStatusResponse {
    pub(crate) ok: bool,
    pub(crate) description: Option<String>,
}
