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

Not released yet. The sign-in and `records export` are tested against
medical-common's mock server; the first run against Kaiser's sandbox waits
on the app's approval at developer.kp.org.

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

4. Sign in once per member, in the browser, on kp.org's own pages:

```sh
kp auth login --label adult1        # opens kp.org; consent; back to this machine
kp auth login --org sandbox         # one of Kaiser's synthetic test members
```

A member who acts for family members on kp.org signs in once for each,
picking them on Kaiser's page, with a label for each (`--label child1`).

## Use

```sh
kp patients list                                  # who is signed in
kp records export --ndjson --patient adult1 | medrec import --from kp
medrec import --from kp                           # the same, run by medrec
kp records export --ndjson --since 2026-01-01     # only what changed since
kp api GET 'ExplanationOfBenefit?patient=<id>'    # one read, the raw answer
```

`records export` pulls every claim (`ExplanationOfBenefit`), coverage,
encounter and pharmacy dispense Kaiser serves for the member, then the
patient, providers, organizations and locations they name. It writes
medical-common's `records-stream/v1`. A search Kaiser refuses or fails is
listed in the stream's trailer as skipped; the export stops only when the
sign-in, the network or the output fails.

## Commands

| Command | Does |
|---|---|
| `kp orgs list`, `kp orgs search <TEXT>` | the health plan and its sandbox (`org-list/v1`) |
| `kp auth set-credential [--sandbox] [--overwrite]` | store the app's client id and secret |
| `kp auth login [--org sandbox] [--label <WHO>] [--no-browser] [--timeout <MIN>]` | sign a member in; waits up to `--timeout` minutes (default 10, at most 60) |
| `kp auth status` | what is stored (`auth-status/v1`), per environment and member |
| `kp auth logout [--patient <WHO>] [--org <ORG>] [--forget]` | remove sign-ins; `--forget` also the client credentials and settings |
| `kp patients list [--status]` | the signed-in members (`patient-list/v1`) |
| `kp records export --ndjson\|-o <DIR> [--since <DATE>]` | the member's records as `records-stream/v1`; `-o` for the sandbox only |
| `kp api GET <PATH>` | one FHIR read as the member |
| `kp config path\|show\|set\|unset` | `redirect_port` (default 7469), `fhir_base` |
| `kp self-update`, `kp completions <SHELL>`, `kp info` | the family's standard commands |

Every command takes `--json`, except `records export --ndjson`, whose
output is already the NDJSON stream. Exit codes: 0 ok, 2 usage, 3 auth, 4 not
found, 5 upstream, 6 confirmation required.

## Build

```sh
make install    # cargo install --locked, then signs the binary (macOS)
make verify     # what CI runs
```

Licensed under MIT or Apache-2.0.
