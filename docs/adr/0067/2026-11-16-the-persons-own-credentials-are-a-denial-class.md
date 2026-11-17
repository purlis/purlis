# Amended 2026-11-16: the person's own credentials are a denial class

*Ruling Q321, points 17 and 27, with the file list as the person ruled it (escape table rows
E-FILE-20, E-FILE-21 and E-IPC-08), and ruling Q325 of 2026-11-17, which holds the class at the
floor only where the project's policy enforces it.* Section 5 gains a sixth class. Before this, the floor said that a read outside the denial classes is not
held, and the full sandbox said nothing about the person's own keys. So a chat could read
`~/.ssh/id_ed25519` or `~/.aws/credentials` at either level.

6. **A chat never reads the person's own credentials** (`credentials`), **at the full sandbox,
   and at the floor (Basic) only where the project's policy enforces it** (ruling Q325, which
   amends Q321 point 17). Under the shipped template, `watch`, the floor leaves the class out
   whole, so the person's own `gh`, git over https, `aws`, `gcloud`, `az` and `docker` keep
   working there as before, and no folder above one of these files is held either: a tool's first
   run can still make `~/.aws` or `~/.config/gcloud`. A project holds it at the floor with
   `[policy] secrets.credential_files = "enforce"` (ADR 0125's family of that name, `off` or
   `enforce`), and the `team` and `strict` templates enforce it. Where it holds, it is in what
   every harness is compiled to: Claude Code's `denyRead` and its `Read()` deny rules, the
   Seatbelt profile of a wrapped chat on macOS, and the profile purlis's own wrap reads on Linux.
   Each entry is denied to reads and writes. The class is lifted only by the per-chat opt-out in
   section 7, as every class is, and the audit's `lifted` list names it. No Sandbox runs as the
   person and holds none of it.

   - **The SSH folder, `~/.ssh`, denied whole**, so a key made in it while the chat runs is held
     whatever its name (`id_*`, `*_key`, `*.pem`, or one of the person's own), and so is one in a
     folder below it. Inside it, and only directly in it, what holds no secret is read again,
     because `ssh` and `git` need it to work through an agent: `config`, `known_hosts`,
     `known_hosts.old` and a public key (`*.pub`). Those are read, never written: Claude Code's
     `allowRead`, and an `allow file-read*` after the folder's rule in the profile. Claude Code's
     own Read tool is held from the whole folder, as a permission rule takes nothing back.
   - **The forge and cloud credential files**, each with what is below it (`CREDENTIAL_FILES`):
     `~/.git-credentials`, `~/.config/git/credentials`, `~/.netrc`, `~/.aws/credentials`,
     `~/.aws/sso/cache/`, `~/.aws/cli/cache/`, gcloud's `credentials.db`, `access_tokens.db`,
     `application_default_credentials.json` and `legacy_credentials/` under `~/.config/gcloud`,
     `~/.azure/`, `~/.docker/config.json`, gh's `~/.config/gh/hosts.yml` and glab's
     `~/.config/glab-cli/config.yml`.
   - **Where a tool's own variable moves one, the moved file too** (`credential_paths`), as the
     list already follows `OP_CONFIG_DIR`: `GH_CONFIG_DIR` (or `XDG_CONFIG_HOME/gh`) for gh's,
     `XDG_CONFIG_HOME` for git's and glab's, `CLOUDSDK_CONFIG` for gcloud's,
     `AWS_SHARED_CREDENTIALS_FILE`, `DOCKER_CONFIG` and `NETRC`. A value that is not an absolute
     path, or that names the home or a folder above it, moves nothing. The defaults stay denied
     too.

   Each is denied to writes too, so no chat logs in there. This changes
   [the home's Docker folders](2026-10-20-the-homes-docker-folders-are-never-a-chats-to-write.md)
   where the class holds: `~/.docker/config.json` is no longer the chat's to write, and the rest
   of `~/.docker` still is. A person's Allow on a folder that holds one of these files, such as
   `~/.config/gh`, lifts the folder and never the file in it.

   **What this costs, said plainly.** Where the class holds, a chat gets a forge token only
   through purlis's vault. Plain `gh`, `glab` or git over https that relies on one of these files
   stops working there, and so does a cloud command that reads its credentials file, and `ssh`
   does not add a new host to `known_hosts`. A refused read or write of one says so in its
   Notice, whatever else the path is (gh's and glab's folders are later code's too), with the
   ways that work: a forge token through purlis's vault, a push through the app, and, on macOS,
   where a socket can be allowed to a chat (D-sock-7), the person's SSH agent allowed for the
   chat. It says the class holds at the full sandbox and at the basic one where the project's
   policy says so. It never gives the vault's sentence, and no grant lifts it.

   **What stays a residual:** a key kept outside `~/.ssh`; a credential a tool keeps elsewhere
   than this list names, or moves by a variable not named here; and the class at the floor under
   `watch` and `hold`. Each is read until a later amendment names it.

**SSH agents (point 27).** `SSH_AUTH_SOCK` still passes into a chat unchanged (ADR 0047).

- **The person's own agent is unreachable unless granted.** The socket that variable names is,
  to the sandbox, like any other local socket: a chat at either level reaches it only after a
  person allows it, for that chat or always (D-sock-1, ADR 0093), and its Notice says what that
  lends ("lets this chat use your SSH keys"). This was already how both levels compiled, and
  this amendment makes it a rule, with its own tests.
- **purlis's per-chat agent is reachable by design.** It does not exist yet, and its cell is
  owed to #1350. When #1350 builds it, the agent purlis holds for a chat is allowed by exactly
  its path, with no person's grant, and no other socket is. Until it does, a chat at either level signs over SSH only through a
  granted agent, or through the app (`purlis worktree push`).

**Policy keys (Q307 Q6, Q325).** Nothing here is a new hard-coded limit. One key is wired now:

| Key | Default | Decides |
| --- | --- | --- |
| `secrets.credential_files` | `off` under `open`, `watch` and `hold`; `enforce` under `team` and `strict` | whether the floor holds this class (`off` or `enforce`; `log` is enforced). The full sandbox holds it whatever this says. |

The policy (M75) may name the finer decisions as keys of their own, with today's value as each
default:

| Key | Default | Decides |
| --- | --- | --- |
| `sandbox.credentials.ssh-keys` | `deny` | reads in `~/.ssh` but `config`, `known_hosts*` and `*.pub` |
| `sandbox.credentials.git-credentials` | `deny` | reads of git's two credential files |
| `sandbox.credentials.netrc` | `deny` | reads of `~/.netrc` |
| `sandbox.credentials.aws` | `deny` | reads of `~/.aws/credentials` and its SSO and CLI caches |
| `sandbox.credentials.gcloud` | `deny` | reads of gcloud's credentials, tokens and application default credentials |
| `sandbox.credentials.azure` | `deny` | reads of `~/.azure/` |
| `sandbox.credentials.docker` | `deny` | reads of `~/.docker/config.json` |
| `sandbox.credentials.gh` | `deny` | reads of gh's `hosts.yml` |
| `sandbox.credentials.glab` | `deny` | reads of glab's `config.yml` |
| `sandbox.sockets.ssh-agent.persons` | `ask` | the person's own agent socket (granted per chat, project or you) |
| `sandbox.sockets.ssh-agent.purlis` | `allow` | the per-chat agent purlis holds (#1350) |

Under Q308's templates, the `sandbox` family's `off` lifts this class with the rest.
