# CLAUDE.md

`AGENTS.md` is canonical; read it first. The rules that matter most:

- Never read, print or summarize a real record, document or member name.
  Develop and dogfood on the mock server or Kaiser's sandbox (synthetic
  members) only.
- Never print or log a token, client id or secret, and never pass one in
  argv.
- If a `kp` call hangs, it waits on a Keychain approval: stop calling it and
  tell the owner.
- No MCP surface: no `mcp` command and no cli-mcp registration.
