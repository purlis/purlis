# The runner image

A **runner** is the same session host as the person's own machine: `purlis serve`, on a
machine with no window (spec #2041). In a pool, Docker on this machine or a cluster, each
runner is a container started from a **runner image**. purlis's CI builds one for every
green `main`. A team can also build its own, as long as it keeps the contract below.

## purlis's own image

- **Where:** `ghcr.io/purlis/purlis-runner`, a private package of the `purlis` organisation
  on GitHub. It stays private until the maintainer rules to publish it, and pulling it needs
  a GitHub sign-in with read access to the package.
- **What:** `linux/amd64` and `linux/arm64` under one index. Each architecture is built on
  a GitHub runner of that architecture, from the `purlis` binary compiled there the way the
  Linux release binary is: the pinned toolchain, `cargo auditable`, the `dev-release`
  profile, no cache.
- **Tags:** `main` for the newest image, and `sha-<commit>` for the commit it was built
  from. A tag can move, so a pool names the image by its **digest**. The run's summary
  and its `runner-image` artifact record the digest.
- **Signed:** each image is signed keyless with [cosign](https://docs.sigstore.dev/), by the
  `runner-image.yml` workflow on `main`, through GitHub's OIDC identity. No signing key is
  stored anywhere. To check an image before you use it:

  ```bash
  cosign verify ghcr.io/purlis/purlis-runner@sha256:<digest> \
    --certificate-identity https://github.com/purlis/purlis/.github/workflows/runner-image.yml@refs/heads/main \
    --certificate-oidc-issuer https://token.actions.githubusercontent.com
  ```

- **Checked:** nothing is tagged or signed until the contract below has passed on the
  exact bytes that were pushed, for both architectures. A pull request that changes the
  image builds it and checks it the same way, and pushes nothing.

The image is Ubuntu 24.04 (its base pinned by digest) with `ca-certificates`, `git` and
`tini`, the `purlis` binary, and one user, `purlis` (10001), who is not root. Its source is
[`runner-image/Dockerfile`](../runner-image/Dockerfile).

## The contract

Any runner image, purlis's own or a team's, holds all of this.
`tools/runner-image-contract.mjs` checks an image against it:

```bash
node tools/runner-image-contract.mjs <image>
```

It reads the image's config, runs a check inside the image as the image's own user, and
starts the image's default command. Then it prints every problem it found, one to a line.

| What | Where, and why |
|---|---|
| Linux | The image is for `linux`, on `amd64` or `arm64`. |
| The `purlis` binary | An executable at `/usr/local/bin/purlis`, of the version the desktop's app runs. `purlis --version` answers `purlis <version> (<profile> profile)`. purlis's binary is built on Ubuntu 24.04 and needs glibc 2.39 or later. |
| A user who is not root | `USER` is a user number other than 0 (`USER 10001:10001`), so a cluster can check `runAsNonRoot`. The image runs as that user and never as root. |
| A home | `HOME` is an absolute path to a folder the user can write. purlis keeps its config and data under it, by the XDG base directory rules, and a kept claim's volume is mounted there. |
| `/tmp` | Writable. The session host makes its socket's folder there (`/tmp/purlis-<uid>/`, closed to every other user) unless `XDG_RUNTIME_DIR` names a folder of the user's own. With a read-only root file system, mount a writable volume at `/tmp` and at `HOME`. |
| The default command | `purlis serve`, run under an init that reaps processes (`tini`, `dumb-init`, `catatonit` or `docker-init`) as PID 1: `ENTRYPOINT ["/usr/bin/tini", "--", "/usr/local/bin/purlis"]` and `CMD ["serve"]`. It prints `ready` once it is serving. |
| Tools | `git` and `bash` on `PATH`, and a certificate bundle at `/etc/ssl/certs/ca-certificates.crt`. |
| No harness | The image carries no harness (`claude`, `codex`, `opencode`). A runner gets exactly the harness version its profile pins (spec #2030). |
| No secret | No credential, token or key is built into the image. What a runner chat needs comes from the person's desktop, one command at a time (spec #2041). |

### What the Linux sandbox needs

A chat on a runner runs inside purlis's Linux sandbox (#1040), exactly as a chat on the
person's machine does. The sandbox is Landlock plus a seccomp filter that purlis applies to
itself. It needs no extra capability, no privileged container and no user namespace, so it
works the same in Docker and in a pod. It needs:

- a Linux kernel with Landlock enabled (`landlock` in the kernel's list of security
  modules);
- a container runtime whose seccomp profile allows the `landlock_create_ruleset`,
  `landlock_add_rule`, `landlock_restrict_self` and `seccomp` system calls. Docker's
  default profile and Kubernetes' `RuntimeDefault` allow them.

## A team's own image

Start from any Linux base, add what your team's chats need, and keep the contract. Take the
`purlis` binary from purlis's image, by digest:

```dockerfile
COPY --from=ghcr.io/purlis/purlis-runner@sha256:<digest> /usr/local/bin/purlis /usr/local/bin/purlis
```

[`runner-image/team-example.Dockerfile`](../runner-image/team-example.Dockerfile) is a
complete example on Debian, with two tools of a team's own. CI builds it and checks it
against the contract on every run, so the contract is known to be one an image other than
purlis's can keep.
