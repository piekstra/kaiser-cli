//! Kaiser Permanente's health-plan Patient Access API: the organizations
//! `kp` reaches (production, and the sandbox with Kaiser's synthetic test
//! members), their FHIR base, and the scopes Kaiser requires. `docs/api.md`
//! has the sources.

use pk_cli_core::CliError;
use pk_med_records::Organization;
use pk_med_smart::{ScopeSet, VendorQuirks};

/// `kp`'s id for the health plan: the `--org` value, the stream header's
/// `organization.id` and the Keychain prefix of its sign-ins.
pub const ORG_ID: &str = "kaiser-health-plan";

/// The sandbox's own id, so a store never files a synthetic member's
/// records beside a real member's.
pub const SANDBOX_ORG_ID: &str = "kaiser-health-plan-sandbox";

pub const ORG_NAME: &str = "Kaiser Permanente health plan";

/// The FHIR base of the Patient Access API (Smile CDR behind Apigee), one for
/// every region, for the sandbox too. Kaiser's guide also writes
/// `[base]/HPEOB/...` in its search table; its own example request has no
/// `HPEOB`.
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

/// Which of the app's two client registrations a sign-in uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Environment {
    Production,
    Sandbox,
}

impl Environment {
    pub const ALL: [Environment; 2] = [Environment::Production, Environment::Sandbox];

    pub fn as_str(self) -> &'static str {
        match self {
            Environment::Production => "production",
            Environment::Sandbox => "sandbox",
        }
    }

    /// The scopes a sign-in asks for.
    pub fn scope(self) -> String {
        match self {
            Environment::Production => PRODUCTION_SCOPE.to_string(),
            Environment::Sandbox => format!("{PRODUCTION_SCOPE} {SANDBOX_SCOPE}"),
        }
    }

    pub fn scopes(self) -> ScopeSet {
        ScopeSet::parse(&self.scope())
    }

    pub fn org_id(self) -> &'static str {
        match self {
            Environment::Production => ORG_ID,
            Environment::Sandbox => SANDBOX_ORG_ID,
        }
    }

    /// The environment of an organization id kp issued.
    pub fn of_org(org_id: &str) -> Option<Self> {
        Environment::ALL.into_iter().find(|e| e.org_id() == org_id)
    }
}

/// An organization as `org-list/v1` and a stream header carry it, at
/// `fhir_base` (Kaiser's, or a test server's from the config).
pub fn organization(environment: Environment, fhir_base: &str) -> Organization {
    let (name, location) = match environment {
        Environment::Production => (ORG_NAME.to_string(), "all regions"),
        Environment::Sandbox => (format!("{ORG_NAME} (sandbox)"), "synthetic test members"),
    };
    let mut aliases = vec![
        "Kaiser Foundation Health Plan".to_string(),
        "Kaiser Permanente".to_string(),
        "KP".to_string(),
    ];
    if environment == Environment::Sandbox {
        aliases.push("sandbox".to_string());
    }
    Organization {
        id: environment.org_id().to_string(),
        name,
        fhir_base: pk_med_fhir::normalize_base(fhir_base),
        vendor: Some("kaiser".to_string()),
        location: Some(location.to_string()),
        aliases,
    }
}

/// Whether `org` matches every word of `text`, against its id, name, other
/// names and Kaiser's regions, ignoring case.
pub fn matches(org: &Organization, text: &str) -> bool {
    let mut haystack = vec![org.name.clone(), org.id.clone()];
    haystack.extend(org.aliases.iter().cloned());
    haystack.extend(REGIONS.iter().map(|r| r.to_string()));
    let haystack = haystack.join(" ").to_lowercase();
    let words: Vec<String> = text.split_whitespace().map(str::to_lowercase).collect();
    !words.is_empty() && words.iter().all(|w| haystack.contains(w.as_str()))
}

/// Resolve `--org`: an id, `sandbox`, or text that matches one of the two
/// (text both match, such as `kaiser`, is production).
pub fn resolve_org(value: &str) -> Result<Environment, CliError> {
    let v = value.trim().to_lowercase();
    match v.as_str() {
        ORG_ID | "production" => return Ok(Environment::Production),
        SANDBOX_ORG_ID | "sandbox" => return Ok(Environment::Sandbox),
        _ => {}
    }
    let hits: Vec<Environment> = Environment::ALL
        .into_iter()
        .filter(|e| matches(&organization(*e, FHIR_BASE), value))
        .collect();
    match hits.as_slice() {
        [one] => Ok(*one),
        [_, _] => Ok(Environment::Production),
        _ => Err(CliError::NotFound(format!(
            "no organization matches `{}`; kp reaches `{ORG_ID}` and `{SANDBOX_ORG_ID}`",
            value.trim()
        ))),
    }
}

/// What Kaiser's server does differently from SMART as written: it
/// publishes no `.well-known/smart-configuration` (Apigee answers 400), so
/// discovery asks the CapabilityStatement first.
#[derive(Debug, Clone, Copy, Default)]
pub struct KaiserQuirks;

impl VendorQuirks for KaiserQuirks {
    fn metadata_first(&self) -> bool {
        true
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
        assert!(Environment::Sandbox.scopes().contains("sandbox"));
        assert!(Environment::Production
            .scopes()
            .contains("ExplanationOfBenefit"));
    }

    #[test]
    fn organizations_are_normalized_and_distinct() {
        let p = organization(Environment::Production, &format!("{FHIR_BASE}/"));
        let s = organization(Environment::Sandbox, FHIR_BASE);
        assert_eq!(p.fhir_base, FHIR_BASE);
        assert_ne!(p.id, s.id);
        assert_eq!(Environment::of_org(&s.id), Some(Environment::Sandbox));
        assert_eq!(Environment::of_org("other"), None);
    }

    #[test]
    fn search_and_resolve() {
        let p = organization(Environment::Production, FHIR_BASE);
        assert!(matches(&p, "kaiser"));
        assert!(matches(&p, "KAISER northwest"));
        assert!(!matches(&p, "mychart"));
        assert!(!matches(&p, "   "));
        assert_eq!(
            resolve_org("kaiser-health-plan").unwrap(),
            Environment::Production
        );
        assert_eq!(resolve_org("Kaiser").unwrap(), Environment::Production);
        assert_eq!(resolve_org("health plan").unwrap(), Environment::Production);
        assert_eq!(resolve_org("sandbox").unwrap(), Environment::Sandbox);
        assert_eq!(resolve_org("kaiser sandbox").unwrap(), Environment::Sandbox);
        assert_eq!(resolve_org("nyu").unwrap_err().exit_code(), 4);
    }
}
