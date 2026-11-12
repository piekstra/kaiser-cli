//! `kp auth set-credential|status|logout`.

use std::io::IsTerminal;

use pk_cli_auth::{AuthMethod, AuthStatus};
use pk_cli_core::{output, CliError};
use pk_cli_secrets::Secret;
use serde_json::{json, Value};

use super::Ctx;
use crate::cli::{AuthCmd, LogoutArgs, SetCredentialArgs};
use crate::credentials::{self, client_account, ClientCredentials};
use crate::kaiser::Environment;

const ENVIRONMENTS: [Environment; 2] = [Environment::Production, Environment::Sandbox];

pub fn run(ctx: &Ctx, cmd: &AuthCmd) -> Result<(), CliError> {
    match cmd {
        AuthCmd::SetCredential(args) => set_credential(ctx, args),
        AuthCmd::Status => status(ctx),
        AuthCmd::Logout(args) => logout(ctx, args),
    }
}

fn set_credential(ctx: &Ctx, args: &SetCredentialArgs) -> Result<(), CliError> {
    if args.source.stdin && args.source.from_env.is_some() {
        return Err(CliError::Usage(
            "pass exactly one of --stdin or --from-env".into(),
        ));
    }
    let environment = if args.sandbox {
        Environment::Sandbox
    } else {
        Environment::Production
    };
    let interactive =
        !args.source.stdin && args.source.from_env.is_none() && std::io::stdin().is_terminal();
    // Input that comes from a flag or a pipe is checked before the Keychain
    // is opened; a prompt waits until the refusal below, so nobody types a
    // secret only to have it turned away.
    let piped = if interactive {
        None
    } else {
        Some(read_piped(args)?)
    };
    let state_file = ctx.state_file()?;
    let mut state = state_file.load()?;
    if !args.overwrite
        && (state.has_client(environment)
            || credentials::read_client(ctx.vault.as_ref(), environment)?.is_some())
    {
        return Err(CliError::ConfirmationRequired(format!(
            "a {} client is already stored",
            environment.as_str()
        ))
        .with_remedy("pass --overwrite to replace it"));
    }
    let client = match piped {
        Some(client) => client,
        None => prompt_client()?,
    };
    credentials::set_json(ctx.vault.as_ref(), &client_account(environment), &client)?;
    state.add_client(environment);
    state_file.save(&state)?;
    let done = json!({
        "environment": environment.as_str(),
        "stored": true,
        "keychain_service": ctx.vault.service(),
        "keychain_account": client_account(environment),
    });
    output::emit(ctx.json(), "client-credential", done, |_| {
        println!(
            "Stored the {} client id and secret in the Keychain.",
            environment.as_str()
        )
    });
    Ok(())
}

/// The id and secret at two prompts that do not echo.
fn prompt_client() -> Result<ClientCredentials, CliError> {
    let id = Secret::prompt("Kaiser client id (not echoed)")?;
    let secret = Secret::prompt("Kaiser client secret (not echoed)")?;
    ClientCredentials::new(id.expose(), secret.expose())
}

/// The pair from `--stdin`, `--from-env`, or stdin when it is not a terminal.
fn read_piped(args: &SetCredentialArgs) -> Result<ClientCredentials, CliError> {
    let pair = if args.source.stdin || args.source.from_env.is_some() {
        args.source.read(None)?
    } else {
        pk_cli_secrets::read_stdin()?
    };
    ClientCredentials::parse_pair(&pair)
}

fn status(ctx: &Ctx) -> Result<(), CliError> {
    let config = ctx.config()?;
    let state = ctx.state_file()?.load()?;
    let mut clients = Vec::new();
    for environment in ENVIRONMENTS {
        // The Keychain is read only for a client the state records, so a
        // logged-out status raises no prompt.
        let stored = state.has_client(environment)
            && credentials::read_client(ctx.vault.as_ref(), environment)?.is_some();
        clients.push(json!({ "environment": environment.as_str(), "stored": stored }));
    }
    let selected = config.environment();
    let client_stored = clients
        .iter()
        .any(|c| c["environment"] == selected.as_str() && c["stored"] == true);
    let mut status = AuthStatus::new(true, false, AuthMethod::BrowserSession);
    status.credential_in_keychain = Some(client_stored);
    status.session_valid = Some(false);
    let mut v = status.to_json();
    v["environment"] = selected.as_str().into();
    v["clients"] = Value::Array(clients.clone());
    v["patients"] = Value::Array(Vec::new());
    if ctx.json() {
        output::json(&v);
    } else {
        status.render();
        println!("Environment:   {}", selected.as_str());
        for c in &clients {
            println!(
                "Client ({}): {}",
                c["environment"].as_str().unwrap_or_default(),
                if c["stored"] == true {
                    "stored"
                } else {
                    "not stored (`kp auth set-credential`)"
                }
            );
        }
        println!("Patients:      none signed in");
    }
    Ok(())
}

fn logout(ctx: &Ctx, args: &LogoutArgs) -> Result<(), CliError> {
    let mut removed = Vec::new();
    let mut config_cleared = false;
    let mut state_cleared = false;
    if args.forget {
        for environment in ENVIRONMENTS {
            let account = client_account(environment);
            if ctx.vault.delete(&account)? {
                removed.push(account);
            }
        }
        config_cleared = ctx.store.clear()?;
        state_cleared = ctx.state_file()?.clear()?;
    }
    let done = json!({
        "removed": removed,
        "config_cleared": config_cleared,
        "state_cleared": state_cleared,
    });
    output::emit(ctx.json(), "auth-logout", done, |v| {
        let removed = v["removed"].as_array().map_or(0, Vec::len);
        if args.forget {
            println!(
                "Removed {removed} client credential(s){}{}.",
                if v["config_cleared"] == true {
                    ", the config"
                } else {
                    ""
                },
                if v["state_cleared"] == true {
                    ", the state file"
                } else {
                    ""
                },
            );
        } else {
            println!("No sign-ins stored; nothing to remove (--forget also removes the client credentials).");
        }
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::credentials::MemoryVault;
    use pk_cli_config::ConfigStore;

    fn ctx(dir: &std::path::Path) -> Ctx {
        Ctx {
            common: Default::default(),
            store: ConfigStore::new("kp").with_override(Some(dir.join("config.json"))),
            vault: Box::new(MemoryVault::default()),
        }
    }

    #[test]
    fn set_credential_refuses_to_replace_without_overwrite() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = ctx(dir.path());
        let account = client_account(Environment::Production);
        credentials::set_json(
            ctx.vault.as_ref(),
            &account,
            &ClientCredentials::new("client-one", "secret-one").unwrap(),
        )
        .unwrap();
        // A variable only this test reads, so parallel tests do not race.
        std::env::set_var("KP_TEST_REFUSE_PAIR", "client-two:secret-two");
        let mut args = SetCredentialArgs {
            source: pk_cli_secrets::SecretSourceArgs {
                stdin: false,
                from_env: Some("KP_TEST_REFUSE_PAIR".into()),
            },
            sandbox: false,
            overwrite: false,
        };
        let err = set_credential(&ctx, &args).unwrap_err();
        assert_eq!(err.exit_code(), 6);
        args.overwrite = true;
        set_credential(&ctx, &args).unwrap();
        let stored = credentials::read_client(ctx.vault.as_ref(), Environment::Production)
            .unwrap()
            .unwrap();
        assert_eq!(stored.client_id, "client-two");
        assert!(ctx
            .state_file()
            .unwrap()
            .load()
            .unwrap()
            .has_client(Environment::Production));
    }

    #[test]
    fn logout_forget_removes_both_clients_and_the_files() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = ctx(dir.path());
        for environment in ENVIRONMENTS {
            credentials::set_json(
                ctx.vault.as_ref(),
                &client_account(environment),
                &ClientCredentials::new("client-one", "secret-one").unwrap(),
            )
            .unwrap();
        }
        let state_file = ctx.state_file().unwrap();
        let mut state = state_file.load().unwrap();
        state.add_client(Environment::Production);
        state_file.save(&state).unwrap();
        logout(&ctx, &LogoutArgs { forget: true }).unwrap();
        assert!(ctx.vault.read("client/production").unwrap().is_none());
        assert!(ctx.vault.read("client/sandbox").unwrap().is_none());
        assert!(!state_file.path().exists());
    }
}
