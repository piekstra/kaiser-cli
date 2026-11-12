# Contributing

- Read `AGENTS.md` (layout, rules for medical data and credentials) and
  `docs/api.md` (Kaiser's API, its sources and its traps) first.
- `make verify` runs what CI runs: fmt, clippy with `-D warnings`, the tests
  and the smoke test on the release binary. It needs no network beyond the
  first dependency fetch and never reads the real Keychain.
- Test data is invented. Never commit a real response, record or export;
  `tests/no_personal_data.rs` and gitleaks check every change.
- Every change lands through a pull request with a review and green checks.
- Dates on the command line are `YYYY-MM-DD`; every command takes `--json`.
