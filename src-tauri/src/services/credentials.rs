//! Credential storage via OS keyring (never SQLite).
//!
//! Doubao (ASR) and DashScope (summary) keys live in separate keyring accounts.
//! In unit tests, in-memory stores are used so CI never touches the real keyring.

use crate::error::{AppErrorDto, CmdResult};

/// Doubao speech new-console API Key (sent as `X-Api-Key`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoubaoCredentials {
    pub api_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DashScopeCredentials {
    pub api_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TosCredentials {
    pub access_key_id: String,
    pub secret_access_key: String,
}

#[cfg(test)]
mod doubao_store {
    use super::*;
    use std::cell::RefCell;

    thread_local! {
        static MEMORY: RefCell<Option<String>> = const { RefCell::new(None) };
    }

    pub fn get() -> CmdResult<Option<DoubaoCredentials>> {
        Ok(MEMORY.with(|cell| {
            cell.borrow().as_ref().map(|api_key| DoubaoCredentials {
                api_key: api_key.clone(),
            })
        }))
    }

    pub fn set(api_key: &str) -> CmdResult<()> {
        MEMORY.with(|cell| {
            *cell.borrow_mut() = Some(api_key.to_string());
        });
        Ok(())
    }

    pub fn clear() -> CmdResult<()> {
        MEMORY.with(|cell| {
            *cell.borrow_mut() = None;
        });
        Ok(())
    }

    pub fn reset_for_test() {
        let _ = clear();
    }
}

#[cfg(not(test))]
mod doubao_store {
    use super::*;
    use keyring::Entry;

    const SERVICE: &str = "meetphant";
    const LEGACY_SERVICE: &str = "meetly";
    const ACCOUNT_API_KEY: &str = "doubao_api_key";
    /// Old-console App Id / Access Token accounts; no longer supported.
    const LEGACY_ACCOUNTS: [&str; 2] = ["doubao_app_id", "doubao_access_token"];

    fn entry() -> CmdResult<Entry> {
        Entry::new(SERVICE, ACCOUNT_API_KEY)
            .map_err(|_| AppErrorDto::internal("Failed to open credential store"))
    }

    pub fn get() -> CmdResult<Option<DoubaoCredentials>> {
        match entry()?.get_password() {
            Ok(value) if !value.is_empty() => Ok(Some(DoubaoCredentials { api_key: value })),
            Ok(_) => Ok(None),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(_) => Err(AppErrorDto::internal("Failed to read credentials")),
        }
    }

    pub fn set(api_key: &str) -> CmdResult<()> {
        // Do not forward keyring Display into IPC (may include paths).
        entry()?
            .set_password(api_key)
            .map_err(|_| AppErrorDto::internal("Failed to store Doubao API key"))?;
        match get()? {
            Some(stored) if stored.api_key == api_key => Ok(()),
            Some(_) | None => Err(AppErrorDto::internal(
                "Credential store write did not persist; check OS keyring access",
            )),
        }
    }

    pub fn clear() -> CmdResult<()> {
        match entry()?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(_) => Err(AppErrorDto::internal("Failed to clear credentials")),
        }
    }

    /// Old-console App Id + Access Token cannot be converted into an API Key,
    /// so best-effort delete them from both the current and pre-rename services.
    pub fn migrate_legacy() {
        for service in [SERVICE, LEGACY_SERVICE] {
            for account in LEGACY_ACCOUNTS {
                if let Ok(legacy) = Entry::new(service, account) {
                    let _ = legacy.delete_credential();
                }
            }
        }
    }
}

#[cfg(test)]
mod dashscope_store {
    use super::*;
    use std::cell::RefCell;

    thread_local! {
        static MEMORY: RefCell<Option<String>> = const { RefCell::new(None) };
    }

    pub fn get() -> CmdResult<Option<DashScopeCredentials>> {
        Ok(MEMORY.with(|cell| {
            cell.borrow().as_ref().map(|api_key| DashScopeCredentials {
                api_key: api_key.clone(),
            })
        }))
    }

    pub fn set(api_key: &str) -> CmdResult<()> {
        MEMORY.with(|cell| {
            *cell.borrow_mut() = Some(api_key.to_string());
        });
        Ok(())
    }

    pub fn clear() -> CmdResult<()> {
        MEMORY.with(|cell| {
            *cell.borrow_mut() = None;
        });
        Ok(())
    }

    pub fn reset_for_test() {
        let _ = clear();
    }
}

#[cfg(not(test))]
mod dashscope_store {
    use super::*;
    use keyring::Entry;

    const SERVICE: &str = "meetphant";
    const LEGACY_SERVICE: &str = "meetly";
    const ACCOUNT_API_KEY: &str = "dashscope_api_key";

    fn entry() -> CmdResult<Entry> {
        Entry::new(SERVICE, ACCOUNT_API_KEY)
            .map_err(|_| AppErrorDto::internal("Failed to open credential store"))
    }

    pub fn get() -> CmdResult<Option<DashScopeCredentials>> {
        match entry()?.get_password() {
            Ok(value) if !value.is_empty() => Ok(Some(DashScopeCredentials { api_key: value })),
            Ok(_) => Ok(None),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(_) => Err(AppErrorDto::internal("Failed to read credentials")),
        }
    }

    pub fn set(api_key: &str) -> CmdResult<()> {
        // Do not forward keyring Display into IPC (may include paths).
        entry()?
            .set_password(api_key)
            .map_err(|_| AppErrorDto::internal("Failed to store DashScope API key"))?;
        match get()? {
            Some(stored) if stored.api_key == api_key => Ok(()),
            Some(_) | None => Err(AppErrorDto::internal(
                "Credential store write did not persist; check OS keyring access",
            )),
        }
    }

    pub fn clear() -> CmdResult<()> {
        match entry()?.delete_credential() {
            Ok(()) => Ok(()),
            Err(keyring::Error::NoEntry) => Ok(()),
            Err(_) => Err(AppErrorDto::internal("Failed to clear credentials")),
        }
    }

    /// One-time migration from the pre-rename `meetly` keyring service.
    pub fn migrate_legacy() {
        if matches!(get(), Ok(Some(_))) {
            return;
        }
        let Ok(legacy_entry) = Entry::new(LEGACY_SERVICE, ACCOUNT_API_KEY) else {
            return;
        };
        let Ok(api_key) = legacy_entry.get_password() else {
            return;
        };
        if api_key.is_empty() {
            return;
        }
        if set(&api_key).is_ok() {
            let _ = legacy_entry.delete_credential();
        }
    }
}

pub fn is_configured() -> bool {
    matches!(doubao_store::get(), Ok(Some(_)))
}

pub fn is_dashscope_configured() -> bool {
    matches!(dashscope_store::get(), Ok(Some(_)))
}

pub fn get_credentials() -> CmdResult<Option<DoubaoCredentials>> {
    doubao_store::get()
}

pub fn get_dashscope_credentials() -> CmdResult<Option<DashScopeCredentials>> {
    dashscope_store::get()
}

pub fn require_credentials() -> CmdResult<DoubaoCredentials> {
    doubao_store::get()?.ok_or_else(AppErrorDto::asr_not_configured)
}

pub fn require_dashscope_credentials() -> CmdResult<DashScopeCredentials> {
    dashscope_store::get()?.ok_or_else(AppErrorDto::summary_not_configured)
}

/// Persist Doubao API key. Empty strings are rejected.
pub fn set_credentials(api_key: &str) -> CmdResult<()> {
    let api_key = api_key.trim();
    if api_key.is_empty() {
        return Err(AppErrorDto::settings_invalid(
            "Doubao API key cannot be empty",
        ));
    }
    doubao_store::set(api_key)
}

/// Persist DashScope API key. Empty strings are rejected.
pub fn set_dashscope_credentials(api_key: &str) -> CmdResult<()> {
    let api_key = api_key.trim();
    if api_key.is_empty() {
        return Err(AppErrorDto::settings_invalid(
            "DashScope API key cannot be empty",
        ));
    }
    dashscope_store::set(api_key)
}

pub fn clear_credentials() -> CmdResult<()> {
    doubao_store::clear()
}

pub fn clear_dashscope_credentials() -> CmdResult<()> {
    dashscope_store::clear()
}

#[cfg(test)]
mod tos_store {
    use super::*;
    use std::cell::RefCell;

    thread_local! {
        static MEMORY: RefCell<Option<(String, String)>> = const { RefCell::new(None) };
    }

    pub fn get() -> CmdResult<Option<TosCredentials>> {
        Ok(MEMORY.with(|cell| {
            cell.borrow().as_ref().map(|(ak, sk)| TosCredentials {
                access_key_id: ak.clone(),
                secret_access_key: sk.clone(),
            })
        }))
    }

    pub fn set(access_key_id: &str, secret_access_key: &str) -> CmdResult<()> {
        MEMORY.with(|cell| {
            *cell.borrow_mut() = Some((access_key_id.to_string(), secret_access_key.to_string()));
        });
        Ok(())
    }

    pub fn clear() -> CmdResult<()> {
        MEMORY.with(|cell| {
            *cell.borrow_mut() = None;
        });
        Ok(())
    }

    pub fn reset_for_test() {
        let _ = clear();
    }
}

#[cfg(not(test))]
mod tos_store {
    use super::*;
    use keyring::Entry;

    const SERVICE: &str = "meetphant";
    const LEGACY_SERVICE: &str = "meetly";
    const ACCOUNT_AK: &str = "tos_access_key_id";
    const ACCOUNT_SK: &str = "tos_secret_access_key";

    fn entry(account: &str) -> CmdResult<Entry> {
        Entry::new(SERVICE, account)
            .map_err(|_| AppErrorDto::internal("Failed to open credential store"))
    }

    fn read_secret(account: &str) -> CmdResult<Option<String>> {
        match entry(account)?.get_password() {
            Ok(value) if !value.is_empty() => Ok(Some(value)),
            Ok(_) => Ok(None),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(_) => Err(AppErrorDto::internal("Failed to read credentials")),
        }
    }

    pub fn get() -> CmdResult<Option<TosCredentials>> {
        let access_key_id = read_secret(ACCOUNT_AK)?;
        let secret_access_key = read_secret(ACCOUNT_SK)?;
        match (access_key_id, secret_access_key) {
            (Some(access_key_id), Some(secret_access_key)) => Ok(Some(TosCredentials {
                access_key_id,
                secret_access_key,
            })),
            _ => Ok(None),
        }
    }

    pub fn set(access_key_id: &str, secret_access_key: &str) -> CmdResult<()> {
        entry(ACCOUNT_AK)?
            .set_password(access_key_id)
            .map_err(|_| AppErrorDto::internal("Failed to store TOS access key id"))?;
        entry(ACCOUNT_SK)?
            .set_password(secret_access_key)
            .map_err(|_| AppErrorDto::internal("Failed to store TOS secret access key"))?;
        match get()? {
            Some(stored)
                if stored.access_key_id == access_key_id
                    && stored.secret_access_key == secret_access_key =>
            {
                Ok(())
            }
            Some(_) | None => Err(AppErrorDto::internal(
                "Credential store write did not persist; check OS keyring access",
            )),
        }
    }

    pub fn clear() -> CmdResult<()> {
        for account in [ACCOUNT_AK, ACCOUNT_SK] {
            match entry(account)?.delete_credential() {
                Ok(()) => {}
                Err(keyring::Error::NoEntry) => {}
                Err(_) => {
                    return Err(AppErrorDto::internal("Failed to clear credentials"));
                }
            }
        }
        Ok(())
    }

    /// One-time migration from the pre-rename `meetly` keyring service.
    pub fn migrate_legacy() {
        if matches!(get(), Ok(Some(_))) {
            return;
        }
        let Ok(ak_entry) = Entry::new(LEGACY_SERVICE, ACCOUNT_AK) else {
            return;
        };
        let Ok(sk_entry) = Entry::new(LEGACY_SERVICE, ACCOUNT_SK) else {
            return;
        };
        let (Ok(access_key_id), Ok(secret_access_key)) =
            (ak_entry.get_password(), sk_entry.get_password())
        else {
            return;
        };
        if access_key_id.is_empty() || secret_access_key.is_empty() {
            return;
        }
        if set(&access_key_id, &secret_access_key).is_ok() {
            let _ = ak_entry.delete_credential();
            let _ = sk_entry.delete_credential();
        }
    }
}

pub fn is_tos_secrets_configured() -> bool {
    matches!(tos_store::get(), Ok(Some(_)))
}

pub fn get_tos_credentials() -> CmdResult<Option<TosCredentials>> {
    tos_store::get()
}

pub fn require_tos_credentials() -> CmdResult<TosCredentials> {
    tos_store::get()?.ok_or_else(AppErrorDto::tos_not_configured)
}

/// Persist TOS credentials. Empty strings are rejected.
pub fn set_tos_credentials(access_key_id: &str, secret_access_key: &str) -> CmdResult<()> {
    let access_key_id = access_key_id.trim();
    let secret_access_key = secret_access_key.trim();
    if access_key_id.is_empty() || secret_access_key.is_empty() {
        return Err(AppErrorDto::settings_invalid(
            "TOS access key id and secret access key cannot be empty",
        ));
    }
    tos_store::set(access_key_id, secret_access_key)
}

pub fn clear_tos_credentials() -> CmdResult<()> {
    tos_store::clear()
}

#[cfg(test)]
pub fn reset_for_test() {
    doubao_store::reset_for_test();
    dashscope_store::reset_for_test();
    tos_store::reset_for_test();
}

/// Move any credentials saved under the pre-rename `meetly` keyring service
/// over to the current `meetphant` service, and drop unsupported old-console
/// Doubao App Id / Access Token entries. Safe to call on every startup.
#[cfg(not(test))]
pub fn migrate_legacy_credentials() {
    doubao_store::migrate_legacy();
    dashscope_store::migrate_legacy();
    tos_store::migrate_legacy();
}

#[cfg(test)]
pub fn migrate_legacy_credentials() {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configured_flag_false_until_api_key_set() {
        reset_for_test();
        assert!(!is_configured());
        set_credentials("  doubao-api-key  ").expect("set");
        assert!(is_configured());
        let creds = get_credentials().expect("get").expect("some");
        assert_eq!(creds.api_key, "doubao-api-key");
        clear_credentials().expect("clear");
        assert!(!is_configured());
    }

    #[test]
    fn empty_credentials_rejected() {
        reset_for_test();
        let err = set_credentials("   ").expect_err("empty key");
        assert_eq!(err.code, "SETTINGS_INVALID");
        assert!(!is_configured());
    }

    #[test]
    fn dashscope_configured_flag_without_leaking_key() {
        reset_for_test();
        assert!(!is_dashscope_configured());
        set_dashscope_credentials("sk-secret-key").expect("set");
        assert!(is_dashscope_configured());
        let creds = get_dashscope_credentials().expect("get").expect("some");
        assert_eq!(creds.api_key, "sk-secret-key");
        clear_dashscope_credentials().expect("clear");
        assert!(!is_dashscope_configured());
    }

    #[test]
    fn empty_dashscope_key_rejected() {
        reset_for_test();
        let err = set_dashscope_credentials("   ").expect_err("empty");
        assert_eq!(err.code, "SETTINGS_INVALID");
    }

    #[test]
    fn tos_secrets_configured_flag() {
        reset_for_test();
        assert!(!is_tos_secrets_configured());
        set_tos_credentials("ak", "sk").expect("set");
        assert!(is_tos_secrets_configured());
        let creds = get_tos_credentials().expect("get").expect("some");
        assert_eq!(creds.access_key_id, "ak");
        assert_eq!(creds.secret_access_key, "sk");
        clear_tos_credentials().expect("clear");
        assert!(!is_tos_secrets_configured());
    }
}
