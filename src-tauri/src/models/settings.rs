use serde::{Deserialize, Serialize};

/// Allowed `theme_preference` values persisted in Settings.
pub const THEME_PREFERENCE_SYSTEM: &str = "system";
pub const THEME_PREFERENCE_LIGHT: &str = "light";
pub const THEME_PREFERENCE_DARK: &str = "dark";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Settings {
    pub hotwords: Vec<String>,
    pub context_text: String,
    /// True when a Doubao API key is present in the OS keyring.
    /// Secrets themselves are never returned over IPC.
    pub doubao_configured: bool,
    /// True when a DashScope API key is present in the OS keyring.
    /// The key itself is never returned over IPC.
    pub dashscope_configured: bool,
    /// True when TOS AK+SK (keyring) and region+bucket (SQLite) are all present.
    /// AK/SK themselves are never returned over IPC.
    pub tos_configured: bool,
    /// Non-secret TOS region (e.g. `cn-beijing`).
    pub tos_region: String,
    /// Non-secret TOS bucket name.
    pub tos_bucket: String,
    /// Optional custom TOS endpoint; empty means default `https://tos-{region}.volces.com`.
    pub tos_endpoint: String,
    /// User override for recording output directory. Empty → use default Documents/Meetphant/Recordings.
    pub recording_dir: String,
    /// Effective recording directory after resolving the empty-default rule.
    pub recording_dir_resolved: String,
    /// UI theme preference: `system` | `light` | `dark`.
    pub theme_preference: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            hotwords: Vec::new(),
            context_text: String::new(),
            doubao_configured: false,
            dashscope_configured: false,
            tos_configured: false,
            tos_region: String::new(),
            tos_bucket: String::new(),
            tos_endpoint: String::new(),
            recording_dir: String::new(),
            recording_dir_resolved: String::new(),
            theme_preference: THEME_PREFERENCE_SYSTEM.to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SettingsUpdate {
    pub hotwords: Option<Vec<String>>,
    pub context_text: Option<String>,
    /// Write-only Doubao new-console API Key (never echoed by settings_get).
    pub doubao_api_key: Option<String>,
    /// Write-only DashScope API key (never echoed by settings_get).
    pub dashscope_api_key: Option<String>,
    /// Write-only TOS Access Key Id (never echoed by settings_get).
    pub tos_access_key_id: Option<String>,
    /// Write-only TOS Secret Access Key (never echoed by settings_get).
    pub tos_secret_access_key: Option<String>,
    /// Non-secret TOS region.
    pub tos_region: Option<String>,
    /// Non-secret TOS bucket.
    pub tos_bucket: Option<String>,
    /// Optional custom TOS endpoint.
    pub tos_endpoint: Option<String>,
    /// Recording output directory override; empty string resets to default.
    pub recording_dir: Option<String>,
    /// UI theme preference: `system` | `light` | `dark`.
    pub theme_preference: Option<String>,
}
