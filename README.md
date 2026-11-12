# kp — Kaiser Permanente health-plan claims

`kp` downloads your own Kaiser Permanente health-plan claims (Explanation of
Benefit) and coverage through Kaiser's Patient Access API, the CMS
interoperability API Kaiser publishes at
[developer.kp.org](https://developer.kp.org). It runs on your computer and
sends nothing anywhere else ([PRIVACY.md](PRIVACY.md)).

Unofficial; not affiliated with or endorsed by Kaiser Permanente.

Clinical records (visits, results, notes) at Kaiser come through Epic:
use [`mychart`](https://github.com/piekstra/mychart-cli) for those.
[docs/api.md](docs/api.md) explains the split.

## Status

Not released yet. This version answers `orgs`, `auth` and `config` and
stores your app's Kaiser client registration; the browser sign-in and
`records export` (medical-common's `health-records/v1` profile, read by
`medrec import --from kp`) follow before v0.1.0.

## Setup

Kaiser's API needs a registered app, and its developer terms keep the app's
client id and secret confidential, so each household registers its own:

1. Create an account at [developer.kp.org](https://developer.kp.org), add
   an application, and request the "Patient Access API - Explanation of
   Benefits and Associated Resources". Kaiser reviews the request by email.
2. Set the app's redirect URI to `http://127.0.0.1:7469/callback`, for the
   sandbox and for production.
3. Generate the client id and secret, then store them in the Keychain:

```sh
kp auth set-credential              # production; asks for each without echoing it
kp auth set-credential --sandbox    # the sandbox, with Kaiser's synthetic test members
op read "op://<vault>/<item>/pair" | kp auth set-credential --stdin   # `<id>:<secret>`
```

## Commands

| Command | Does |
|---|---|
| `kp orgs list`, `kp orgs search <TEXT>` | the one organization kp reaches (`org-list/v1`) |
| `kp auth set-credential [--sandbox] [--overwrite]` | store the app's client id and secret |
| `kp auth status` | what is stored (`auth-status/v1`) |
| `kp auth logout [--forget]` | remove sign-ins; `--forget` also the client credentials and settings |
| `kp config path\|show\|set\|unset` | `environment` (`production` or `sandbox`), `redirect_port`, `fhir_base` |
| `kp self-update`, `kp completions <SHELL>`, `kp info` | the family's standard commands |

Every command takes `--json`. Exit codes: 0 ok, 2 usage, 3 auth, 4 not
found, 5 upstream, 6 confirmation required.

## Build

```sh
make install    # cargo install --locked, then signs the binary (macOS)
make verify     # what CI runs
```

Licensed under MIT or Apache-2.0.
