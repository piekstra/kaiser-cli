//! `kp orgs list|search`: the organizations kp reaches, as org-list/v1.

use pk_cli_core::{output, CliError};
use pk_med_records::{org_list, Organization};

use super::Ctx;
use crate::cli::OrgsCmd;
use crate::kaiser::{self, Environment};

pub fn run(ctx: &Ctx, cmd: &OrgsCmd) -> Result<(), CliError> {
    let base = ctx.config()?.fhir_base();
    let all: Vec<Organization> = Environment::ALL
        .into_iter()
        .map(|e| kaiser::organization(e, &base))
        .collect();
    let items = match cmd {
        OrgsCmd::List => all,
        OrgsCmd::Search { text } => {
            if text.trim().is_empty() {
                return Err(CliError::Usage("give some text to search for".into()));
            }
            all.into_iter()
                .filter(|o| kaiser::matches(o, text))
                .collect()
        }
    };
    let list = serde_json::to_value(org_list(items)).expect("org-list/v1 serializes");
    if ctx.json() {
        output::json(&list);
    } else {
        let rows = output::rows_of(&list, "items");
        if rows.is_empty() {
            ctx.note("no organization matches (`kp orgs list` shows both)");
        }
        output::table(&output::table_view(
            &rows,
            &["id", "name", "location", "fhir_base"],
        ));
    }
    Ok(())
}
