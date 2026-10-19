# The runner image

A **runner** is the same session host as the person's own machine: `purlis serve`, on a
machine with no window (spec #2041). In a pool, Docker on this machine or a cluster, each
runner is a container started from a **runner image**. purlis's CI builds one for every
green `main`. A team can also build its own, as long as it keeps the contract below.

## purlis's own image

- **Where:** `ghcr.io/purlis/purlis-runner`, a private package of the `purlis` organisation
  on GitHub. It stays private until the maintainer rules to publish it, and pulling it needs
  a GitHub sign-in with read access to the package. Before each push, the workflow checks
  through GitHub's packages API that the package is private. On the very first run there is
  no package yet, so a placeholder holding one small text file is pushed to create it, and
  the package is shown to be private before any of purlis goes in. Before any tag or
  signature, the workflow checks again that the package is private, and that the registry
  refuses both images to anyone who has not signed in. If either check cannot get a clear
  answer, the run stops.
- **What:** `linux/amd64` and `linux/arm64` under one index. Each architecture is built on
  a GitHub runner of that architecture, from the `purlis` binary compiled there the way the
  Linux release binary is: the pinned toolchain, `cargo auditable`, the `dev-release`
  profile, no cache. purlis's plugin is copied in beside it from the same commit.
- **Tags:** `sha-<commit>` for the commit it was built from, and `main` for the image of
  the latest commit. `main` only moves forward: when an older commit's run finishes after
  a newer one's, its image gets only its `sha-<commit>` tag. Tags can move, so a pool names
  the image by its **digest**. The run's summary and its `runner-image` artifact record the
  digest.
- **Signed:** each image is signed keyless with [cosign](https://docs.sigstore.dev/), by the
  `runner-image.yml` workflow on `main`, through GitHub's OIDC identity. No signing key is
  stored anywhere. Keyless signing writes an entry to Sigstore's public transparency log.
  The entry names the image and its digest, and the image itself stays private. The
  maintainer accepted that entry (ruled 2026-10-15). To check an image before you use it:

  ```bash
  cosign verify ghcr.io/purlis/purlis-runner@sha256:<digest> \
    --certificate-identity https://github.com/purlis/purlis/.github/workflows/runner-image.yml@refs/heads/main \
    --certificate-oidc-issuer https://token.actions.githubusercontent.com
  ```

- **Checked:** each architecture's image is pushed untagged, by digest, and pulled back.
  The contract below then runs on exactly the bytes that were pushed. Nothing is tagged or
  signed until it has passed for both architectures. A pull request that changes the image
  builds and checks it the same way, and pushes nothing.

The image is Ubuntu 26.04 LTS (its base pinned by digest) with `ca-certificates`, `git` and
`tini`, the `purlis` binary and purlis's plugin beside it, and one user, `purlis` (10001),
who is not root. Its source is [`runner-image/Dockerfile`](../runner-image/Dockerfile).

## The contract

Any runner image, purlis's own or a team's, holds all of this.

### What the check reads

`tools/runner-image-contract.mjs` checks these rows:

```bash
node tools/runner-image-contract.mjs <image>
```

It reads the image's config and runs a check inside the image as the image's own user. Then
it starts the image's default command. It prints every problem it found, one to a line.

| What | Where, and why |
|---|---|
| Linux | The image is for `linux`. |
| `/bin/sh` | A POSIX shell at `/bin/sh`. The check inside the image runs with it. |
| The `purlis` binary | An executable at `/usr/local/bin/purlis`, and `purlis --version` answers with a line that starts with `purlis `. |
| purlis's plugin | `/usr/local/lib/purlis/plugin`, holding `.claude-plugin/plugin.json`, readable by the image's user. `purlis serve` arms a Claude Code chat with the plugin it finds beside its binary, and refuses a sandboxed Claude Code chat when there is none. |
| A user who is not root | `USER` is a user number other than 0 (`USER 10001:10001`), so a cluster can check `runAsNonRoot`. Inside the image, `id -u` is not 0. |
| A home | `HOME` is an absolute path to a folder the user can write. purlis keeps its config and data under it, by the XDG base directory rules, and a kept claim's volume is mounted there. |
| `/tmp` | Writable. The session host makes its socket's folder there (`/tmp/purlis-<uid>/`, closed to every other user) unless `XDG_RUNTIME_DIR` names a folder of the user's own. With a read-only root file system, mount a writable volume at `/tmp` and at `HOME`. |
| The default command | `purlis serve`, run under an init that reaps processes (`tini`, `dumb-init`, `catatonit` or `docker-init`) as PID 1: `ENTRYPOINT ["/usr/bin/tini", "--", "/usr/local/bin/purlis"]` and `CMD ["serve"]`. It prints `ready` within a minute. |
| Tools | `git` and `bash` on `PATH`, and a certificate bundle at `/etc/ssl/certs/ca-certificates.crt`. |
| No harness | No `claude`, `codex` or `opencode` on `PATH`. A runner gets exactly the harness version its profile pins (spec #2030). |

### What the team keeps

The check does not read these rows. The team building the image keeps them.

| What | Why |
|---|---|
| The same version as the app | `/usr/local/bin/purlis` is the version the person's app runs. Its `--version` line ends in `(dev-release profile)` or `(release profile)`. Take the binary from purlis's image of that version, by digest. |
| A matching C library | purlis's binary is built on Ubuntu 24.04 and needs glibc 2.39 or later. |
| An architecture purlis builds | `amd64` or `arm64`. |
| No secret | No credential, token or key is built into the image or set in its environment. What a runner chat needs comes from the person's desktop, one command at a time (spec #2041). |
| No identity in `HOME` | The image's `HOME` holds no purlis machine store with a device id, no link key and no peers file: nothing a build step got by running `purlis` in it. A new runner's home starts as a copy of the image's, and a runner mints its device id and makes its link key at its one pairing, so a home that came with any of them is refused at pairing. Nothing in the image runs a `purlis` command before `purlis serve` (an entrypoint wrapper or a health check): one that mints a device id first makes the pairing refuse too. Everything else in `HOME` is the runner's first contents, trusted as the image's binaries are (ADR 0078, amended 2026-10-18). |

### What the Linux sandbox needs

A chat on a runner runs inside purlis's Linux sandbox, exactly as a chat on the person's
machine does (#1040). The sandbox is Landlock, a seccomp filter the chat applies to itself,
and a supervisor process that is the chat's parent. It needs no extra capability, no
privileged container and no user namespace, so it works the same in Docker and in a pod. It
needs:

- a Linux kernel with Landlock version 6 or later (Linux 6.12 or later), enabled
  (`landlock` in the kernel's list of security modules);
- a container runtime whose seccomp profile allows the `landlock_create_ruleset`,
  `landlock_add_rule`, `landlock_restrict_self`, `seccomp` and `process_vm_readv` system
  calls. Docker's default profile and Kubernetes' `RuntimeDefault` allow them.

On a kernel without it, a sandboxed chat is refused and the refusal says which version is
needed.

## On a Docker pool

A Docker pool on the person's machine checks each runner container before it is paired and
before every link. One of the checks is that the container runs on a standard runtime: `runc`
or `crun`. Docker Desktop's Enhanced Container Isolation runs every container on the Sysbox
runtime instead, so while it is turned on a Docker pool refuses its runners, and the refusal
says the container does not run on a standard container runtime.

## A team's own image

Start from any Linux base, add what your team's chats need, and keep the contract. Take the
`purlis` binary from purlis's image, by digest:

```dockerfile
COPY --from=ghcr.io/purlis/purlis-runner@sha256:<digest> /usr/local/bin/purlis /usr/local/bin/purlis
```

[`runner-image/team-example.Dockerfile`](../runner-image/team-example.Dockerfile) is a
complete example on Debian, with two tools of a team's own. CI builds it and checks it
against the contract on every run. That shows an image other than purlis's can keep the
contract.
