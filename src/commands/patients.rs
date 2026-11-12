//! `kp patients list`: the members signed in, as patient-list/v1.

use pk_cli_core::{output, CliError};
use pk_med_records::patient_list;
use pk_med_smart::SessionStore;
use serde_json::Value;

use super::Ctx;
use crate::cli::PatientsCmd;
use crate::kaiser;
use crate::session::{now, Sessions};

pub fn run(ctx: &Ctx, cmd: &PatientsCmd) -> Result<(), CliError> {
    let PatientsCmd::List { status, org } = cmd;
    let list = list(ctx, *status, org.as_deref())?;
    if ctx.json() {
        output::json(&list);
        return Ok(());
    }
    let rows = output::rows_of(&list, "items");
    if rows.is_empty() {
        println!("No member is signed in (`kp auth login --label <WHO>`).");
        return Ok(());
    }
    let mut columns = vec!["label", "id", "organization", "relationship"];
    if *status {
        columns.extend(["usable", "refused", "error"]);
    }
    output::table(&output::table_view(&rows, &columns));
    Ok(())
}

/// `patient-list/v1` for the members `org` selects. With `status`, each row
/// also says whether its sign-in is usable and whether Kaiser refused it; a
/// sign-in that cannot be read is that row's `error`, not the command's.
pub fn list(ctx: &Ctx, status: bool, org: Option<&str>) -> Result<Value, CliError> {
    let org = org.map(kaiser::resolve_org).transpose()?;
    let contexts: Vec<_> = ctx
        .contexts()?
        .into_iter()
        .filter(|c| org.is_none_or(|e| c.organization == e.org_id()))
        .collect();
    let sessions = Sessions(ctx.vault.clone());
    let mut list = serde_json::to_value(patient_list(
        contexts.iter().map(|c| c.patient_context()).collect(),
    ))
    .expect("patient-list/v1 serializes");
    if !status {
        return Ok(list);
    }
    if let Some(rows) = list["items"].as_array_mut() {
        for (row, c) in rows.iter_mut().zip(&contexts) {
            match sessions.load(&c.account()) {
                Ok(session) => {
                    // A token Kaiser refused is never handed out again; the
                    // next use renews it, so the sign-in is usable only if
                    // it can.
                    let usable = session.as_ref().is_some_and(|s| {
                        (s.refused_at.is_none() && !s.tokens.is_expired(now(), 0)) || s.renewable()
                    });
                    row["usable"] = usable.into();
                    row["refused"] = session
                        .as_ref()
                        .is_some_and(|s| s.refused_at.is_some())
                        .into();
                }
                Err(e) => {
                    row["usable"] = false.into();
                    row["error"] = e.message().into();
                }
            }
        }
    }
    Ok(list)
}
