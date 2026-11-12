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
    /// The organizations kp reaches: Kaiser Permanente's health plan, and its
    /// sandbox (org-list/v1).
    #[command(subcommand)]
    Orgs(OrgsCmd),
    /// The app's client registration with Kaiser, signing in as a member,
    /// and the stored sign-ins.
    #[command(subcommand)]
    Auth(AuthCmd),
    /// The members signed in (patient-list/v1).
    #[command(subcommand)]
    Patients(PatientsCmd),
    /// A member's claims and coverage as a records-stream/v1 NDJSON stream.
    #[command(subcommand)]
    Records(RecordsCmd),
    /// One FHIR read as the signed-in member: `kp api GET
    /// ExplanationOfBenefit?patient=<id>`.
    Api(ApiArgs),
    /// Non-secret settings: the redirect port, a test server's FHIR base.
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
    /// Every organization kp reaches.
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
    /// Sign in as a member in the system browser (kp.org's own sign-in and
    /// consent pages, PKCE, a redirect to this machine). One sign-in per
    /// member; a member who acts for family members on kp.org signs in once
    /// for each, picking them on Kaiser's page.
    Login(LoginArgs),
    /// What is stored and whether it can be used (auth-status/v1).
    Status,
    /// Remove stored sign-ins (all, or those --org and --patient select);
    /// --forget also removes the client credentials, the config and the
    /// state file.
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
pub struct LoginArgs {
    /// The organization, an id from `kp orgs list`: `kaiser-health-plan`
    /// (the default), or `kaiser-health-plan-sandbox` (`sandbox` for short)
    /// for Kaiser's synthetic test members.
    #[arg(long, value_name = "ORG", default_value = "kaiser-health-plan")]
    pub org: String,
    /// A name for this member in this household (`adult1`, `child1`), to
    /// pick them with --patient later.
    #[arg(long, value_name = "WHO")]
    pub label: Option<String>,
    /// Print the sign-in URL instead of opening a browser.
    #[arg(long)]
    pub no_browser: bool,
    /// How long to wait for the sign-in to finish, in minutes.
    #[arg(long, value_name = "MIN", default_value_t = 10)]
    pub timeout: u64,
}

#[derive(Args, Debug)]
pub struct LogoutArgs {
    #[command(flatten)]
    pub who: PatientArgs,
    /// Also remove the client id and secret of both environments, the config
    /// and the state file.
    #[arg(long)]
    pub forget: bool,
}

#[derive(Subcommand, Debug)]
pub enum PatientsCmd {
    /// The members signed in, and how they were reached (self or proxy).
    #[command(visible_alias = "ls")]
    List {
        /// Also read each stored sign-in to report whether it is usable
        /// (reads the Keychain).
        #[arg(long)]
        status: bool,
        /// Only members at this organization: `kaiser-health-plan` or
        /// `kaiser-health-plan-sandbox` (`sandbox` for short).
        #[arg(long, value_name = "ORG")]
        org: Option<String>,
    },
}

/// Which signed-in member a command reads.
#[derive(Args, Debug, Clone, Default)]
pub struct PatientArgs {
    /// The member: a label from `auth login --label` or the FHIR id from
    /// `patients list`. Needed when more than one member is signed in.
    #[arg(long, value_name = "ID")]
    pub patient: Option<String>,
    /// Only members at this organization: `kaiser-health-plan` or
    /// `kaiser-health-plan-sandbox` (`sandbox` for short).
    #[arg(long, value_name = "ORG")]
    pub org: Option<String>,
}

#[derive(Subcommand, Debug)]
pub enum RecordsCmd {
    /// Every claim, coverage, encounter and dispense Kaiser serves for the
    /// member, with the patient, providers, organizations and locations they
    /// name, as records-stream/v1 (what `medrec import --from kp` reads).
    Export(ExportArgs),
}

#[derive(Args, Debug)]
pub struct ExportArgs {
    #[command(flatten)]
    pub who: PatientArgs,
    /// Only resources Kaiser changed on or after this day (YYYY-MM-DD); the
    /// member's own Patient record comes whole.
    #[arg(long, value_name = "YYYY-MM-DD")]
    pub since: Option<String>,
    /// Write the stream to stdout.
    #[arg(long, conflicts_with = "output")]
    pub ndjson: bool,
    /// Write the stream to DIR/records.ndjson instead (the sandbox and test
    /// servers only: real records go straight into medrec).
    #[arg(short, long, value_name = "DIR")]
    pub output: Option<std::path::PathBuf>,
}

#[derive(Args, Debug)]
pub struct ApiArgs {
    /// GET only: kp reads and never writes.
    pub method: String,
    /// A path under the FHIR base (`ExplanationOfBenefit?patient=<id>`,
    /// `Coverage/<id>`), or a URL on that server.
    pub path: String,
    #[command(flatten)]
    pub who: PatientArgs,
}

#[derive(Subcommand, Debug)]
pub enum ConfigCmd {
    /// Print the config file path.
    Path,
    /// Show the effective configuration.
    Show,
    /// Set a key: redirect_port, fhir_base.
    Set { key: String, value: String },
    /// Remove a key.
    Unset { key: String },
}
