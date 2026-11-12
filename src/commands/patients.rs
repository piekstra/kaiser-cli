//! `kp patients list`: the members signed in, as patient-list/v1.

use pk_cli_core::{output, CliError};
use pk_med_records::patient_list;
use pk_med_smart::SessionStore;

use super::Ctx;
use crate::cli::PatientsCmd;
use crate::session::{now, Sessions};

pub fn run(ctx: &Ctx, cmd: &PatientsCmd) -> Result<(), CliError> {
    let PatientsCmd::List { status } = cmd;
    let contexts = ctx.contexts()?;
    let sessions = Sessions(ctx.vault.clone());
    let mut items = Vec::new();
    let mut checked = Vec::new();
    for c in &contexts {
        items.push(c.patient_context());
        if *status {
            let session = sessions.load(&c.account())?;
            // A token Kaiser refused is never handed out again; the next
            // use renews it, so the sign-in is usable only if it can.
            let usable = session.as_ref().is_some_and(|s| {
                (s.refused_at.is_none() && !s.tokens.is_expired(now(), 0)) || s.renewable()
            });
            let refused = session.as_ref().is_some_and(|s| s.refused_at.is_some());
            checked.push((usable, refused));
        }
    }
    let mut list = serde_json::to_value(patient_list(items)).expect("patient-list/v1 serializes");
    if *status {
        if let Some(rows) = list["items"].as_array_mut() {
            for (row, (usable, refused)) in rows.iter_mut().zip(&checked) {
                row["usable"] = (*usable).into();
                row["refused"] = (*refused).into();
            }
        }
    }
    if ctx.json() {
        output::json(&list);
    } else {
        let rows = output::rows_of(&list, "items");
        if rows.is_empty() {
            println!("No member is signed in (`kp auth login --label <WHO>`).");
            return Ok(());
        }
        let mut columns = vec!["label", "id", "organization", "relationship"];
        if *status {
            columns.extend(["usable", "refused"]);
        }
        output::table(&output::table_view(&rows, &columns));
    }
    Ok(())
}
