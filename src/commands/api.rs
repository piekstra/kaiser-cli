//! `kp api GET <PATH>`: one FHIR read as the signed-in member, the server's
//! answer on stdout.

use pk_cli_core::{block_on, CliError};
use pk_med_fhir::FhirClient;

use super::{sign_in_again, transport, Ctx, BIN};
use crate::cli::ApiArgs;

pub fn run(ctx: &Ctx, args: &ApiArgs) -> Result<(), CliError> {
    if !args.method.eq_ignore_ascii_case("GET") {
        return Err(CliError::Usage(
            "kp reads and never writes: `api` takes GET only".into(),
        ));
    }
    if args.path.trim().is_empty() {
        return Err(CliError::Usage("give a path under the FHIR base".into()));
    }
    let context = ctx.signed_in(&args.who)?;
    let live = ctx.live(&context)?;
    crate::kaiser::require_server(context.environment()?, &live.fhir_base())?;
    let t = transport()?;
    let source = live.clone();
    let fhir = FhirClient::new(&t, &live.fhir_base(), BIN, env!("CARGO_PKG_VERSION"))?
        .bearer_with(move |refused| source.bearer(refused));
    let body = block_on(fhir.get_raw(&args.path)).map_err(|e| sign_in_again(e, &context))?;
    match serde_json::from_str::<serde_json::Value>(&body) {
        Ok(v) if !ctx.json() => println!(
            "{}",
            serde_json::to_string_pretty(&v).unwrap_or(body.clone())
        ),
        _ => println!("{}", body.trim_end()),
    }
    Ok(())
}
