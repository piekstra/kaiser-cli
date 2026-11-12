//! What `records export` pulls from Kaiser's Patient Access API, in order:
//!
//! 1. the member's `Patient` record;
//! 2. a search by `patient` of each type Kaiser's CapabilityStatement serves
//!    for a member ([`SEARCHES`]), bounded under `--since` by `_lastUpdated`,
//!    which the CapabilityStatement lists for every type and Kaiser's guide
//!    documents for `ExplanationOfBenefit`;
//! 3. a read of each resource those name by reference ([`REFERENCED`]):
//!    Kaiser's guide serves Coverage, Location, Organization, Patient and
//!    Practitioner by the ids an EOB references.
//!
//! A refused or failed search or read is a [`Skip`]; the export stops only
//! when the sign-in, the network or the output fails.

use std::collections::BTreeSet;
use std::io::Write;

use pk_cli_core::transport::Transport;
use pk_cli_core::{block_on, CliError, Reason};
use pk_med_fhir::{FhirClient, RawResource};
use pk_med_records::stream::skip;
use pk_med_records::{RecordsWriter, Skip};
use serde_json::Value;

/// The searches by `patient`, in pull order.
pub const SEARCHES: &[&str] = &[
    "ExplanationOfBenefit",
    "Coverage",
    "Encounter",
    "MedicationDispense",
];

/// The types a reference is read as, sorted.
pub const REFERENCED: &[&str] = &[
    "Coverage",
    "Location",
    "Organization",
    "Patient",
    "Practitioner",
];

/// The types a `--since` export reads whole rather than bounded: the
/// member's Patient record, and what the bounded resources reference.
pub fn since_unbounded() -> Vec<String> {
    REFERENCED.iter().map(|t| t.to_string()).collect()
}

/// Whether a failure ends the export rather than skipping one item: the
/// sign-in refused or gone, or the server unreachable.
fn stops(err: &CliError) -> bool {
    matches!(
        err.reason(),
        Some(Reason::CredentialRefused) | Some(Reason::NoCredential)
    ) || err.reason() == Some(pk_med_fhir::UNREACHABLE)
        || (err.exit_code() == 3 && err.reason() != Some(Reason::Forbidden))
}

fn skip_code(err: &CliError) -> &'static str {
    match err.exit_code() {
        3 => skip::FORBIDDEN,
        4 => skip::NOT_FOUND,
        _ => skip::UPSTREAM,
    }
}

/// Pull everything for `patient` into `w`.
pub fn run<T: Transport, W: Write>(
    fhir: &FhirClient<T>,
    w: &mut RecordsWriter<W>,
    patient: &str,
    since: Option<&str>,
    note: &dyn Fn(String),
) -> Result<(), CliError> {
    let base = fhir.base().to_string();
    let mut wanted = BTreeSet::new();

    match block_on(fhir.read("Patient", patient)) {
        Ok(r) => {
            w.resource(&r)?;
        }
        Err(e) if stops(&e) => return Err(e),
        Err(e) => w.skip(Skip::resource("Patient", "", skip_code(&e), e.message())),
    }

    for &resource_type in SEARCHES {
        let mut params = vec![("patient", patient.to_string())];
        let mut label = resource_type.to_string();
        if let Some(day) = since {
            params.push(("_lastUpdated", format!("ge{day}")));
            label = format!("{resource_type}?_lastUpdated=ge{day}");
        }
        match block_on(fhir.search(resource_type, &params)) {
            Ok(found) => {
                note(format!(
                    "{resource_type}: {} over {} page(s)",
                    found.resources.len(),
                    found.pages
                ));
                for r in found.resources.iter().chain(&found.included) {
                    w.resource(r)?;
                    references(r, &base, &mut wanted);
                }
                if found.truncated {
                    w.skip(Skip::search(
                        label,
                        skip::PAGE_CAP,
                        format!("stopped after {} pages", found.pages),
                    ));
                }
            }
            Err(e) if stops(&e) => return Err(e),
            Err(e) => {
                let code = if e.exit_code() == 4 {
                    skip::UNSUPPORTED
                } else {
                    skip_code(&e)
                };
                w.skip(Skip::search(label, code, e.message()));
            }
        }
    }

    for (resource_type, id) in wanted {
        if w.written(&resource_type, &id) {
            continue;
        }
        match block_on(fhir.read(&resource_type, &id)) {
            Ok(r) => {
                w.resource(&r)?;
            }
            Err(e) if stops(&e) => return Err(e),
            Err(e) => w.skip(Skip::resource(
                resource_type.as_str(),
                id.as_str(),
                skip_code(&e),
                e.message(),
            )),
        }
    }
    Ok(())
}

/// Every `reference` in `r` to a [`REFERENCED`] type on this server, as
/// `(type, id)`: relative (`Organization/o1`), or absolute under `base`.
/// Contained references (`#…`) and other servers' are left alone.
pub fn references(r: &RawResource, base: &str, out: &mut BTreeSet<(String, String)>) {
    let Ok(value) = r.value() else { return };
    let mut stack = vec![&value];
    while let Some(v) = stack.pop() {
        match v {
            Value::Object(map) => {
                for (key, child) in map {
                    if key == "reference" {
                        if let Some(pair) = child.as_str().and_then(|s| parse_reference(s, base)) {
                            out.insert(pair);
                        }
                    } else {
                        stack.push(child);
                    }
                }
            }
            Value::Array(items) => stack.extend(items),
            _ => {}
        }
    }
}

fn parse_reference(reference: &str, base: &str) -> Option<(String, String)> {
    let relative = if reference.contains("://") {
        reference.strip_prefix(base)?.strip_prefix('/')?
    } else {
        reference
    };
    let mut parts = relative.split('/');
    let (resource_type, id) = (parts.next()?, parts.next()?);
    match parts.next() {
        None | Some("_history") => {}
        Some(_) => return None,
    }
    let id_ok = !id.is_empty()
        && id.len() <= 64
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.');
    (REFERENCED.contains(&resource_type) && id_ok)
        .then(|| (resource_type.to_string(), id.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: &str = "https://fhir.example.org/r4";

    #[test]
    fn references_are_relative_or_on_this_base() {
        let r = RawResource::parse(
            r##"{"resourceType":"ExplanationOfBenefit","id":"eob-0001",
            "patient":{"reference":"Patient/pat-0001"},
            "insurer":{"reference":"#payer"},
            "provider":{"reference":"https://fhir.example.org/r4/Practitioner/prac-0001"},
            "facility":{"reference":"https://elsewhere.example.org/r4/Location/loc-9"},
            "careTeam":[{"provider":{"reference":"Organization/org-0001/_history/2"}}],
            "insurance":[{"coverage":{"reference":"Coverage/cov-0001"}}],
            "item":[{"locationReference":{"reference":"Location/loc-0001"}},
                    {"encounter":[{"reference":"Encounter/enc-0001"}]},
                    {"x":{"reference":"Organization/bad id"}}]}"##,
        )
        .unwrap();
        let mut out = BTreeSet::new();
        references(&r, BASE, &mut out);
        let got: Vec<String> = out.iter().map(|(t, i)| format!("{t}/{i}")).collect();
        assert_eq!(
            got,
            [
                "Coverage/cov-0001",
                "Location/loc-0001",
                "Organization/org-0001",
                "Patient/pat-0001",
                "Practitioner/prac-0001",
            ]
        );
    }

    #[test]
    fn only_a_refused_sign_in_or_an_unreachable_server_stops() {
        let refused = CliError::Auth("x".into()).because(Reason::CredentialRefused);
        let forbidden = CliError::Auth("x".into()).because(Reason::Forbidden);
        let unreachable = CliError::Upstream("x".into()).because(pk_med_fhir::UNREACHABLE);
        assert!(stops(&refused));
        assert!(stops(&unreachable));
        assert!(!stops(&forbidden));
        assert!(!stops(&CliError::NotFound("x".into())));
        assert!(!stops(&CliError::Upstream("x".into())));
        assert_eq!(skip_code(&forbidden), skip::FORBIDDEN);
    }
}
