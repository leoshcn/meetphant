//! Credential storage via OS keyring (never SQLite).
//!
//! Doubao (ASR) and summary LLM keys live in separate keyring accounts.
//! In unit tests, in-memory stores are used so CI never touches the real keyring.

use std::fmt;

use crate::error::{AppErrorDto, CmdResult};

/// Doubao speech new-console API Key (sent as `X-Api-Key`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoubaoCredentials {
    pub api_key: String,
}

/// API key for the configured OpenAI-compatible summary provider.
#[derive(Clone, PartialEq, Eq)]
pub struct SummaryLlmCredentials {
    pub api_key: String,
}

impl fmt::Debug for SummaryLlmCredentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SummaryLlmCredentials")
            .field("api_key", &"<redacted>")
            .finish()
    }
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

/// Decide whether a legacy DashScope key should be copied into the summary LLM
/// account: only when the new account is empty, taking the first non-empty source.
fn pick_migration_source(current: Option<&str>, sources: &[Option<String>]) -> Option<String> {
    if current.map(|c| !c.trim().is_empty()).unwrap_or(false) {
        return None;
    }
    sources
        .iter()
        .flatten()
        .map(|s| s.trim())
        .find(|s| !s.is_empty())
        .map(str::to_string)
}

#[cfg(test)]
mod summary_llm_store {
    use super::*;
    use std::cell::RefCell;

    thread_local! {
        static MEMORY: RefCell<Option<String>> = const { RefCell::new(None) };
        /// Simulates the pre-upgrade `dashscope_api_key` keyring entry.
        static LEGACY_DASHSCOPE: RefCell<Option<String>> = const { RefCell::new(None) };
    }

    pub fn get() -> CmdResult<Option<SummaryLlmCredentials>> {
        Ok(MEMORY.with(|cell| {
            cell.borrow().as_ref().map(|api_key| SummaryLlmCredentials {
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
        LEGACY_DASHSCOPE.with(|cell| {
            *cell.borrow_mut() = None;
        });
        Ok(())
    }

    pub fn reset_for_test() {
        let _ = clear();
    }

    pub fn seed_legacy(api_key: &str) {
        LEGACY_DASHSCOPE.with(|cell| {
            *cell.borrow_mut() = Some(api_key.to_string());
        });
    }

    pub fn legacy() -> Option<String> {
        LEGACY_DASHSCOPE.with(|cell| cell.borrow().clone())
    }

    pub fn migrate_legacy() {
        let current = get().ok().flatten().map(|c| c.api_key);
        if let Some(key) = pick_migration_source(current.as_deref(), &[legacy()]) {
            let _ = set(&key);
        }
    }
}

#[cfg(not(test))]
mod summary_llm_store {
    use super::*;
    use keyring::Entry;

    const SERVICE: &str = "meetphant";
    const LEGACY_SERVICE: &str = "meetly";
    const ACCOUNT_API_KEY: &str = "summary_llm_api_key";
    /// Pre-configurable-provider account; copied (not moved) on upgrade so an
    /// older build can still read it after a rollback.
    const LEGACY_ACCOUNT: &str = "dashscope_api_key";

    fn entry() -> CmdResult<Entry> {
        Entry::new(SERVICE, ACCOUNT_API_KEY)
            .map_err(|_| AppErrorDto::internal("Failed to open credential store"))
    }

    pub fn get() -> CmdResult<Option<SummaryLlmCredentials>> {
        match entry()?.get_password() {
            Ok(value) if !value.is_empty() => Ok(Some(SummaryLlmCredentials { api_key: value })),
            Ok(_) => Ok(None),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(_) => Err(AppErrorDto::internal("Failed to read credentials")),
        }
    }

    pub fn set(api_key: &str) -> CmdResult<()> {
        // Do not forward keyring Display into IPC (may include paths).
        entry()?
            .set_password(api_key)
            .map_err(|_| AppErrorDto::internal("Failed to store summary model API key"))?;
        match get()? {
            Some(stored) if stored.api_key == api_key => Ok(()),
            Some(_) | None => Err(AppErrorDto::internal(
                "Credential store write did not persist; check OS keyring access",
            )),
        }
    }

    fn delete(service: &str, account: &str) -> CmdResult<()> {
        let entry = Entry::new(service, account)
            .map_err(|_| AppErrorDto::internal("Failed to open credential store"))?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(_) => Err(AppErrorDto::internal("Failed to clear credentials")),
        }
    }

    /// Clears the key and the legacy DashScope copies, so the startup
    /// migration cannot resurrect a key the user explicitly cleared.
    pub fn clear() -> CmdResult<()> {
        delete(SERVICE, ACCOUNT_API_KEY)?;
        delete(SERVICE, LEGACY_ACCOUNT)?;
        delete(LEGACY_SERVICE, LEGACY_ACCOUNT)
    }

    fn read_legacy(service: &str) -> Option<String> {
        Entry::new(service, LEGACY_ACCOUNT)
            .ok()?
            .get_password()
            .ok()
            .filter(|v| !v.is_empty())
    }

    /// Copy `dashscope_api_key` (current `meetphant` service first, then the
    /// pre-rename `meetly` service) into `summary_llm_api_key` when the latter
    /// is empty. Old entries are kept. Safe to call on every startup.
    pub fn migrate_legacy() {
        // Only migrate when the new account is definitely empty (not on read errors).
        let current = match get() {
            Ok(current) => current.map(|c| c.api_key),
            Err(_) => return,
        };
        let sources = [read_legacy(SERVICE), read_legacy(LEGACY_SERVICE)];
        if let Some(key) = pick_migration_source(current.as_deref(), &sources) {
            if let Err(err) = set(&key) {
                tracing::warn!(code = %err.code, "summary llm key migration failed");
            }
        }
    }
}

pub fn is_configured() -> bool {
    matches!(doubao_store::get(), Ok(Some(_)))
}

/// True when a summary LLM API key is present in the keyring.
pub fn is_summary_llm_configured() -> bool {
    matches!(summary_llm_store::get(), Ok(Some(_)))
}

pub fn get_credentials() -> CmdResult<Option<DoubaoCredentials>> {
    doubao_store::get()
}

pub fn get_summary_llm_credentials() -> CmdResult<Option<SummaryLlmCredentials>> {
    summary_llm_store::get()
}

pub fn require_credentials() -> CmdResult<DoubaoCredentials> {
    doubao_store::get()?.ok_or_else(AppErrorDto::asr_not_configured)
}

pub fn require_summary_llm_credentials() -> CmdResult<SummaryLlmCredentials> {
    summary_llm_store::get()?.ok_or_else(AppErrorDto::summary_not_configured)
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

/// Persist the summary LLM API key. Empty strings are rejected.
pub fn set_summary_llm_credentials(api_key: &str) -> CmdResult<()> {
    let api_key = api_key.trim();
    if api_key.is_empty() {
        return Err(AppErrorDto::settings_invalid(
            "Summary model API key cannot be empty",
        ));
    }
    summary_llm_store::set(api_key)
}

pub fn clear_credentials() -> CmdResult<()> {
    doubao_store::clear()
}

pub fn clear_summary_llm_credentials() -> CmdResult<()> {
    summary_llm_store::clear()
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
    summary_llm_store::reset_for_test();
    tos_store::reset_for_test();
}

/// Simulate a pre-upgrade `dashscope_api_key` keyring entry (tests only).
#[cfg(test)]
pub fn seed_legacy_dashscope_key_for_test(api_key: &str) {
    summary_llm_store::seed_legacy(api_key);
}

/// Whether the simulated legacy `dashscope_api_key` entry still exists (tests only).
#[cfg(test)]
pub fn legacy_dashscope_key_for_test() -> Option<String> {
    summary_llm_store::legacy()
}

/// Move any credentials saved under the pre-rename `meetly` keyring service
/// over to the current `meetphant` service, copy the legacy DashScope key into
/// the summary LLM account (keeping the old entry), and drop unsupported
/// old-console Doubao App Id / Access Token entries. Safe to call on every startup.
#[cfg(not(test))]
pub fn migrate_legacy_credentials() {
    doubao_store::migrate_legacy();
    summary_llm_store::migrate_legacy();
    tos_store::migrate_legacy();
}

/// Test build: only the summary LLM copy-migration has an in-memory equivalent.
#[cfg(test)]
pub fn migrate_legacy_credentials() {
    summary_llm_store::migrate_legacy();
}

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
    fn summary_llm_configured_flag_without_leaking_key() {
        reset_for_test();
        assert!(!is_summary_llm_configured());
        set_summary_llm_credentials("sk-secret-key").expect("set");
        assert!(is_summary_llm_configured());
        let creds = get_summary_llm_credentials().expect("get").expect("some");
        assert_eq!(creds.api_key, "sk-secret-key");
        assert!(!format!("{creds:?}").contains("sk-secret-key"));
        clear_summary_llm_credentials().expect("clear");
        assert!(!is_summary_llm_configured());
    }

    #[test]
    fn empty_summary_llm_key_rejected() {
        reset_for_test();
        let err = set_summary_llm_credentials("   ").expect_err("empty");
        assert_eq!(err.code, "SETTINGS_INVALID");
    }

    #[test]
    fn migration_source_only_when_new_account_empty() {
        let legacy = [None, Some("  sk-meetly  ".to_string())];
        assert_eq!(
            pick_migration_source(None, &legacy).as_deref(),
            Some("sk-meetly")
        );
        assert_eq!(
            pick_migration_source(Some(""), &legacy).as_deref(),
            Some("sk-meetly")
        );
        assert_eq!(pick_migration_source(Some("sk-new"), &legacy), None);
        assert_eq!(pick_migration_source(None, &[None, Some(" ".into())]), None);
        // Current `meetphant` service wins over the pre-rename `meetly` service.
        assert_eq!(
            pick_migration_source(None, &[Some("sk-a".into()), Some("sk-b".into())]).as_deref(),
            Some("sk-a")
        );
    }

    #[test]
    fn legacy_dashscope_key_is_copied_and_kept() {
        reset_for_test();
        seed_legacy_dashscope_key_for_test("sk-legacy");
        migrate_legacy_credentials();
        let creds = get_summary_llm_credentials().expect("get").expect("some");
        assert_eq!(creds.api_key, "sk-legacy");
        assert_eq!(
            legacy_dashscope_key_for_test().as_deref(),
            Some("sk-legacy")
        );

        // Re-running does not overwrite a newer key.
        set_summary_llm_credentials("sk-deepseek").unwrap();
        migrate_legacy_credentials();
        let creds = get_summary_llm_credentials().expect("get").expect("some");
        assert_eq!(creds.api_key, "sk-deepseek");
    }

    #[test]
    fn clear_prevents_legacy_key_resurrection() {
        reset_for_test();
        seed_legacy_dashscope_key_for_test("sk-legacy");
        migrate_legacy_credentials();
        assert!(is_summary_llm_configured());
        clear_summary_llm_credentials().expect("clear");
        migrate_legacy_credentials();
        assert!(!is_summary_llm_configured());
        assert_eq!(legacy_dashscope_key_for_test(), None);
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
