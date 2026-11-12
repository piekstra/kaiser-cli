# Changelog

## Unreleased

- `orgs list` and `orgs search`: Kaiser Permanente's health plan and its
  sandbox (`kaiser-health-plan`, `kaiser-health-plan-sandbox`) as
  `org-list/v1`.
- `auth set-credential`: the app's Kaiser client id and secret, per
  environment, in the Keychain, from a prompt that does not echo, `--stdin`
  or `--from-env`; refuses to replace a stored one without `--overwrite`.
- `auth login`: SMART App Launch in the system browser (pk-med-smart):
  endpoints from the CapabilityStatement, PKCE S256, the client secret as
  HTTP Basic, one sign-in per member with an optional `--label`.
- `auth status` (`auth-status/v1`, with clients and members) and
  `auth logout [--patient] [--org] [--forget]`.
- `patients list [--org]` (`patient-list/v1`; `--status` adds whether
  each sign-in is usable and whether Kaiser refused it).
- `records export --ndjson | -o <DIR> [--since]`: `health-records/v1`'s
  `records-stream/v1`, for `medrec import --from kp`. A token Kaiser
  refuses is renewed once; refused or failing searches are skips. `-o`
  replaces `records.ndjson` only with a complete stream (an incomplete one
  goes to `records.incomplete.ndjson`), and only for the sandbox.
- A production sign-in goes to Kaiser's server only (https on `kp.org`), a
  sandbox one to Kaiser's or to a server on this machine; `fhir_base` takes
  nothing else.
- A config that still has the retired `environment` key is refused, naming
  `--org kaiser-health-plan-sandbox`, until `kp config unset environment`.
- On medical-common v0.2.0 and cli-common v0.31.1.
- `api GET <PATH>`: one FHIR read as the member.
- `config`: `redirect_port`, `fhir_base` (Kaiser's, or a test server on
  this machine).
- `self-update`, `completions`, `info`.
- `docs/api.md`: how Kaiser's records are reached, clinical (Epic) and
  claims (the Patient Access API), with sources, and what kp pulls.
