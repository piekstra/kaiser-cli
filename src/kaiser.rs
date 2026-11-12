//! Kaiser Permanente's health-plan Patient Access API: the one organization
//! `kp` reaches, its endpoints and the scopes it requires. `docs/api.md` has
//! the sources.

use pk_med_records::Organization;

/// `kp`'s id for the health plan, the `--org` value and the keychain prefix.
pub const ORG_ID: &str = "kaiser-health-plan";

pub const ORG_NAME: &str = "Kaiser Permanente health plan";

/// The FHIR base of the Patient Access API (Smile CDR behind Apigee), one for
/// every region. Kaiser's guide also writes `[base]/HPEOB/...` in its search
/// table; its own example request has no `HPEOB`.
pub const FHIR_BASE: &str =
    "https://kpx-service-bus.kp.org/service/cdo/siae/healthplankpxv1rc/FHIR/api";

/// The regions the API covers, by Kaiser's guide, for `orgs search`.
pub const REGIONS: &[&str] = &[
    "Northern California",
    "Southern California",
    "Colorado",
    "Georgia",
    "Hawaii",
    "Mid-Atlantic States",
    "Northwest",
    "Washington",
];

/// The scopes Kaiser requires in production. `ExplanationOfBenefit` is
/// Kaiser's own scope token, not a SMART one.
pub const PRODUCTION_SCOPE: &str =
    "patient/*.read launch/patient offline_access ExplanationOfBenefit";

/// Added to the production scopes for Kaiser's sandbox.
pub const SANDBOX_SCOPE: &str = "sandbox";

/// Which of the two client registrations a command uses. Both reach the same
/// hosts; the sandbox is selected by its own client and the `sandbox` scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Environment {
    Production,
    Sandbox,
}

impl Environment {
    pub fn as_str(self) -> &'static str {
        match self {
            Environment::Production => "production",
            Environment::Sandbox => "sandbox",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim() {
            "production" => Some(Environment::Production),
            "sandbox" => Some(Environment::Sandbox),
            _ => None,
        }
    }

    /// The scope string a sign-in asks for.
    pub fn scope(self) -> String {
        match self {
            Environment::Production => PRODUCTION_SCOPE.to_string(),
            Environment::Sandbox => format!("{PRODUCTION_SCOPE} {SANDBOX_SCOPE}"),
        }
    }
}

/// The health plan as an `org-list/v1` item and a stream header's
/// `organization`.
pub fn organization() -> Organization {
    Organization {
        id: ORG_ID.to_string(),
        name: ORG_NAME.to_string(),
        fhir_base: pk_med_fhir::normalize_base(FHIR_BASE),
        vendor: Some("kaiser".to_string()),
        location: Some("all regions".to_string()),
        aliases: vec![
            "Kaiser Foundation Health Plan".to_string(),
            "Kaiser Permanente".to_string(),
            "KP".to_string(),
        ],
    }
}

/// Whether the organization matches every word of `text`, against its name,
/// aliases and regions, ignoring case.
pub fn matches(text: &str) -> bool {
    let org = organization();
    let mut haystack = vec![org.name.clone(), org.id.clone()];
    haystack.extend(org.aliases.iter().cloned());
    haystack.extend(REGIONS.iter().map(|r| r.to_string()));
    let haystack = haystack.join(" ").to_lowercase();
    let words: Vec<String> = text.split_whitespace().map(str::to_lowercase).collect();
    !words.is_empty() && words.iter().all(|w| haystack.contains(w.as_str()))
}

/// Resolve `--org`: the id, or any text `orgs search` matches.
pub fn resolve_org(value: &str) -> Result<Organization, pk_cli_core::CliError> {
    if value.trim().eq_ignore_ascii_case(ORG_ID) || matches(value) {
        Ok(organization())
    } else {
        Err(pk_cli_core::CliError::NotFound(format!(
            "no organization matches `{}`; kp reaches one: `{ORG_ID}` ({ORG_NAME})",
            value.trim()
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scopes_per_environment() {
        assert_eq!(
            Environment::Production.scope(),
            "patient/*.read launch/patient offline_access ExplanationOfBenefit"
        );
        assert!(Environment::Sandbox.scope().ends_with(" sandbox"));
        assert_eq!(Environment::parse("sandbox"), Some(Environment::Sandbox));
        assert_eq!(Environment::parse("prod"), None);
    }

    #[test]
    fn organization_is_normalized() {
        let org = organization();
        assert_eq!(org.fhir_base, FHIR_BASE);
        assert!(!org.fhir_base.ends_with('/'));
    }

    #[test]
    fn search_and_resolve() {
        assert!(matches("kaiser"));
        assert!(matches("KAISER northwest"));
        assert!(matches("health plan"));
        assert!(!matches("mychart"));
        assert!(!matches("   "));
        assert_eq!(resolve_org("kaiser-health-plan").unwrap().id, ORG_ID);
        assert_eq!(resolve_org("Kaiser").unwrap().id, ORG_ID);
        assert_eq!(resolve_org("nyu").unwrap_err().exit_code(), 4);
    }
}
