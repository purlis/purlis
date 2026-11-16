### Security

- **A sandboxed chat never reads your SSH private keys or your forge and cloud credential
  files.** At the full sandbox and the basic sandbox, a chat can't read the private keys in
  `~/.ssh`, `~/.git-credentials`, `~/.netrc`, `~/.aws/credentials`, gcloud's credentials,
  `~/.azure`, `~/.docker/config.json`, or gh's and glab's token files. A chat there gets a forge
  token only through purlis's vault, so plain `gh` or git over https that relies on those files
  stops working, and the block's notice says what works instead. Your own SSH agent stays
  unreachable until you allow it for a chat (ADR 0067 as amended 2026-11-16, Q321).
