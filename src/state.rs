//! The non-secret record of what this install keeps in the Keychain, as
//! `state.json` beside the config: which client registrations are stored.
//! `auth status` reads it first, so a command run with nothing stored opens
//! no Keychain item and raises no prompt.

use std::path::{Path, PathBuf};

use pk_cli_core::CliError;
use serde::{Deserialize, Serialize};

use crate::kaiser::Environment;

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct State {
    /// The environments whose client id and secret are in the Keychain.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub clients: Vec<Environment>,
}

impl State {
    pub fn has_client(&self, environment: Environment) -> bool {
        self.clients.contains(&environment)
    }

    pub fn add_client(&mut self, environment: Environment) {
        if !self.has_client(environment) {
            self.clients.push(environment);
        }
    }

    pub fn remove_client(&mut self, environment: Environment) {
        self.clients.retain(|e| *e != environment);
    }
}

/// `state.json` in the config file's directory.
pub struct StateFile {
    path: PathBuf,
}

impl StateFile {
    pub fn beside(config_path: &Path) -> Self {
        let dir = config_path.parent().unwrap_or(Path::new("."));
        StateFile {
            path: dir.join("state.json"),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn load(&self) -> Result<State, CliError> {
        match std::fs::read_to_string(&self.path) {
            Ok(text) => serde_json::from_str(&text).map_err(|e| {
                CliError::Other(format!(
                    "{} {}",
                    self.path.display(),
                    pk_med_fhir::describe_json_error(&e)
                ))
            }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(State::default()),
            Err(e) => Err(CliError::Other(format!(
                "reading {}: {e}",
                self.path.display()
            ))),
        }
    }

    pub fn save(&self, state: &State) -> Result<(), CliError> {
        let body = serde_json::to_string_pretty(state)
            .map_err(|e| CliError::Other(format!("serializing {}: {e}", self.path.display())))?;
        crate::fsutil::write_private(&self.path, body.as_bytes())
    }

    pub fn clear(&self) -> Result<bool, CliError> {
        match std::fs::remove_file(&self.path) {
            Ok(()) => Ok(true),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(CliError::Other(format!(
                "removing {}: {e}",
                self.path.display()
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clients_are_recorded_once_and_removed() {
        let dir = tempfile::tempdir().unwrap();
        let file = StateFile::beside(&dir.path().join("kp").join("config.json"));
        assert!(file.load().unwrap().clients.is_empty());
        let mut state = State::default();
        state.add_client(Environment::Sandbox);
        state.add_client(Environment::Sandbox);
        file.save(&state).unwrap();
        let back = file.load().unwrap();
        assert_eq!(back.clients, [Environment::Sandbox]);
        let mut back = back;
        back.remove_client(Environment::Sandbox);
        assert!(!back.has_client(Environment::Sandbox));
        assert!(file.clear().unwrap());
        assert!(!file.clear().unwrap());
    }
}
