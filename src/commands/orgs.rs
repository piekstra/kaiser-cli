//! `kp orgs list|search`: the one organization kp reaches, as org-list/v1.

use pk_cli_core::{output, CliError};
use pk_med_records::{org_list, Organization};

use super::Ctx;
use crate::cli::OrgsCmd;
use crate::kaiser;

pub fn run(ctx: &Ctx, cmd: &OrgsCmd) -> Result<(), CliError> {
    let items = match cmd {
        OrgsCmd::List => vec![kaiser::organization()],
        OrgsCmd::Search { text } => {
            if text.trim().is_empty() {
                return Err(CliError::Usage("give some text to search for".into()));
            }
            if kaiser::matches(text) {
                vec![kaiser::organization()]
            } else {
                Vec::new()
            }
        }
    };
    emit(ctx, items);
    Ok(())
}

fn emit(ctx: &Ctx, items: Vec<Organization>) {
    let list = serde_json::to_value(org_list(items)).expect("org-list/v1 serializes");
    if ctx.json() {
        output::json(&list);
    } else {
        let rows = output::rows_of(&list, "items");
        if rows.is_empty() {
            ctx.note("no organization matches; kp reaches one: `kp orgs list`");
        }
        output::table(&output::table_view(
            &rows,
            &["id", "name", "location", "fhir_base"],
        ));
    }
}
