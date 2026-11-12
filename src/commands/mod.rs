//! Command implementations. `main` builds a [`Ctx`] and dispatches; the
//! tests build one over an in-memory vault, a scratch config directory and
//! lock, and a stand-in browser.

pub mod api;
pub mod auth;
pub mod config;
pub mod orgs;
pub mod patients;
pub mod records;

use std::path::PathBuf;
use std::sync::Arc;

use pk_cli_auth::refresh_lock::RefreshLock;
use pk_cli_config::ConfigStore;
use pk_cli_core::{CliError, CommonArgs};
use pk_cli_http::BlockingTransport;
use pk_cli_secrets::CredentialStore;

use crate::cli::PatientArgs;
use crate::config::Config;
use crate::credentials::{self, ClientCredentials, Vault};
use crate::kaiser::{self, Environment};
use crate::session::{Context, Live};
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
    pub vault: Arc<dyn Vault>,
    pub lock: RefreshLock,
    /// Opens the sign-in URL; returns whether something was launched.
    pub browser: Box<dyn Fn(&str) -> bool>,
    /// Listen on any loopback port, not the configured one (tests, whose
    /// mock server redirects wherever it is asked to).
    pub any_port: bool,
}

impl Ctx {
    /// The binary's context: the Keychain (or `KP_KEYCHAIN_SERVICE`), the
    /// family's refresh lock, `~/.config/kp`, the system browser.
    pub fn for_binary(common: CommonArgs, config: Option<PathBuf>) -> Result<Self, CliError> {
        Ok(Ctx {
            common,
            store: ConfigStore::new(BIN).with_override(config),
            vault: Arc::new(credential_store()),
            lock: RefreshLock::for_binary(BIN)?,
            browser: Box::new(|url| {
                pk_cli_auth::oauth::open_browser(url);
                true
            }),
            any_port: false,
        })
    }

    pub fn json(&self) -> bool {
        self.common.json
    }

    pub fn config(&self) -> Result<Config, CliError> {
        let config: Config = self.store.load()?;
        config.check()?;
        Ok(config)
    }

    pub fn state_file(&self) -> Result<StateFile, CliError> {
        Ok(StateFile::beside(&self.store.path()?))
    }

    pub fn contexts(&self) -> Result<Vec<Context>, CliError> {
        Ok(self.state_file()?.load()?.contexts)
    }

    /// The signed-in member `who` names (the only one, when one is held).
    pub fn signed_in(&self, who: &PatientArgs) -> Result<Context, CliError> {
        let org = who.org.as_deref().map(kaiser::resolve_org).transpose()?;
        let contexts = self.contexts()?;
        crate::session::pick(&contexts, who.patient.as_deref(), org).cloned()
    }

    /// The client credentials for an environment, or the auth error that
    /// says how to store them.
    pub fn client(&self, environment: Environment) -> Result<ClientCredentials, CliError> {
        credentials::read_client(self.vault.as_ref(), environment)?.ok_or_else(|| {
            CliError::Auth(format!(
                "no {} client id and secret are stored",
                environment.as_str()
            ))
            .with_remedy(match environment {
                Environment::Production => "run `kp auth set-credential`",
                Environment::Sandbox => "run `kp auth set-credential --sandbox`",
            })
        })
    }

    /// A context's sign-in, renewed when its token has lapsed.
    pub fn live(&self, context: &Context) -> Result<Live<BlockingTransport>, CliError> {
        let client = self.client(context.environment()?)?;
        Live::start(
            transport()?,
            self.vault.clone(),
            self.lock.clone(),
            client,
            context,
        )
        .map_err(|e| sign_in_again(e, context))
    }

    pub fn note(&self, msg: impl AsRef<str>) {
        if !self.common.quiet {
            eprintln!("{}", msg.as_ref());
        }
    }
}

/// An auth failure's remedy, as kp spells the sign-in for this member.
pub fn sign_in_again(e: CliError, context: &Context) -> CliError {
    if e.exit_code() != 3 {
        return e;
    }
    let org = match context.environment() {
        Ok(Environment::Sandbox) => "sandbox",
        _ => kaiser::ORG_ID,
    };
    let label = context
        .label
        .as_deref()
        .map(|l| format!(" --label {l}"))
        .unwrap_or_default();
    e.with_remedy(format!("run `{BIN} auth login --org {org}{label}`"))
}

/// An error as one line for stderr, its remedy after it, for a command
/// whose stdout already carries its output.
pub fn report(e: &CliError) {
    match e.remedy() {
        Some(remedy) => eprintln!("error: {}; {remedy}", e.message()),
        None => eprintln!("error: {e}"),
    }
}

pub fn credential_store() -> CredentialStore {
    match std::env::var(KEYCHAIN_SERVICE_ENV) {
        Ok(service) if !service.trim().is_empty() => CredentialStore::new(service),
        _ => CredentialStore::for_binary(BIN),
    }
}

/// The transport for Kaiser's FHIR and OAuth endpoints: no redirects, the
/// family's 60 s timeout.
pub fn transport() -> Result<BlockingTransport, CliError> {
    BlockingTransport::new()
}

/// RFC 3339 for Unix seconds.
pub fn rfc3339(secs: u64) -> String {
    pk_cli_core::dates::fmt_rfc3339(secs as i64)
}
