//! Non-secret settings (`~/.config/kp/config.json`). The client id and secret
//! Kaiser issues are credentials under Kaiser's developer terms, so they live
//! in the Keychain (`kp auth client set`), never here.

use pk_cli_core::CliError;
use serde::{Deserialize, Serialize};

use crate::kaiser::{Environment, FHIR_BASE};

/// The loopback port the sign-in's redirect comes back to. Kaiser matches the
/// redirect URI registered in its developer portal, so the port is fixed.
pub const DEFAULT_REDIRECT_PORT: u16 = 7469;
pub const REDIRECT_PATH: &str = "/callback";
pub const REDIRECT_HOST: &str = "127.0.0.1";

pub const KEYS: &str = "environment, redirect_port, fhir_base";

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Config {
    /// `production` (the default) or `sandbox`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub environment: Option<Environment>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub redirect_port: Option<u16>,
    /// Another FHIR base: a local test server (`http://127.0.0.1:<port>`), or
    /// Kaiser's if it moves. Only https, or http to this machine.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fhir_base: Option<String>,
}

impl Config {
    pub fn environment(&self) -> Environment {
        self.environment.unwrap_or(Environment::Production)
    }

    pub fn redirect_port(&self) -> u16 {
        self.redirect_port.unwrap_or(DEFAULT_REDIRECT_PORT)
    }

    /// `http://127.0.0.1:<port>/callback`, the URI registered with Kaiser.
    pub fn redirect_uri(&self) -> String {
        format!(
            "http://{REDIRECT_HOST}:{}{REDIRECT_PATH}",
            self.redirect_port()
        )
    }

    pub fn fhir_base(&self) -> String {
        pk_med_fhir::normalize_base(self.fhir_base.as_deref().unwrap_or(FHIR_BASE))
    }

    pub fn set(&mut self, key: &str, value: &str) -> Result<(), CliError> {
        match key {
            "environment" => {
                self.environment = Some(Environment::parse(value).ok_or_else(|| {
                    CliError::Usage("environment is production or sandbox".into())
                })?)
            }
            "redirect_port" => {
                let port: u16 = value.parse().ok().filter(|p| *p >= 1024).ok_or_else(|| {
                    CliError::Usage(format!("`{value}` is not a port from 1024 to 65535"))
                })?;
                self.redirect_port = Some(port);
            }
            "fhir_base" => {
                pk_cli_core::transport::require_secure_url(value)?;
                self.fhir_base = Some(pk_med_fhir::normalize_base(value));
            }
            other => return Err(unknown(other)),
        }
        Ok(())
    }

    pub fn unset(&mut self, key: &str) -> Result<(), CliError> {
        match key {
            "environment" => self.environment = None,
            "redirect_port" => self.redirect_port = None,
            "fhir_base" => self.fhir_base = None,
            other => return Err(unknown(other)),
        }
        Ok(())
    }
}

fn unknown(key: &str) -> CliError {
    CliError::Usage(format!("unknown config key `{key}` (known: {KEYS})"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults() {
        let c = Config::default();
        assert_eq!(c.environment(), Environment::Production);
        assert_eq!(c.redirect_uri(), "http://127.0.0.1:7469/callback");
        assert_eq!(c.fhir_base(), FHIR_BASE);
    }

    #[test]
    fn keys_set_and_unset() {
        let mut c = Config::default();
        c.set("environment", "sandbox").unwrap();
        assert_eq!(c.environment(), Environment::Sandbox);
        assert!(c.set("environment", "staging").is_err());
        c.set("redirect_port", "8123").unwrap();
        assert_eq!(c.redirect_uri(), "http://127.0.0.1:8123/callback");
        assert!(c.set("redirect_port", "80").is_err());
        assert!(c.set("fhir_base", "http://example.com/fhir").is_err());
        c.set("fhir_base", "http://127.0.0.1:9/fhir/").unwrap();
        assert_eq!(c.fhir_base(), "http://127.0.0.1:9/fhir");
        c.unset("fhir_base").unwrap();
        assert_eq!(c.fhir_base(), FHIR_BASE);
        assert_eq!(c.set("nope", "x").unwrap_err().exit_code(), 2);
    }
}
