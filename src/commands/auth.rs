//! `kp auth set-credential|login|status|logout`.

use std::io::IsTerminal;
use std::time::Duration;

use pk_cli_auth::oauth::OAuthClient;
use pk_cli_auth::{AuthLogin, AuthMethod, AuthStatus};
use pk_cli_core::{block_on, output, CliError};
use pk_cli_secrets::Secret;
use pk_med_fhir::FhirClient;
use pk_med_smart::{begin, discover, LaunchRequest, SessionStore, SmartApp, SmartSession};
use serde_json::{json, Value};

use super::{rfc3339, transport, Ctx, BIN};
use crate::cli::{AuthCmd, LoginArgs, LogoutArgs, SetCredentialArgs};
use crate::credentials::{self, client_account, ClientCredentials};
use crate::kaiser::{self, Environment, KaiserQuirks};
use crate::session::{client_auth, now, Context, Sessions};

pub fn run(ctx: &Ctx, cmd: &AuthCmd) -> Result<(), CliError> {
    match cmd {
        AuthCmd::SetCredential(args) => set_credential(ctx, args),
        AuthCmd::Login(args) => login(ctx, args),
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

/// A label names a person in this household (`adult1`), never read from a
/// record: short, printable, no spaces.
fn check_label(label: &str) -> Result<(), CliError> {
    let ok = !label.is_empty()
        && label.len() <= 40
        && label
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c));
    if ok {
        Ok(())
    } else {
        Err(CliError::Usage(
            "a label is up to 40 letters, digits, `-`, `_` or `.` (`adult1`, `child1`)".into(),
        ))
    }
}

fn login(ctx: &Ctx, args: &LoginArgs) -> Result<(), CliError> {
    let environment = kaiser::resolve_org(&args.org)?;
    if let Some(label) = &args.label {
        check_label(label)?;
    }
    if args.timeout == 0 || args.timeout > 60 {
        return Err(CliError::Usage("--timeout is from 1 to 60 minutes".into()));
    }
    let config = ctx.config()?;
    let client = ctx.client(environment)?;
    let base = config.fhir_base();
    let t = transport()?;
    let smart = block_on(discover(
        &t,
        &base,
        &KaiserQuirks,
        BIN,
        env!("CARGO_PKG_VERSION"),
    ))?;
    ctx.lock.before_signin()?;
    let app = SmartApp {
        app: BIN.into(),
        client: client_auth(&client),
        ports: if ctx.any_port {
            Vec::new()
        } else {
            vec![config.redirect_port()]
        },
    };
    let pending = begin(
        &app,
        LaunchRequest {
            config: &smart,
            fhir_base: &base,
            aud: None,
            scopes: environment.scopes(),
            quirks: &KaiserQuirks,
        },
    )?;
    if args.no_browser || !(ctx.browser)(pending.url()) {
        ctx.note("Open this address in a browser and sign in to kp.org:");
        eprintln!("{}", pending.url());
    } else {
        ctx.note("Signing in to kp.org in your browser; waiting for it to finish.");
    }
    let code = pending.wait_for_code(Duration::from_secs(args.timeout * 60), |reason| {
        ctx.note(format!(
            "{reason}. Still waiting; press Ctrl-C to start again."
        ))
    })?;
    let oauth = OAuthClient::new(t.clone(), BIN, env!("CARGO_PKG_VERSION"));
    let session = block_on(pending.exchange(
        &oauth,
        &client_auth(&client),
        &code,
        environment.org_id(),
        now(),
    ))?;
    if session.patient.as_deref().is_none_or(str::is_empty) {
        return Err(CliError::Upstream(
            "Kaiser's token answer named no member (the launch/patient context); nothing was stored"
                .into(),
        ));
    }
    let sessions = Sessions(ctx.vault.clone());
    ctx.lock.store_signin(|| sessions.save(&session))?;
    let context = Context::from_session(&session, args.label.clone(), rfc3339(now()));
    let state_file = ctx.state_file()?;
    let mut state = state_file.load()?;
    state.upsert_context(context.clone());
    state_file.save(&state)?;
    let verified = verify(&t, &session, &context.id);
    if !verified {
        ctx.note(
            "Signed in, but reading the member's Patient record failed; `kp auth status` shows the sign-in.",
        );
    }
    let mut v = AuthLogin::stored(verified).to_json();
    v["patient"] = serde_json::to_value(context.patient_context()).unwrap_or_default();
    output::emit(ctx.json(), "auth-login", v, |_| {
        println!(
            "Signed in member {} at {}.",
            context.display(),
            context.organization
        )
    });
    Ok(())
}

/// One read of the member's own Patient record, kept nowhere: whether the
/// new sign-in works.
fn verify(t: &pk_cli_http::BlockingTransport, session: &SmartSession, patient: &str) -> bool {
    FhirClient::new(t, &session.fhir_base, BIN, env!("CARGO_PKG_VERSION"))
        .map(|c| c.bearer(session.bearer()))
        .and_then(|c| block_on(c.read("Patient", patient)))
        .is_ok()
}

fn status(ctx: &Ctx) -> Result<(), CliError> {
    let state = ctx.state_file()?.load()?;
    let sessions = Sessions(ctx.vault.clone());
    let mut clients = Vec::new();
    let mut any_client = false;
    for environment in Environment::ALL {
        // The Keychain is read only for what the state records, so a
        // logged-out status raises no prompt.
        let stored = state.has_client(environment)
            && credentials::read_client(ctx.vault.as_ref(), environment)?.is_some();
        any_client |= stored;
        clients.push(json!({ "environment": environment.as_str(), "stored": stored }));
    }
    let mut rows = Vec::new();
    let mut any_stored = false;
    let mut any_usable = false;
    for c in &state.contexts {
        let session = sessions.load(&c.account())?;
        let usable = session
            .as_ref()
            .is_some_and(|s| !s.tokens.is_expired(now(), 0) || s.renewable());
        any_stored |= session.is_some();
        any_usable |= usable;
        let mut row = serde_json::to_value(c.patient_context()).unwrap_or_default();
        row["credential_in_keychain"] = session.is_some().into();
        row["usable"] = usable.into();
        row["renewable"] = session.as_ref().is_some_and(SmartSession::renewable).into();
        if let Some(at) = session.as_ref().and_then(|s| s.tokens.expires_at_rfc3339()) {
            row["token_expires_at"] = at.into();
        }
        rows.push(row);
    }
    let mut status = AuthStatus::new(true, any_usable, AuthMethod::BrowserSession);
    status.credential_in_keychain = Some(any_stored || any_client);
    status.session_valid = Some(any_usable);
    let mut v = status.to_json();
    v["clients"] = Value::Array(clients.clone());
    v["patients"] = Value::Array(rows.clone());
    if ctx.json() {
        output::json(&v);
    } else {
        status.render();
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
        if rows.is_empty() {
            println!("Members:       none signed in (`kp auth login --label <WHO>`)");
        } else {
            println!();
            let view: Vec<Value> = rows
                .iter()
                .map(|r| {
                    json!({
                        "member": r["label"].as_str().unwrap_or(r["id"].as_str().unwrap_or_default()),
                        "organization": r["organization"],
                        "relationship": r["relationship"],
                        "usable": r["usable"],
                        "renewable": r["renewable"],
                        "token_expires": r["token_expires_at"],
                    })
                })
                .collect();
            output::table(&view);
        }
    }
    Ok(())
}

fn logout(ctx: &Ctx, args: &LogoutArgs) -> Result<(), CliError> {
    let org = args
        .who
        .org
        .as_deref()
        .map(kaiser::resolve_org)
        .transpose()?;
    let state_file = ctx.state_file()?;
    let mut state = state_file.load()?;
    let chosen: Vec<Context> = match &args.who.patient {
        Some(q) => vec![crate::session::pick(&state.contexts, Some(q), org)?.clone()],
        None => state
            .contexts
            .iter()
            .filter(|c| org.is_none_or(|e| c.organization == e.org_id()))
            .cloned()
            .collect(),
    };
    let sessions = Sessions(ctx.vault.clone());
    let mut removed = Vec::new();
    for c in &chosen {
        let account = c.account();
        if ctx.lock.forget(|| sessions.delete(&account))? {
            removed.push(account);
        }
        state.remove_context(&c.organization, &c.id);
    }
    state_file.save(&state)?;
    let mut config_cleared = false;
    let mut state_cleared = false;
    if args.forget {
        for environment in Environment::ALL {
            let account = client_account(environment);
            if ctx.vault.delete(&account)? {
                removed.push(account);
            }
        }
        config_cleared = ctx.store.clear()?;
        state_cleared = state_file.clear()?;
    }
    let done = json!({
        "removed": removed,
        "signed_out": chosen.iter().map(Context::display).collect::<Vec<_>>(),
        "config_cleared": config_cleared,
        "state_cleared": state_cleared,
    });
    output::emit(ctx.json(), "auth-logout", done, |v| {
        let members = v["signed_out"].as_array().map_or(0, Vec::len);
        println!(
            "Signed out {members} member(s){}.",
            if args.forget {
                "; removed the client credentials, the config and the state file"
            } else {
                ""
            }
        );
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::PatientArgs;
    use crate::credentials::MemoryVault;
    use pk_cli_auth::refresh_lock::RefreshLock;
    use pk_cli_config::ConfigStore;
    use std::sync::Arc;

    fn ctx(dir: &std::path::Path) -> Ctx {
        Ctx {
            common: Default::default(),
            store: ConfigStore::new("kp").with_override(Some(dir.join("config.json"))),
            vault: Arc::new(MemoryVault::default()),
            lock: RefreshLock::at(dir.join("refresh.lock")),
            browser: Box::new(|_| false),
            any_port: true,
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
        for environment in Environment::ALL {
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
        logout(
            &ctx,
            &LogoutArgs {
                who: PatientArgs::default(),
                forget: true,
            },
        )
        .unwrap();
        assert!(ctx.vault.read("client/production").unwrap().is_none());
        assert!(ctx.vault.read("client/sandbox").unwrap().is_none());
        assert!(!state_file.path().exists());
    }

    #[test]
    fn labels_are_short_and_plain() {
        for ok in ["adult1", "child-2", "a.b_c"] {
            check_label(ok).unwrap();
        }
        let long = "x".repeat(41);
        for bad in ["", "has space", "ünïcode", long.as_str()] {
            assert_eq!(check_label(bad).unwrap_err().exit_code(), 2, "{bad}");
        }
    }
}
