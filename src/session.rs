//! The member sign-ins this install holds: one Keychain item per
//! (organization, member) holding pk-med-smart's `SmartSession`, and a row
//! for each in `state.json` (`Context`), so `patients list` and a logged-out
//! `auth status` open no Keychain item.

use std::sync::{Arc, Mutex};

use pk_cli_auth::oauth::{ClientAuth, OAuthClient};
use pk_cli_auth::refresh_lock::RefreshLock;
use pk_cli_core::transport::Transport;
use pk_cli_core::CliError;
use pk_cli_secrets::Secret;
use pk_med_records::dto::{kind, relationship};
use pk_med_records::PatientContext;
use pk_med_smart::{decode_session, Renewer, SessionStore, SmartSession, SESSION_FORMAT};
use serde::{Deserialize, Serialize};

use crate::credentials::{ClientCredentials, Vault};
use crate::kaiser::{Environment, KaiserQuirks};

/// One row of `state.json`'s `contexts`: everything about a sign-in but its
/// tokens.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Context {
    /// The member's FHIR `Patient.id` at Kaiser (`--patient`).
    pub id: String,
    /// kp's organization id (`kaiser-health-plan`, or the sandbox's).
    pub organization: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relationship: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fhir_user: Option<String>,
    /// RFC 3339.
    pub signed_in_at: String,
}

impl Context {
    pub fn from_session(
        session: &SmartSession,
        label: Option<String>,
        signed_in_at: String,
    ) -> Self {
        let id = session.patient.clone().unwrap_or_default();
        Context {
            relationship: relationship_of(session.fhir_user.as_deref(), &id),
            id,
            organization: session.organization.clone(),
            label,
            fhir_user: session.fhir_user.clone(),
            signed_in_at,
        }
    }

    pub fn account(&self) -> String {
        pk_med_smart::account(&self.organization, &self.id)
    }

    pub fn environment(&self) -> Result<Environment, CliError> {
        Environment::of_org(&self.organization).ok_or_else(|| {
            CliError::Other(format!(
                "state.json names an organization kp does not know ({})",
                self.organization
            ))
        })
    }

    /// The label, else the FHIR id.
    pub fn display(&self) -> String {
        self.label.clone().unwrap_or_else(|| self.id.clone())
    }

    /// The `patient-list/v1` item and the stream header's `patient`. The
    /// name is never read from the record.
    pub fn patient_context(&self) -> PatientContext {
        PatientContext {
            id: self.id.clone(),
            organization: self.organization.clone(),
            label: self.label.clone(),
            relationship: self.relationship.clone(),
            kind: Some(kind::MEMBER.to_string()),
            member_role: None,
            fhir_user: self.fhir_user.clone(),
            name: None,
            session: Some(self.account()),
        }
    }
}

/// `self` when the signed-in user is the member, `proxy` when it is someone
/// else (a parent acting for a child); unknown when Kaiser does not say.
fn relationship_of(fhir_user: Option<&str>, patient: &str) -> Option<String> {
    let user = fhir_user?;
    let own =
        user == format!("Patient/{patient}") || user.ends_with(&format!("/Patient/{patient}"));
    Some(
        if own {
            relationship::SELF
        } else {
            relationship::PROXY
        }
        .to_string(),
    )
}

/// Resolve `--patient` (and `--org`) against the held contexts: with none
/// given and one held, that one.
pub fn pick<'a>(
    contexts: &'a [Context],
    patient: Option<&str>,
    org: Option<Environment>,
) -> Result<&'a Context, CliError> {
    let narrowed: Vec<Context> = contexts
        .iter()
        .filter(|c| org.is_none_or(|e| c.organization == e.org_id()))
        .cloned()
        .collect();
    if narrowed.is_empty() {
        return Err(CliError::Auth(match org {
            Some(e) if !contexts.is_empty() => format!("no member is signed in at {}", e.org_id()),
            _ => "no member is signed in".to_string(),
        })
        .with_remedy("run `kp auth login --org kaiser-health-plan --label <WHO>`"));
    }
    let chosen = match patient {
        None if narrowed.len() == 1 => narrowed[0].clone(),
        None => {
            return Err(CliError::Usage(format!(
                "{} members are signed in; name one with --patient ({})",
                narrowed.len(),
                narrowed
                    .iter()
                    .map(Context::display)
                    .collect::<Vec<_>>()
                    .join(", ")
            )))
        }
        Some(q) => pk_cli_core::resolve::pick(
            &narrowed,
            q,
            |c| vec![c.id.clone()],
            |c| c.label.as_deref().unwrap_or(&c.id),
            "signed-in member",
        )?
        .clone(),
    };
    contexts
        .iter()
        .find(|c| c.organization == chosen.organization && c.id == chosen.id)
        .ok_or_else(|| CliError::Other("the chosen member vanished from the list".into()))
}

/// pk-med-smart's sessions, in kp's Keychain items: read with
/// `decode_session` (which refuses an item from a newer release) and
/// written stamped with this release's `SESSION_FORMAT`. An error never
/// names the account, which holds the member's id.
#[derive(Clone)]
pub struct Sessions(pub Arc<dyn Vault>);

impl Sessions {
    fn hide(&self, e: CliError, account: &str) -> CliError {
        let text = e.to_string();
        if !text.contains(account) {
            return e;
        }
        let shown = match account.rsplit_once('/') {
            Some((head, _)) => format!("{head}/…"),
            None => "a sign-in".to_string(),
        };
        CliError::Keychain(text.replace(account, &shown))
    }
}

impl SessionStore for Sessions {
    fn load(&self, account: &str) -> Result<Option<SmartSession>, CliError> {
        let Some(text) = self.0.read(account).map_err(|e| self.hide(e, account))? else {
            return Ok(None);
        };
        decode_session(text.expose()).map(Some)
    }

    fn save(&self, session: &SmartSession) -> Result<(), CliError> {
        let account = session.account();
        let mut value = serde_json::to_value(session)
            .map_err(|_| CliError::Other("a sign-in did not serialize".into()))?;
        value["format"] = serde_json::Value::from(SESSION_FORMAT);
        self.0
            .write(&account, &Secret::new(value.to_string()))
            .map_err(|e| self.hide(e, &account))
    }

    fn delete(&self, account: &str) -> Result<bool, CliError> {
        self.0.delete(account).map_err(|e| self.hide(e, account))
    }
}

/// How kp authenticates to Kaiser's token endpoint: the confidential
/// client's id and secret as an HTTP Basic header.
pub fn client_auth(client: &ClientCredentials) -> ClientAuth {
    ClientAuth::SecretBasic {
        client_id: client.client_id.clone(),
        secret: client.secret(),
    }
}

pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// A sign-in in use: its session renewed when lapsed, and renewed again
/// when Kaiser refuses its token mid-run. Owns what renewal needs, so the
/// FHIR client can call it from its `bearer_with` source.
#[derive(Clone)]
pub struct Live<T> {
    pub account: String,
    sessions: Sessions,
    lock: RefreshLock,
    oauth: OAuthClient<T>,
    client: ClientCredentials,
    current: Arc<Mutex<SmartSession>>,
}

impl<T: Transport + Clone + Send + Sync + 'static> Live<T> {
    /// The context's session, renewed first when its token has lapsed.
    pub fn start(
        transport: T,
        vault: Arc<dyn Vault>,
        lock: RefreshLock,
        client: ClientCredentials,
        context: &Context,
    ) -> Result<Self, CliError> {
        let account = context.account();
        let sessions = Sessions(vault);
        // Where the stored sign-in points is checked before a renewal sends
        // the client id and secret to its token endpoint.
        if let Some(stored) = sessions.load(&account)? {
            let environment = context.environment()?;
            crate::kaiser::require_server(environment, &stored.fhir_base)?;
            crate::kaiser::require_server(environment, &stored.token_endpoint)?;
        }
        let oauth = OAuthClient::new(transport, crate::commands::BIN, env!("CARGO_PKG_VERSION"));
        let auth = client_auth(&client);
        let session =
            Renewer::new(&sessions, &lock, &oauth, &auth, &KaiserQuirks).fresh(&account, now())?;
        Ok(Live {
            account,
            sessions,
            lock,
            oauth,
            client,
            current: Arc::new(Mutex::new(session)),
        })
    }

    pub fn session(&self) -> SmartSession {
        self.current.lock().expect("the session lock").clone()
    }

    pub fn fhir_base(&self) -> String {
        self.session().fhir_base
    }

    /// The bearer for the next request: the current token, or after Kaiser
    /// refused `refused`, a renewed one.
    pub fn bearer(&self, refused: Option<&Secret>) -> Result<Secret, CliError> {
        let Some(refused) = refused else {
            return Ok(self.session().bearer());
        };
        let auth = client_auth(&self.client);
        let renewed = Renewer::new(
            &self.sessions,
            &self.lock,
            &self.oauth,
            &auth,
            &KaiserQuirks,
        )
        .after_refusal(&self.account, refused.expose(), now())?;
        let bearer = renewed.bearer();
        *self.current.lock().expect("the session lock") = renewed;
        Ok(bearer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(id: &str, label: Option<&str>, org: Environment) -> Context {
        Context {
            id: id.into(),
            organization: org.org_id().into(),
            label: label.map(Into::into),
            relationship: None,
            fhir_user: None,
            signed_in_at: "2020-01-01T00:00:00Z".into(),
        }
    }

    #[test]
    fn pick_takes_the_only_one_or_asks() {
        let one = [ctx("pat-0001", Some("adult1"), Environment::Sandbox)];
        assert_eq!(pick(&one, None, None).unwrap().id, "pat-0001");
        assert_eq!(pick(&[], None, None).unwrap_err().exit_code(), 3);
        let two = [
            ctx("pat-0001", Some("adult1"), Environment::Sandbox),
            ctx("pat-0002", Some("child1"), Environment::Sandbox),
        ];
        assert_eq!(pick(&two, None, None).unwrap_err().exit_code(), 2);
        assert_eq!(pick(&two, Some("child1"), None).unwrap().id, "pat-0002");
        assert_eq!(pick(&two, Some("pat-0001"), None).unwrap().id, "pat-0001");
        assert_eq!(pick(&two, Some("nobody"), None).unwrap_err().exit_code(), 4);
        assert_eq!(
            pick(&two, None, Some(Environment::Production))
                .unwrap_err()
                .exit_code(),
            3
        );
    }

    #[test]
    fn relationship_comes_from_fhir_user_only() {
        assert_eq!(relationship_of(None, "p1"), None);
        assert_eq!(
            relationship_of(Some("Patient/p1"), "p1").as_deref(),
            Some("self")
        );
        assert_eq!(
            relationship_of(Some("https://x.example/fhir/Patient/p1"), "p1").as_deref(),
            Some("self")
        );
        assert_eq!(
            relationship_of(Some("RelatedPerson/r1"), "p1").as_deref(),
            Some("proxy")
        );
    }

    #[test]
    fn a_context_is_a_member_with_its_keychain_account() {
        let c = ctx("pat-0001", Some("adult1"), Environment::Sandbox).patient_context();
        assert_eq!(c.kind.as_deref(), Some("member"));
        assert_eq!(
            c.session.as_deref(),
            Some("smart/kaiser-health-plan-sandbox/pat-0001")
        );
        assert!(c.name.is_none());
    }
}
