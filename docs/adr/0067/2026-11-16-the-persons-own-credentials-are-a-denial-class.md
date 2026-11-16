# Amended 2026-11-16: the person's own credentials are a denial class

*Ruling Q321, points 17 and 27, with the file list as the person ruled it (escape table rows
E-FILE-20, E-FILE-21 and E-IPC-08).* Section 5 gains a sixth class. Before this, the floor said that a read outside the denial classes is not
held, and the full sandbox said nothing about the person's own keys. So a chat could read
`~/.ssh/id_ed25519` or `~/.aws/credentials` at either level.

6. **A chat never reads the person's own credentials** (`credentials`). This holds at the full
   sandbox and at the floor (Basic), in what every harness is compiled to: Claude Code's
   `denyRead` and its `Read()` deny rules, the Seatbelt profile of a wrapped chat on macOS, and
   the profile purlis's own wrap reads on Linux. Each entry is denied to reads and writes. The
   class is lifted only by the per-chat opt-out in section 7, as every class is, and the audit's
   `lifted` list names it. No Sandbox runs as the person and holds none of it.

   - **An SSH private key in `~/.ssh`.** These names are denied whether or not they are there,
     so a key made while the chat runs is held: `id_rsa`, `id_dsa`, `id_ecdsa`, `id_ecdsa_sk`,
     `id_ed25519`, `id_ed25519_sk`, `id_xmss` and `identity`. So is every other file directly in
     `~/.ssh` as the chat starts, unless it is one that holds no secret. Those stay readable,
     because `ssh` and `git` need them to work through an agent: a `.pub` key, `known_hosts*`,
     `config`, `config.d`, `authorized_keys*`, `allowed_signers`, `environment` and `rc`. All of
     them but `known_hosts*` and `allowed_signers` are already held from writes by class 5. So
     the person's `id_*` and `*_key` files are denied, and so is a key by any other name.
   - **The forge and cloud credential files**, as the person ruled them, each with what is
     below it (`CREDENTIAL_FILES`): `~/.git-credentials`, `~/.netrc`, `~/.aws/credentials`,
     gcloud's `~/.config/gcloud/credentials.db` and `~/.config/gcloud/legacy_credentials/`,
     `~/.azure/`, `~/.docker/config.json`, gh's `~/.config/gh/hosts.yml` and glab's
     `~/.config/glab-cli/config.yml`.

   Each is denied to writes too, so no chat logs in there. This changes
   [the home's Docker folders](2026-10-20-the-homes-docker-folders-are-never-a-chats-to-write.md):
   `~/.docker/config.json` is no longer the chat's to write, and the rest of `~/.docker` still
   is. A person's Allow on a folder that holds one of these files, such as `~/.config/gh`,
   lifts the folder and never the file in it.

   **What this costs, said plainly.** A chat at the full sandbox or the floor gets a forge token
   only through purlis's vault. Plain `gh`, `glab` or git over https that relies on one of these
   files stops working there, and so does a cloud command that reads its credentials file. A
   refused read of one says so in its Notice, with the ways that work: a forge token through
   purlis's vault, a push through the app, or the person's SSH agent allowed for the chat. It
   never gives the vault's sentence, and no grant lifts it.

   **What stays a residual:** a key with a name of the person's own, added to `~/.ssh` after the
   chat started; a key kept outside `~/.ssh`; a credential file a tool's own variable moves
   (`GH_CONFIG_DIR`, `XDG_CONFIG_HOME`, `AWS_SHARED_CREDENTIALS_FILE`, `CLOUDSDK_CONFIG`,
   `DOCKER_CONFIG`); and a token a tool keeps elsewhere in its folder, such as gcloud's
   `access_tokens.db`. Each is read until a later amendment names it.

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

**Future policy keys (Q307 Q6).** Nothing here is a new hard-coded limit. The policy (M75) names
each decision as a key, with today's value as its default:

| Key | Default | Decides |
| --- | --- | --- |
| `sandbox.credentials.ssh-keys` | `deny` | reads of an SSH private key in `~/.ssh` |
| `sandbox.credentials.git-credentials` | `deny` | reads of `~/.git-credentials` |
| `sandbox.credentials.netrc` | `deny` | reads of `~/.netrc` |
| `sandbox.credentials.aws` | `deny` | reads of `~/.aws/credentials` |
| `sandbox.credentials.gcloud` | `deny` | reads of gcloud's `credentials.db` and `legacy_credentials/` |
| `sandbox.credentials.azure` | `deny` | reads of `~/.azure/` |
| `sandbox.credentials.docker` | `deny` | reads of `~/.docker/config.json` |
| `sandbox.credentials.gh` | `deny` | reads of gh's `hosts.yml` |
| `sandbox.credentials.glab` | `deny` | reads of glab's `config.yml` |
| `sandbox.sockets.ssh-agent.persons` | `ask` | the person's own agent socket (granted per chat, project or you) |
| `sandbox.sockets.ssh-agent.purlis` | `allow` | the per-chat agent purlis holds (#1350) |

Under Q308's templates, the `sandbox` family's `floor` keeps today's floor rules and this class,
and `off` lifts it with the rest.
