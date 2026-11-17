### Security

- **A chat at the full sandbox never reads your SSH private keys or your forge and cloud
  credential files.** It can't read what is in `~/.ssh` (but `config`, `known_hosts` and public
  keys), `~/.git-credentials`, `~/.netrc`, AWS's credentials and token caches, gcloud's
  credentials and tokens, `~/.azure`, `~/.docker/config.json`, or gh's and glab's token files,
  also where `GH_CONFIG_DIR`, `XDG_CONFIG_HOME`, `AWS_SHARED_CREDENTIALS_FILE`, `DOCKER_CONFIG`,
  `CLOUDSDK_CONFIG` or `NETRC` moves them. At the basic sandbox your tools keep working as before,
  unless the project's `[policy]` sets `secrets.credential_files = "enforce"` (the `team` and
  `strict` templates do). Where it holds, a chat gets a forge token only through purlis's vault,
  so plain `gh` or git over https that relies on those files stops working, and the block's
  notice says what works instead. Your own SSH agent stays unreachable until you allow it for a
  chat (ADR 0067 as amended 2026-11-16, Q321 and Q325).
