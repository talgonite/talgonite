use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameServerMirror {
    pub id: String,
    pub name: String,
    pub address: String,
    pub port: u16,
    pub is_default: bool,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedLoginMirror {
    pub id: String,
    pub username: String,
    pub last_used: u64,
    pub character_preview: Option<LoginCharacterPreviewMirror>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginCharacterPreviewMirror {
    pub name: String,
    pub class: String,
    pub level: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "screen", rename_all = "snake_case")]
pub enum UiScreenState {
    MainMenu,
    ServerSelector,
    AddServer,
    EditServer,
    LoginSelector,
    Login,
    CharacterCreator,
    LoggingIn { username: String },
    LoginFailure { message: String },
}

impl Default for UiScreenState {
    fn default() -> Self {
        UiScreenState::MainMenu
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiSnapshot {
    pub screen: UiScreenState,
    pub servers: Vec<ServerSummary>,
    pub selected_server_id: Option<String>,
    pub selected_login_id: Option<String>,
    pub logins: Vec<LoginSummary>,
    pub add_server: Option<AddEditServerData>,
    pub edit_server: Option<AddEditServerData>,
    pub login_form: Option<LoginFormData>,
    pub character_creator: Option<CharacterCreatorData>,
}

impl Default for UiSnapshot {
    fn default() -> Self {
        Self {
            screen: UiScreenState::default(),
            servers: vec![],
            selected_server_id: None,
            selected_login_id: None,
            logins: vec![],
            add_server: None,
            edit_server: None,
            login_form: None,
            character_creator: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerSummary {
    pub id: String,
    pub name: String,
    pub address: String,
    pub port: u16,
    pub is_default: bool,
    pub description: Option<String>,
    pub selected: bool,
}

impl From<(&GameServerMirror, bool)> for ServerSummary {
    fn from((g, sel): (&GameServerMirror, bool)) -> Self {
        Self {
            id: g.id.clone(),
            name: g.name.clone(),
            address: g.address.clone(),
            port: g.port,
            is_default: g.is_default,
            description: g.description.clone(),
            selected: sel,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginSummary {
    pub id: String,
    pub username: String,
    pub last_used: u64,
    pub preview: Option<CharacterPreviewSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CharacterPreviewSummary {
    pub name: String,
    pub class: String,
    pub level: u32,
}

impl From<&SavedLoginMirror> for LoginSummary {
    fn from(l: &SavedLoginMirror) -> Self {
        Self {
            id: l.id.clone(),
            username: l.username.clone(),
            last_used: l.last_used,
            preview: l
                .character_preview
                .as_ref()
                .map(|p| CharacterPreviewSummary {
                    name: p.name.clone(),
                    class: p.class.clone(),
                    level: p.level,
                }),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddEditServerData {
    pub name: String,
    pub address: String,
    pub port: String,
    pub description: String,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CharacterCreatorData {
    pub name: String,
    pub password: String,
    pub gender: String,
    pub hair_style: u8,
    pub hair_color: u8,
    pub name_valid: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginFormData {
    pub username: String,
    pub password: String,
    pub error_message: Option<String>,
}
