# AGENTS.md — kaiser-cli

`kp` reads a Kaiser Permanente member's own health-plan claims
(Explanation of Benefit) and coverage through Kaiser's Patient Access API.
It conforms to piekstra-cli/1 and medical-common's `health-records/v1`
profile, on the `pk-cli-*` crates from
[cli-common](https://github.com/piekstra/cli-common) and the `pk-med-*`
crates from [medical-common](https://github.com/piekstra/medical-common).

Clinical records at Kaiser come through Epic, so they are `mychart`'s, not
kp's (`docs/api.md`, Route).

## Layout

| File | Owns |
|---|---|
| `src/main.rs` | parsing, dispatch, `info`, exit codes |
| `src/cli.rs` | the clap tree |
| `src/commands/*.rs` | one module per command group; `Ctx` (config, vault) in `mod.rs` |
| `src/kaiser.rs` | the one organization: its FHIR base, regions, scopes per environment, `--org` resolution and search |
| `src/config.rs` | non-secret settings: environment, redirect port, FHIR base override |
| `src/credentials.rs` | the Keychain behind a trait (`Vault`, `MemoryVault` in tests), the app's client id and secret |
| `src/state.rs` | `state.json`: which Keychain items exist, so a logged-out command opens none |
| `src/fsutil.rs` | owner-only files written whole |
| `docs/api.md` | Kaiser's API, quoted, with sources and traps; read before changing auth or export |

## Medical data

Medical records are the most sensitive data a household has.

- The repository carries no personal data: invented test data only, checked
  by `tests/no_personal_data.rs` and gitleaks.
- Develop and dogfood against medical-common's mock server or Kaiser's
  sandbox (its test members are synthetic). A check against real data
  reports counts, statuses and exit codes only, or is left to the owner in
  their own terminal. An agent never reads, prints or summarizes a real
  record, document or member name.
- No MCP surface: kp is not registered in cli-mcp and ships no `mcp`
  command, because a tool call would hand records to the model making it
  (the effort's plan, §8, until the owner decides otherwise).

## Credentials

- Kaiser registers each app as a confidential client. Its client id and
  secret are one Keychain item per environment: service `piekstra.kp`,
  account `client/production` or `client/sandbox`, stored by
  `kp auth set-credential` from a prompt that does not echo, `--stdin` or
  `--from-env`. Kaiser's terms keep both confidential: never printed,
  logged, or put in argv.
- Tests and `make verify` never read the real Keychain:
  `KP_KEYCHAIN_SERVICE` names an empty service for the surface tests and
  `smoke`; the in-process tests use `MemoryVault`.
- If a `kp` call hangs, it waits on a Keychain approval. Stop calling it and
  tell the owner; a retry only raises another dialog.

## Conventions

- `--json` on every command, one schema-tagged document on stdout.
- Arguments, and secrets from `--stdin` or `--from-env`, are checked before
  the Keychain or the network.
- Read-only: kp never writes to Kaiser.
- Every binary-producing `make` target signs with the stable
  `pk-cli-codesign` identity.

## Tests

`make verify` (fmt, clippy, tests, smoke), offline:

- `tests/cli_surface.rs`: the binary's help, exit codes, JSON errors,
  `info`, completions, `orgs`, `config`, logged-out `auth status`.
- `tests/no_personal_data.rs`: personal-data shapes in tracked files.
- Unit tests in each module.

## Definition of done

`make verify` and CI green, the installed binary dogfooded against the mock
and Kaiser's sandbox, `docs/api.md` matching what Kaiser does, and no
personal data anywhere in the diff.
