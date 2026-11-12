//! Command implementations. `main` builds a [`Ctx`] and dispatches; the
//! tests build one over an in-memory vault and a scratch config directory.

pub mod auth;
pub mod config;
pub mod orgs;

use std::path::PathBuf;

use pk_cli_config::ConfigStore;
use pk_cli_core::{CliError, CommonArgs};
use pk_cli_secrets::CredentialStore;

use crate::config::Config;
use crate::credentials::Vault;
use crate::state::StateFile;

pub const BIN: &str = "kp";
pub const REPO: &str = "piekstra/kaiser-cli";

/// Names the Keychain service a run uses instead of `piekstra.kp`. For the
/// test suite and for dogfooding error paths: a service that holds nothing
/// reads as signed out without a Keychain prompt. It moves only the
/// Keychain; the config and state stay where they are.
pub const KEYCHAIN_SERVICE_ENV: &str = "KP_KEYCHAIN_SERVICE";

pub struct Ctx {
    pub common: CommonArgs,
    pub store: ConfigStore,
    pub vault: Box<dyn Vault>,
}

impl Ctx {
    /// The binary's context: the Keychain (or `KP_KEYCHAIN_SERVICE`) and
    /// `~/.config/kp`.
    pub fn for_binary(common: CommonArgs, config: Option<PathBuf>) -> Self {
        Ctx {
            common,
            store: ConfigStore::new(BIN).with_override(config),
            vault: Box::new(credential_store()),
        }
    }

    pub fn json(&self) -> bool {
        self.common.json
    }

    pub fn config(&self) -> Result<Config, CliError> {
        self.store.load()
    }

    pub fn state_file(&self) -> Result<StateFile, CliError> {
        Ok(StateFile::beside(&self.store.path()?))
    }

    pub fn note(&self, msg: impl AsRef<str>) {
        if !self.common.quiet {
            eprintln!("{}", msg.as_ref());
        }
    }
}

pub fn credential_store() -> CredentialStore {
    match std::env::var(KEYCHAIN_SERVICE_ENV) {
        Ok(service) if !service.trim().is_empty() => CredentialStore::new(service),
        _ => CredentialStore::for_binary(BIN),
    }
}
