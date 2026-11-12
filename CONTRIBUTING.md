# Contributing

- Read `AGENTS.md` (layout, rules for medical data and credentials) and
  `docs/api.md` (Kaiser's API, its sources and its traps) first.
- `make verify` runs what CI runs: fmt, clippy with `-D warnings`, the tests
  and the smoke test on the release binary. It needs no network beyond the
  first dependency fetch and never reads the real Keychain.
- Dogfood the installed binary against `make mock`, medical-common's mock
  SMART + FHIR server serving one synthetic member, before Kaiser's sandbox.
  It prints its FHIR base and the synthetic client pair:

  ```sh
  make install && make mock     # in another terminal
  export KP_KEYCHAIN_SERVICE=piekstra.kp.dogfood
  kp config set fhir_base <the mock's FHIR base>
  printf '%s' '<the printed pair>' | kp auth set-credential --sandbox --stdin
  kp auth login --org sandbox
  kp records export -o /tmp/kp-mock
  kp auth logout --forget
  ```

- Test data is invented. Never commit a real response, record or export;
  `tests/no_personal_data.rs` and gitleaks check every change.
- Every change lands through a pull request with a review and green checks.
- Dates on the command line are `YYYY-MM-DD`; every command takes `--json`.
