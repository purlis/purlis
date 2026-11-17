# Amended 2026-11-15: a runner's install checks and places in one command, and guards the transfer alone

**Built by #723's review fixes** (RR-14, after the review of #6679), on §5 as V9 and V22a
settled it. What follows was decided in implementation and is marked *decided in
implementation, to be confirmed*.

1. **What the check protects is the transfer, and only the transfer.** It holds that the bytes
   placed in `<config>/server/<ver>/purlis` are the bytes the release signed, whatever the network,
   a proxy or a mirror did to them on the way. It does **not** protect against a compromised
   runner account: whoever can run commands as that user can answer the hash for the file, or
   replace the binary after it is placed, and no check run as that user can tell. §5's *The trust
   this rests on* already says the check guards the file's path and not the runner from itself;
   this names the case people ask about. A runner account someone else controls is a runner to
   remove, not one to install on. *Decided in implementation, to be confirmed* (D-723-11).
2. **Hashing, comparing, `chmod` and `mv` are one command on the runner**
   (`runnerinstall::settle_command`). The verified digest is written into the command, the runner
   compares, and only on a match makes the file executable and renames it; on any other answer the
   same command deletes it and exits with a code of its own. No second SSH run sits between the
   check and the placing, so nothing can be changed between them by the route going away. A version
   already there is hashed again the same way before it is trusted, and placed again, by one rename
   over it, where it is not the file verified (a host running the old file keeps running it).
   *Decided in implementation, to be confirmed* (D-723-12).
3. **Every command is handed to POSIX `sh`, as ADR 0123's forward is** (amended 2026-11-12):
   `exec sh -c '<script>'`, the script holding no `'`, `\` or `!`, so `bash`, `zsh` and `fish`
   as a login shell read it alike. This sharpens the *two places at the far end are shell by
   design*: the probe is POSIX `sh` because it is run by `sh`, whatever the login shell is.
   *Decided in implementation, to be confirmed* (D-723-13).
4. **Only curl downloads, held to https through every redirect** (`curl -q --proto =https
   --proto-redir =https`). wget has no switch that refuses a redirect from https to http for one
   file (its `--https-only` is for the links a recursive download follows), so a runner with wget
   and no curl is told so, and the person installs curl or sends the binary from the desktop.
   *Decided in implementation, to be confirmed* (D-723-14).
5. **The temporary file is made by a run of its own before anything is sent**, so the desktop
   knows its path and deletes it however the delivery ends, a connection cut off included. What it
   could not delete, a desktop that went away mid-delivery, the next probe deletes once it is a day
   old. *Decided in implementation, to be confirmed* (D-723-15).
6. **A dev build's runner assets stay on the `dev` release for five dev builds**, not until the
   next one. Their names carry their version, so an address never names another build's asset, and
   a desktop a few dev builds behind still finds its own (`tools/dev-runner-assets.mjs`). Not a tag
   per dev build: `release.yml` creates, moves and deletes no tag. *Decided in implementation, to be
   confirmed* (D-723-16).
7. **An installed version's host socket is named by a hash of its version**, `purlisd-v<8 hex>.sock`,
   not the version itself, which may be 64 characters: the whole path must fit macOS's 104-byte
   socket address, and the user's own temporary directory there takes about half of it.
   *Decided in implementation, to be confirmed* (D-723-17).
