//! The command tree.

use clap::{Args, Parser, Subcommand};
use clap_complete::Shell;
use pk_cli_core::CommonArgs;
use pk_cli_secrets::SecretSourceArgs;
use pk_cli_selfupdate::SelfUpdateArgs;

/// Pull your own Kaiser Permanente health-plan claims (Explanation of
/// Benefit) and coverage through Kaiser's Patient Access API. Unofficial; not
/// affiliated with or endorsed by Kaiser Permanente.
#[derive(Parser, Debug)]
#[command(name = "kp", version, about, long_about = None)]
pub struct Cli {
    #[command(flatten)]
    pub common: CommonArgs,

    /// Config file location (default ~/.config/kp/config.json).
    #[arg(long, global = true, value_name = "PATH", env = "KP_CONFIG")]
    pub config: Option<std::path::PathBuf>,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// The organization kp reaches: Kaiser Permanente's health plan
    /// (org-list/v1).
    #[command(subcommand)]
    Orgs(OrgsCmd),
    /// The app's client registration with Kaiser, and the stored sign-ins.
    #[command(subcommand)]
    Auth(AuthCmd),
    /// Non-secret settings: the environment, the redirect port.
    #[command(subcommand)]
    Config(ConfigCmd),
    /// Update to the latest release from GitHub.
    SelfUpdate(SelfUpdateArgs),
    /// Print a shell completion script.
    Completions { shell: Shell },
    /// Machine-readable capability discovery (cli-info/v1).
    Info,
}

#[derive(Subcommand, Debug)]
pub enum OrgsCmd {
    /// Every organization kp reaches (one: the health plan, all regions).
    #[command(visible_alias = "ls")]
    List,
    /// The organizations whose name, other names or regions contain every
    /// word of TEXT.
    Search { text: String },
}

#[derive(Subcommand, Debug)]
pub enum AuthCmd {
    /// Store the client id and secret Kaiser's developer portal issued for
    /// this household's app, in the Keychain. On a terminal kp asks for each
    /// without echoing it; with --stdin or --from-env it reads
    /// `<id>:<secret>`, or the id and the secret on two lines. Never on the
    /// command line.
    SetCredential(SetCredentialArgs),
    /// What is stored and whether it can be used (auth-status/v1).
    Status,
    /// Remove stored sign-ins; --forget also removes the client credentials,
    /// the config and the state file.
    Logout(LogoutArgs),
}

#[derive(Args, Debug)]
pub struct SetCredentialArgs {
    #[command(flatten)]
    pub source: SecretSourceArgs,
    /// The sandbox registration (Kaiser's synthetic test members) instead of
    /// production.
    #[arg(long)]
    pub sandbox: bool,
    /// Replace a client already stored for this environment.
    #[arg(long)]
    pub overwrite: bool,
}

#[derive(Args, Debug)]
pub struct LogoutArgs {
    /// Also remove the client id and secret of both environments, the config
    /// and the state file.
    #[arg(long)]
    pub forget: bool,
}

#[derive(Subcommand, Debug)]
pub enum ConfigCmd {
    /// Print the config file path.
    Path,
    /// Show the effective configuration.
    Show,
    /// Set a key: environment (production or sandbox), redirect_port,
    /// fhir_base.
    Set { key: String, value: String },
    /// Remove a key.
    Unset { key: String },
}
