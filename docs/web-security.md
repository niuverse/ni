+++
id = "web-security"
title = "Local web threat model"
kind = "doc"
status = "living"
order = 42
summary = "What the loopback web surface protects against, what it does not, and which tests hold the line."
+++

## Scope

`niucode web` (and `niuse web --features web`) is a single-user, localhost tool.
It has no remote mode and no account system. The event log is the source of
truth; the browser is a client.

## Trust model

- Trusted: the local OS user, the local browser, the session log, the workspace.
- Untrusted: model output, tool output, Markdown links/images, other web origins,
  foreign `Host` headers.
- Not trusted: other local processes/users on a shared machine. A per-run random
  token plus HttpOnly cookie gates the local API; it is not remote auth.

## Controls

| Risk | Control | Test / evidence |
| --- | --- | --- |
| Remote network exposure | bind `127.0.0.1` only | server construction |
| Local API access | per-run random token + HttpOnly SameSite cookie | `access_tokens_are_required_and_cookie_bootstrapped` |
| DNS rebinding / foreign Host | loopback `Host` allowlist | `cross_origin_and_foreign_host_requests_are_rejected` |
| Cross-site POST | loopback `Origin` check on POST | same test |
| Framing / clickjacking | `X-Frame-Options: DENY`, `frame-ancestors 'none'` | response headers |
| Script injection | raw HTML escaped; strict CSP | `markdown_html_escapes_raw_html_and_renders_tables` |
| Link-based script execution | parsed href allowlist (`http`/`https`/`mailto`/fragment) | `links_open_safely_and_block_dangerous_schemes` |
| Remote tracking pixels | all `<img>` stripped to alt text | `remote_images_do_not_load` |
| Unapproved writes | `ApprovalMode::Ask`; blocked approver; 300s fail closed | approval/write tests |
| Runaway approvals | per-tool session allow, cleared on new/switch | session approval test |
| Stale cross-tab state | `session_switched` SSE | switch test |
| Event-log mutation from web | routes are read/session-scoped; no log rewrite | code review |

## Residual risks

- The token is a local isolation measure, not OS-level auth: another process
  running as the same user can read the startup URL, browser cookie store, or
  process memory. The guards stop browser origins and unsolicited local callers,
  not a hostile same-user process.
- The server trusts the workspace/path passed on the command line; it does not
  sandbox its own process.
- Browser extensions, devtools, or a compromised local browser bypass the
  presentation assumptions.
- Approval previews and tool output are displayed as text/cards, but a future
  renderer could reintroduce HTML paths; keep the escaping tests.
- Export contains full session text, including anything the model/tools printed.
- No TLS; do not expose the port through a proxy.

## Non-goals

Remote access, multi-user auth, cloud relay, OAuth, and encrypted browser
storage are out of scope until a separate ADR and threat model exist.
