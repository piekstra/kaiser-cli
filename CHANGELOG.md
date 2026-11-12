# Changelog

## Unreleased

- `orgs list` and `orgs search`: Kaiser Permanente's health plan as
  `org-list/v1`.
- `auth set-credential`: the app's Kaiser client id and secret, per
  environment, in the Keychain, from a prompt that does not echo, `--stdin`
  or `--from-env`.
- `auth status` (`auth-status/v1`) and `auth logout [--forget]`.
- `config`: `environment`, `redirect_port`, `fhir_base`.
- `self-update`, `completions`, `info`.
- `docs/api.md`: how Kaiser's records are reached, clinical (Epic) and
  claims (the Patient Access API), with sources.
