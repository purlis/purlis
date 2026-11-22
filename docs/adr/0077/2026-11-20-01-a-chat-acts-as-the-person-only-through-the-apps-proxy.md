# Amended 2026-11-20: a chat acts as the person only through the app's proxy

Proposed by the steward on 2026-11-20 as slice H of the one-login train, from ADR 0128 ("Amendments
this record makes, verbatim"), which this file writes. It takes effect with the `forge-wiring`
feature; until the train turns that on, the text it replaces still describes the build. It
supersedes the amendments of 2026-10-12-04 (the CLI login import, for GitHub) and 2026-10-29-01
(an agent's GitHub token is minted from the organisation's App).

## What changes

- **The record's title.** Was: *"A forge sign-in is the human's, through one registration per forge
  host, and it mints nothing for an agent"*. Now: *"A forge sign-in is the human's, through one
  registration per forge host, and a chat uses it only through the app's proxy"*. It still mints
  nothing for an agent.
- **FI3.** Was: *"The human's UI token never reaches an agent; agents keep their own narrow
  identity (SD-7a/b, gap G3), and gain no new power from this feature before those land."* Now:
  *"The human's UI token never reaches an agent. A chat acts as the person through the app's forge
  proxy, and holds no token. Within the repos recorded for the chat it has the person's whole role
  there, no more than the proxy's allowlist permits and nothing outside those repos; there is no
  agent identity (SD-7a/b retired)."* The chat having the person's role within its repos is a new
  power relative to a bot's, and this record says so.
- **V16c.** Was: *"Per-agent GitHub tokens (SD-7a) come from a GitHub App the organisation
  registers itself, with charter guiding the setup; […] charter's shared GitHub App (FW-1) is for
  sign-in only and its private key never ships."* Now: *"There are no per-agent tokens. purlis's
  one OAuth registration serves sign-in and, through the proxy, a chat's calls as the person; it
  has no private key. The organisation registers nothing for agents. (A GHES or self-hosted GitLab
  application is registered by an owner, ADR 0128 D1.3.)"*

## What does not change

A project file's `mint = "github-app"` line and a tool that hands a vault value in a forge token
variable keep parsing. They run nothing, and one Notice and a `purlis doctor` row say to sign in
and delete the line. purlis rewrites no project file, and deletes no vault item and no keyring
entry except on the person's press.
