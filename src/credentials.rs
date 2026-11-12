//! Where `kp` keeps secrets: the OS Keychain (service `piekstra.kp`) in the
//! binary, an in-memory map in the tests. Each item is one JSON value.
//!
//! The client id and secret Kaiser issues for this household's app are one
//! item per environment (`client/production`, `client/sandbox`). Kaiser's
//! developer terms hold both confidential, so neither is ever printed, put in
//! a file or taken from argv.

use std::cell::RefCell;
use std::collections::HashMap;

use pk_cli_core::CliError;
use pk_cli_secrets::{CredentialStore, Secret};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::kaiser::Environment;

pub trait Vault {
    fn read(&self, account: &str) -> Result<Option<Secret>, CliError>;
    fn write(&self, account: &str, value: &Secret) -> Result<(), CliError>;
    /// `true` when something was removed.
    fn delete(&self, account: &str) -> Result<bool, CliError>;
    fn service(&self) -> &str;
}

impl Vault for CredentialStore {
    fn read(&self, account: &str) -> Result<Option<Secret>, CliError> {
        self.get(account)
    }
    fn write(&self, account: &str, value: &Secret) -> Result<(), CliError> {
        self.set(account, value)
    }
    fn delete(&self, account: &str) -> Result<bool, CliError> {
        CredentialStore::delete(self, account)
    }
    fn service(&self) -> &str {
        CredentialStore::service(self)
    }
}

/// An item that is present but does not parse is an error naming the
/// account, never `None`: reading it as absent would look like an
/// unexplained sign-out.
pub fn get_json<T: DeserializeOwned>(
    vault: &dyn Vault,
    account: &str,
    remedy: &str,
) -> Result<Option<T>, CliError> {
    match vault.read(account)? {
        None => Ok(None),
        Some(secret) => serde_json::from_str(secret.expose())
            .map(Some)
            .map_err(|e| {
                CliError::Keychain(format!(
                    "the Keychain item {account} in {} is not one this version of kp can read ({})",
                    vault.service(),
                    pk_med_fhir::describe_json_error(&e)
                ))
                .with_remedy(remedy.to_string())
            }),
    }
}

pub fn set_json<T: Serialize>(vault: &dyn Vault, account: &str, value: &T) -> Result<(), CliError> {
    let text = serde_json::to_string(value)
        .map_err(|e| CliError::Other(format!("serializing the Keychain item {account}: {e}")))?;
    vault.write(account, &Secret::new(text))
}

/// The Keychain account for an environment's client registration.
pub fn client_account(environment: Environment) -> String {
    format!("client/{}", environment.as_str())
}

/// The app's client id and secret for one environment. No `Debug`: it holds
/// the secret.
#[derive(Clone, Serialize, Deserialize)]
pub struct ClientCredentials {
    pub client_id: String,
    pub client_secret: String,
}

impl ClientCredentials {
    /// Check a pasted id and secret without quoting either in an error.
    pub fn new(client_id: &str, client_secret: &str) -> Result<Self, CliError> {
        let id = client_id.trim();
        // RFC 7617: HTTP Basic's user-id cannot hold a colon.
        if id.is_empty()
            || id.len() > 200
            || id.contains(':')
            || !id.bytes().all(|b| b.is_ascii_graphic())
        {
            return Err(CliError::Usage(
                "the client id is the printable string Kaiser's portal shows for the app, with no spaces or colons".into(),
            ));
        }
        let secret = client_secret.trim();
        if secret.is_empty() || secret.len() > 1000 || secret.chars().any(char::is_control) {
            return Err(CliError::Usage(
                "the client secret is the single line Kaiser's portal showed when it was generated"
                    .into(),
            ));
        }
        Ok(ClientCredentials {
            client_id: id.to_string(),
            client_secret: secret.to_string(),
        })
    }

    /// Split `<id>:<secret>` or `<id>` and `<secret>` on two lines, as
    /// `--stdin` and `--from-env` take them.
    pub fn parse_pair(text: &Secret) -> Result<Self, CliError> {
        let text = text.expose().trim();
        let split = text
            .split_once('\n')
            .or_else(|| text.split_once(':'))
            .ok_or_else(|| {
                CliError::Usage(
                    "give the client id and secret as `<id>:<secret>`, or on two lines".into(),
                )
            })?;
        Self::new(split.0, split.1)
    }

    pub fn secret(&self) -> Secret {
        Secret::new(self.client_secret.clone())
    }
}

pub const CLIENT_REMEDY: &str = "store it again with `kp auth set-credential --overwrite`";

pub fn read_client(
    vault: &dyn Vault,
    environment: Environment,
) -> Result<Option<ClientCredentials>, CliError> {
    get_json(vault, &client_account(environment), CLIENT_REMEDY)
}

/// An in-memory vault for tests.
#[derive(Default)]
pub struct MemoryVault {
    items: RefCell<HashMap<String, String>>,
}

impl MemoryVault {
    pub fn accounts(&self) -> Vec<String> {
        let mut accounts: Vec<String> = self.items.borrow().keys().cloned().collect();
        accounts.sort();
        accounts
    }
}

impl Vault for MemoryVault {
    fn read(&self, account: &str) -> Result<Option<Secret>, CliError> {
        Ok(self
            .items
            .borrow()
            .get(account)
            .map(|s| Secret::new(s.clone())))
    }
    fn write(&self, account: &str, value: &Secret) -> Result<(), CliError> {
        self.items
            .borrow_mut()
            .insert(account.to_string(), value.expose().to_string());
        Ok(())
    }
    fn delete(&self, account: &str) -> Result<bool, CliError> {
        Ok(self.items.borrow_mut().remove(account).is_some())
    }
    fn service(&self) -> &str {
        "memory"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pair_reads_in_either_form() {
        let a = ClientCredentials::parse_pair(&Secret::new("client-one:secret-one\n")).unwrap();
        assert_eq!(
            (a.client_id.as_str(), a.client_secret.as_str()),
            ("client-one", "secret-one")
        );
        let b = ClientCredentials::parse_pair(&Secret::new("client-two\nsec:ret\n")).unwrap();
        assert_eq!(
            (b.client_id.as_str(), b.client_secret.as_str()),
            ("client-two", "sec:ret")
        );
    }

    #[test]
    fn a_bad_pair_is_a_usage_error_that_quotes_nothing() {
        for bad in [
            "only-one-part",
            "has space:secret",
            ":secret",
            "id:",
            "id\n",
        ] {
            let err = ClientCredentials::parse_pair(&Secret::new(bad))
                .err()
                .unwrap();
            assert_eq!(err.exit_code(), 2, "{bad}");
            let message = err.to_string();
            assert!(
                !message.contains("secret-") && !message.contains("has space"),
                "{message}"
            );
        }
    }

    #[test]
    fn the_client_round_trips_through_the_vault() {
        let vault = MemoryVault::default();
        assert!(read_client(&vault, Environment::Sandbox).unwrap().is_none());
        let c = ClientCredentials::new("client-one", "secret-one").unwrap();
        set_json(&vault, &client_account(Environment::Sandbox), &c).unwrap();
        assert_eq!(vault.accounts(), ["client/sandbox"]);
        let back = read_client(&vault, Environment::Sandbox).unwrap().unwrap();
        assert_eq!(back.client_id, "client-one");
        assert!(read_client(&vault, Environment::Production)
            .unwrap()
            .is_none());
    }

    #[test]
    fn an_unreadable_item_is_an_error_not_absent() {
        let vault = MemoryVault::default();
        vault
            .write(
                &client_account(Environment::Production),
                &Secret::new("{not json"),
            )
            .unwrap();
        let err = read_client(&vault, Environment::Production).err().unwrap();
        assert!(err.to_string().contains("client/production"), "{err}");
    }
}
