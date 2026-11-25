//! Trusted host credential entry points. Dispatch and project DTOs do not carry these types.
use std::fmt;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Provider {
    AssemblyAi,
    Gemini,
}

impl Provider {
    pub const fn environment(self) -> &'static str {
        match self {
            Self::AssemblyAi => "ASSEMBLYAI_API_KEY",
            Self::Gemini => "GEMINI_API_KEY",
        }
    }

    #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
    fn account(self) -> &'static str {
        match self {
            Self::AssemblyAi => "assemblyai",
            Self::Gemini => "gemini",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CredentialSource {
    #[default]
    Once,
    Vault,
    Environment,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CredentialError {
    Missing,
    Invalid,
    Locked,
    Unavailable,
    WrongProvider,
}

impl fmt::Display for CredentialError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(match self {
            Self::Missing => "No key is available from the selected source.",
            Self::Invalid => "Enter a valid printable provider key.",
            Self::Locked => "Unlock the system credential store or use a key for this job.",
            Self::Unavailable => "The system credential store is unavailable. A key can be used for this job.",
            Self::WrongProvider => "The credential belongs to a different provider.",
        })
    }
}

impl std::error::Error for CredentialError {}

/// A job-local handle. Neither this nor its contents can be serialized or formatted.
#[derive(Clone)]
pub struct CredentialRef {
    provider: Provider,
    secret: Source,
}

#[derive(Clone)]
enum Source {
    Once(Arc<str>),
    Vault,
    Environment,
}

pub fn credential_for_job(provider: Provider, source: CredentialSource, input: String) -> Result<CredentialRef, CredentialError> {
    let secret = match source {
        CredentialSource::Once => Source::Once(valid_key(&input)?.into()),
        CredentialSource::Vault => Source::Vault,
        CredentialSource::Environment => Source::Environment,
    };
    Ok(CredentialRef { provider, secret })
}

fn valid_key(key: &str) -> Result<&str, CredentialError> {
    let key = key.trim();
    if key.is_empty() {
        return Err(CredentialError::Missing);
    }
    if key.len() > 2048 || !key.bytes().all(|b| b.is_ascii_graphic()) {
        return Err(CredentialError::Invalid);
    }
    Ok(key)
}

impl CredentialRef {
    pub(crate) fn check_provider(&self, provider: Provider) -> Result<(), CredentialError> {
        if self.provider == provider { Ok(()) } else { Err(CredentialError::WrongProvider) }
    }

    pub(crate) fn resolve(&self, provider: Provider) -> Result<String, CredentialError> {
        self.check_provider(provider)?;
        match &self.secret {
            Source::Once(key) => Ok(key.to_string()),
            Source::Vault => with_entry(provider, |entry| entry.get_password()),
            Source::Environment => {
                let key = std::env::var(provider.environment()).map_err(|_| CredentialError::Missing)?;
                Ok(valid_key(&key)?.to_owned())
            }
        }
    }
}

pub fn remember_credential(provider: Provider, input: &str) -> Result<(), CredentialError> {
    let key = valid_key(input)?;
    with_entry(provider, |entry| entry.set_password(key))
}

pub fn remember_environment_credential(provider: Provider) -> Result<(), CredentialError> {
    let key = std::env::var(provider.environment()).map_err(|_| CredentialError::Missing)?;
    remember_credential(provider, &key)
}

pub fn forget_credential(provider: Provider) -> Result<(), CredentialError> {
    match with_entry(provider, |entry| entry.delete_credential()) {
        Err(CredentialError::Missing) => Ok(()),
        result => result,
    }
}

pub fn remembered_credential_status(provider: Provider) -> Result<(), CredentialError> {
    with_entry(provider, |entry| entry.get_password().map(drop))
}

#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
use keyring::Entry;

#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
fn with_entry<T>(provider: Provider, action: impl FnOnce(&Entry) -> keyring::Result<T>) -> Result<T, CredentialError> {
    with_vault_entry("com.shotclutch.provider", provider.account(), action)
}

#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
fn with_vault_entry<T>(service: &str, account: &str, action: impl FnOnce(&Entry) -> keyring::Result<T>) -> Result<T, CredentialError> {
    static ACCESS: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _guard = ACCESS.lock().map_err(|_| CredentialError::Unavailable)?;
    #[cfg(target_os = "macos")]
    let _interaction = {
        use security_framework::os::macos::keychain::SecKeychain;
        SecKeychain::user_interaction_allowed()
            .and_then(|allowed| allowed.then(SecKeychain::disable_user_interaction).transpose())
            .map_err(|_| CredentialError::Unavailable)?
    };
    Entry::new(service, account).and_then(|entry| action(&entry)).map_err(vault_error)
}

#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
fn vault_error(error: keyring::Error) -> CredentialError {
    #[cfg(target_os = "macos")]
    if let keyring::Error::PlatformFailure(cause) | keyring::Error::NoStorageAccess(cause) = &error
        && let Some(error) = cause.downcast_ref::<security_framework::base::Error>()
    {
        const INTERACTION_NOT_ALLOWED: i32 = -25308;
        return if error.code() == INTERACTION_NOT_ALLOWED { CredentialError::Locked } else { CredentialError::Unavailable };
    }
    match error {
        keyring::Error::NoEntry => CredentialError::Missing,
        keyring::Error::NoStorageAccess(_) => CredentialError::Locked,
        _ => CredentialError::Unavailable,
    }
}

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
struct Entry;
#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
impl Entry {
    fn get_password(&self) -> Result<String, CredentialError> {
        Err(CredentialError::Unavailable)
    }
    fn set_password(&self, _: &str) -> Result<(), CredentialError> {
        Err(CredentialError::Unavailable)
    }
    fn delete_credential(&self) -> Result<(), CredentialError> {
        Err(CredentialError::Unavailable)
    }
}
#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
fn with_entry<T>(_: Provider, _: impl FnOnce(&Entry) -> Result<T, CredentialError>) -> Result<T, CredentialError> {
    Err(CredentialError::Unavailable)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn once_reference_is_provider_bound_and_errors_are_safe() {
        let reference = credential_for_job(Provider::AssemblyAi, CredentialSource::Once, "synthetic-speech-key".into()).unwrap();
        assert_eq!(reference.resolve(Provider::Gemini).unwrap_err(), CredentialError::WrongProvider);
        assert_eq!(reference.resolve(Provider::AssemblyAi).unwrap(), "synthetic-speech-key");
        let mut session = crate::Session::default();
        let error = session.execute("preedit.analyze", serde_json::json!({"apiKey":"synthetic-legacy-key"})).unwrap_err();
        assert!(!error.to_string().contains("synthetic"));
        assert!(session.journal.is_empty());
        assert!(session.log.entries.is_empty());
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn secret_arguments_cannot_enter_saved_or_distributed_data() {
        use serde_json::json;
        use std::fs;

        const TYPED_SECRET: &str = "synthetic-opaque-job-credential";
        const LEGACY_SECRET: &str = "synthetic-rejected-command-credential";
        let (root, _, mut session, asset) = crate::preedit::tests::fixture();
        let project = root.path().join("credential.scproj");
        let reference = credential_for_job(Provider::Gemini, CredentialSource::Once, TYPED_SECRET.into()).unwrap();
        let _provider = crate::preedit::review_provider::GeminiConfig::new(crate::preedit::review_provider::DEFAULT_GEMINI_MODEL, reference).unwrap();
        let params = json!({"item":asset.0.to_string(),"audioStream":0,"model":"assemblyai","apiKey":LEGACY_SECRET});
        let editor = session
            .authorize_editor(
                "scoped",
                crate::service::EditorScope {
                    sequences: None,
                    allow_edits: false,
                    media_roots: vec![root.path().into()],
                    lifetime: std::time::Duration::from_secs(30),
                    max_source_bytes: 1024 * 1024,
                },
            )
            .unwrap();
        let reply = session.execute_as(&editor, "preedit.analyze", params.clone());
        assert!(reply.is_err());
        let dispatched = serde_json::to_string(&reply).unwrap();
        assert!(!dispatched.contains(LEGACY_SECRET));
        assert!(session.execute("preedit.analyze", params).is_err());
        session.execute("file.newSequence", json!({"video":1,"audio":1,"fps":24})).unwrap();
        session.execute("timeline.place", json!({"item":asset.0,"time":0,"duration":shotclutch_time::TICKS_PER_SECOND})).unwrap();
        session.execute("file.saveAs", json!({"path":project})).unwrap();
        session
            .edit("Keep authored note", |project, _| {
                project.notes = "synthetic-private-authored-note".into();
                Ok(())
            })
            .unwrap();
        session.sync_persistence();
        session.persistence.as_ref().unwrap().flush_checked().unwrap();

        fn clean(bytes: &[u8]) {
            let content = String::from_utf8_lossy(bytes);
            assert!(!content.contains(TYPED_SECRET) && !content.contains(LEGACY_SECRET));
        }

        let recovery = crate::autosave::candidate_snapshot(&session.persistence.as_ref().unwrap().session_dir);
        for path in [&project, &recovery] {
            clean(&fs::read(path).unwrap());
        }
        for (name, exclude_unused) in [("Full", false), ("Subset", true)] {
            let destination = root.path().join(name);
            let receipt = session.execute("file.projectManager", json!({"destination":destination,"excludeUnused":exclude_unused,"wait":true})).unwrap();
            clean(&serde_json::to_vec(&receipt).unwrap());
            for path in fs::read_dir(destination).unwrap().flatten().map(|entry| entry.path()).filter(|path| path.is_file()) {
                clean(&fs::read(path).unwrap());
            }
        }
        let exported = root.path().join("export.wav");
        let report = session.execute("file.exportMedia", json!({"path":exported,"format":"wav","wait":true})).unwrap();
        clean(&fs::read(exported).unwrap());
        clean(&serde_json::to_vec(&report).unwrap());
        let diagnostic = root.path().join("diagnostic.json");
        session.export_privacy_diagnostics(&diagnostic).unwrap();
        let report = fs::read(diagnostic).unwrap();
        clean(&report);
        assert!(!String::from_utf8_lossy(&report).contains("synthetic-private-authored-note"));
        assert!(session.log.entries.iter().all(|entry| !entry.message.contains(LEGACY_SECRET)));
        session.shutdown();
    }

    #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
    #[test]
    fn vault_failures_have_distinct_safe_status_and_once_remains_available() {
        let cause = || Box::new(std::io::Error::other("synthetic-private-backend-error"));
        let locked = vault_error(keyring::Error::NoStorageAccess(cause()));
        let unavailable = vault_error(keyring::Error::PlatformFailure(cause()));
        assert_eq!(locked, CredentialError::Locked);
        assert_eq!(unavailable, CredentialError::Unavailable);
        assert!(!locked.to_string().contains("synthetic") && !unavailable.to_string().contains("synthetic"));
        assert!(credential_for_job(Provider::Gemini, CredentialSource::Once, "synthetic-test-only".into()).is_ok());
    }

    #[cfg(target_os = "linux")]
    #[test]
    #[ignore = "requires an intentionally unreachable D-Bus session"]
    fn native_vault_reports_unavailable() {
        assert_eq!(remembered_credential_status(Provider::Gemini), Err(CredentialError::Unavailable));
    }

    // A different process must be able to read the entry the parent writes.
    #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
    #[test]
    #[ignore = "requires an unlocked desktop credential store"]
    fn native_vault_round_trip() {
        const SERVICE: &str = "com.shotclutch.vault-qualification";
        if let Ok(account) = std::env::var("SHOTCLUTCH_VAULT_TEST_ACCOUNT") {
            assert_eq!(with_vault_entry(SERVICE, &account, Entry::get_password).unwrap(), "synthetic-vault-replacement");
            return;
        }
        let account = uuid::Uuid::new_v4().to_string();
        struct Cleanup<'a>(&'a str);
        impl Drop for Cleanup<'_> {
            fn drop(&mut self) {
                let _ = with_vault_entry(SERVICE, self.0, Entry::delete_credential);
            }
        }
        let _cleanup = Cleanup(&account);
        with_vault_entry(SERVICE, &account, |entry| entry.set_password("synthetic-vault-first")).unwrap();
        assert_eq!(with_vault_entry(SERVICE, &account, Entry::get_password).unwrap(), "synthetic-vault-first");
        with_vault_entry(SERVICE, &account, |entry| entry.set_password("synthetic-vault-replacement")).unwrap();
        assert!(
            std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "credentials::tests::native_vault_round_trip", "--ignored"])
                .env("SHOTCLUTCH_VAULT_TEST_ACCOUNT", &account)
                .env_remove(Provider::AssemblyAi.environment())
                .env_remove(Provider::Gemini.environment())
                .status()
                .unwrap()
                .success()
        );
        #[cfg(target_os = "macos")]
        if let (Ok(path), Ok(password)) = (std::env::var("SHOTCLUTCH_TEST_KEYCHAIN"), std::env::var("SHOTCLUTCH_TEST_KEYCHAIN_PASSWORD")) {
            assert!(shotclutch_env::worker_command("security").arg("lock-keychain").arg(&path).status().unwrap().success());
            let locked = with_vault_entry(SERVICE, &account, Entry::get_password).map(drop);
            assert!(shotclutch_env::worker_command("security").args(["unlock-keychain", "-p", &password, &path]).status().unwrap().success());
            assert_eq!(locked, Err(CredentialError::Locked));
            assert_eq!(with_vault_entry(SERVICE, &account, Entry::get_password).unwrap(), "synthetic-vault-replacement");
        }
        with_vault_entry(SERVICE, &account, Entry::delete_credential).unwrap();
        assert_eq!(with_vault_entry(SERVICE, &account, Entry::get_password).unwrap_err(), CredentialError::Missing);
    }
}
