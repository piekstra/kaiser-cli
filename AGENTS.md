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
| `src/commands/*.rs` | one module per command group; `Ctx` (config, vault, refresh lock, browser) in `mod.rs` |
| `src/kaiser.rs` | the two organizations (the health plan and its sandbox): FHIR base, regions, scopes, `--org` resolution and search, `KaiserQuirks` |
| `src/session.rs` | a signed-in member (`Context`), pk-med-smart's sessions in the Keychain (`Sessions`), the live token with renewal (`Live`) |
| `src/export.rs` | what `records export` pulls, in order, and which failures skip rather than stop |
| `src/config.rs` | non-secret settings: redirect port, FHIR base override |
| `src/credentials.rs` | the Keychain behind a trait (`Vault`, `MemoryVault` in tests), the app's client id and secret |
| `src/state.rs` | `state.json`: which Keychain items exist and who is signed in, so a logged-out command opens none |
| `src/fsutil.rs` | owner-only files written whole |
| `examples/mock_kaiser.rs` | `make mock`: medical-common's mock SMART + FHIR server, for dogfooding |
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
- A member's sign-in is pk-med-smart's session, one Keychain item per
  member: account `smart/<org>/<FHIR patient id>`. Errors never name the
  account, since it holds the member's id. Renewal runs under the family's
  refresh lock, so two `kp` runs never spend one refresh token twice.
- Tests and `make verify` never read the real Keychain:
  `KP_KEYCHAIN_SERVICE` names an empty service for the surface tests and
  `smoke`; the in-process tests use `MemoryVault`.
- If a `kp` call hangs, it waits on a Keychain approval. Stop calling it and
  tell the owner; a retry only raises another dialog.

## Conventions

- `--json` on every command, one schema-tagged document on stdout, except
  `records export --ndjson`, whose stream is the output.
- Arguments, and secrets from `--stdin` or `--from-env`, are checked before
  the Keychain or the network.
- Read-only: kp never writes to Kaiser; `api` takes GET only.
- `records export -o` writes only the sandbox's records to a file, and
  refuses before the Keychain otherwise: a real member's records go to
  stdout, straight into `medrec import`.
- A server on this machine (the mock) serves synthetic members, so it is
  reached only as `kaiser-health-plan-sandbox`: `auth login` and
  `records export` refuse the production organization there
  (`kaiser::require_real_server`, which parses the host). medrec files
  records by organization id, and a synthetic member must never carry the
  production one.
- Every binary-producing `make` target signs with the stable
  `pk-cli-codesign` identity.

## Tests

`make verify` (fmt, clippy, tests, smoke), offline:

- `tests/cli_surface.rs`: the binary's help, exit codes, JSON errors,
  `info`, completions, `orgs`, `config`, logged-out `auth status`.
- `tests/e2e.rs`: in process against medical-common's mock server on
  127.0.0.1, with a stand-in browser and `MemoryVault`: sign-in (discovery
  from the CapabilityStatement, HTTP Basic at the token endpoint, the
  scopes), export counts, `--since`, a refused token renewed once, refused
  and failing searches as skips, a refused renewal (exit 3 with kp's
  remedy, `refused_at` recorded, the last complete export kept), a
  declined consent, `-o` refused for a production member, two members and
  logout, `set-credential`.
- `tests/no_personal_data.rs`: personal-data shapes in tracked files.
- Unit tests in each module.

## Definition of done

`make verify` and CI green, the installed binary dogfooded against the mock
and Kaiser's sandbox, `docs/api.md` matching what Kaiser does, and no
personal data anywhere in the diff.
