//! `kp config path|show|set|unset`.

use pk_cli_core::{output, CliError};

use super::Ctx;
use crate::cli::ConfigCmd;
use crate::config::Config;

pub fn run(ctx: &Ctx, cmd: &ConfigCmd) -> Result<(), CliError> {
    match cmd {
        ConfigCmd::Path => {
            let path = ctx.store.path()?.display().to_string();
            output::emit(
                ctx.json(),
                "config-path",
                serde_json::json!({ "path": path }),
                |p| println!("{}", p["path"].as_str().unwrap_or_default()),
            );
            Ok(())
        }
        ConfigCmd::Show => {
            let cfg: Config = ctx.config()?;
            let effective = serde_json::json!({
                "environment": cfg.environment().as_str(),
                "redirect_uri": cfg.redirect_uri(),
                "fhir_base": cfg.fhir_base(),
            });
            output::emit(ctx.json(), "config", effective, output::render);
            Ok(())
        }
        ConfigCmd::Set { key, value } => {
            let mut cfg: Config = ctx.config()?;
            cfg.set(key, value)?;
            ctx.store.save(&cfg)?;
            ctx.note(format!("{key} set"));
            Ok(())
        }
        ConfigCmd::Unset { key } => {
            let mut cfg: Config = ctx.config()?;
            cfg.unset(key)?;
            ctx.store.save(&cfg)?;
            ctx.note(format!("{key} unset"));
            Ok(())
        }
    }
}
