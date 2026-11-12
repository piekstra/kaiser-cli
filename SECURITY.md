# Security

## Reporting a vulnerability in kp

Report it privately through GitHub's private vulnerability reporting
(Security, Report a vulnerability) on
https://github.com/piekstra/kaiser-cli. Do not open a public issue, and do
not include a real record, member number, token or secret in the report.

## A flaw in Kaiser's API

Kaiser's developer terms require a security flaw found in Kaiser's APIs to
be reported to Kaiser (the Kaiser Permanente National Compliance Hotline)
and not disclosed publicly. Report it there, not here. The terms also
forbid security research or testing against Kaiser's APIs without Kaiser's
written approval; kp does neither.

## What kp guarantees

- Credentials (the app's client id and secret, the member's tokens) live
  only in the OS credential store under the service `piekstra.kp`. They are
  never printed, logged, written to a file, or taken from the command line.
- A token is sent only to Kaiser's API origin; a URL from outside the
  binary must be `https` (or `http` to this machine for a test server)
  before a credential travels with it.
- kp reads; it never writes to Kaiser.
- The test suite and `make verify` never read the real Keychain
  (`KP_KEYCHAIN_SERVICE` points them at an empty service).
