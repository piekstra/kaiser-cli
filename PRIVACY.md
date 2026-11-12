# Privacy

`kp` is an open-source command-line program a Kaiser Permanente member runs
on their own computer to download their own health-plan records. This page
is its privacy policy, and the rule its repository is kept by.

## What kp reads

With the member's consent, given on Kaiser's own sign-in and consent pages,
kp reads through Kaiser's Patient Access API (the CMS interoperability API
Kaiser publishes at developer.kp.org):

- claims, as Explanation of Benefit resources;
- coverage;
- the patient, provider, organization and location records those claims
  name.

It reads the records of the member who signed in, or of a family member the
member is allowed to act for on kp.org. It reads nothing else, and it never
changes anything at Kaiser.

## Where the data goes

Only to the member's own computer.

- kp sends no record, and nothing about one, to its authors or to any
  other party. It has no server, no analytics, no telemetry, no crash
  reporting and no advertising.
- What kp downloads is written to the member's standard output or to a file
  the member names, normally piped into the member's own encrypted records
  store.
- The member's sign-in tokens and the app's Kaiser client id and secret are
  kept in the operating system's credential store (the macOS Keychain),
  readable only by the member's account. They are never printed, logged or
  written to a file.
- kp keeps a small settings file and a list of which sign-ins it holds,
  readable only by the member, under `~/.config/kp`. Neither holds a record.

kp connects to Kaiser's API servers, and to GitHub only when the member
asks it to update itself.

## Stopping access

`kp auth logout` removes the sign-ins kp holds; `kp auth logout --forget`
also removes the app's client credentials and its settings. To revoke kp's
access at Kaiser, the member removes the app on kp.org under Profile and
Preferences, Data sharing & authorization, Manage access to health plan
information.

## Contact

Questions about this policy: open an issue at
https://github.com/piekstra/kaiser-cli/issues. Never put a record, a
member number or any personal detail in an issue.

## The repository carries no personal data

Everything committed here is public. It holds:

- no record, response, document or export from a real member, under any
  extension (`.gitignore` blocks `*.ndjson`, `*.har` and capture
  directories);
- no name, member number, claim number, address, phone number, email
  address, dollar amount or date belonging to a real person;
- no client id, client secret, token or cookie.

Test data is invented, with obviously fake values. `tests/no_personal_data.rs`
scans every tracked file for the shapes personal data takes, and CI runs
gitleaks on every push and pull request.
