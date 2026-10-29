# The runner image

A **runner** is the same session host as the person's own machine: `purlis serve`, on a
machine with no window (spec #2041). In a pool, Docker on this machine or a cluster, each
runner is a container started from a **runner image**. purlis builds one for every release it
publishes, dev and stable, of the same version as the release's app. A team can also build its
own, as long as it keeps the contract below.

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
- **Its version:** each release (`release.yml`) calls `runner-image.yml` for the commit and the
  version it publishes, so the image's `purlis --version` says the version its app announces,
  a dev build's `<version>-dev.<run>` included (`PURLIS_VERSION_STAMP`). The release's update
  manifest names the image by digest as `runnerImage`, signed with the updater's key (see
  [Updating](updating.md#the-runner-image-a-release-names)). After an update, a Docker pool of
  this image gets a runner of the new version beside its old one: new chats start on it, and
  the old runner keeps its chats until they end, then is removed.
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
    --certificate-github-workflow-repository purlis/purlis \
    --certificate-oidc-issuer https://token.actions.githubusercontent.com
  ```

  `runner-image.yml` is a workflow a release calls, so the certificate is checked to come from a
  run in `purlis/purlis` itself, and not only to name the workflow.

  The image of a stable release is signed on its tag: use
  `runner-image.yml@refs/tags/v<version>` as the identity.

- **Build provenance and an SBOM:** each published image carries SLSA build provenance,
  attached to the image the release names (its multi-architecture index), and a CycloneDX SBOM
  of each architecture's image. The SBOM lists the image's Ubuntu packages and the crates the
  `purlis` binary was built from. Each is stored by GitHub and pushed to the registry beside
  the image, and the run that pushed the image verifies each one before it ends, so a release
  never names an image without them. To check them yourself, signed in to the registry
  (`gh auth token | docker login ghcr.io -u <you> --password-stdin`):

  ```bash
  # Provenance: built by runner-image.yml in purlis/purlis, on a GitHub-hosted runner.
  gh attestation verify oci://ghcr.io/purlis/purlis-runner@sha256:<digest> \
    --repo purlis/purlis \
    --signer-workflow purlis/purlis/.github/workflows/runner-image.yml \
    --deny-self-hosted-runners

  # The SBOM of one architecture's image: its digest is in `docker buildx imagetools inspect`.
  gh attestation verify oci://ghcr.io/purlis/purlis-runner@sha256:<arch digest> \
    --repo purlis/purlis \
    --signer-workflow purlis/purlis/.github/workflows/runner-image.yml \
    --predicate-type https://cyclonedx.org/bom \
    --format json | jq '.[0].verificationResult.statement.predicate'
  ```

  Add `--source-digest <commit>` to check that it was built from a given commit.

- **Checked:** each architecture's image is pushed untagged, by digest, and pulled back.
  The contract below then runs on exactly the bytes that were pushed. Nothing is tagged or
  signed until it has passed for both architectures. A pull request that changes the image
  builds and checks it the same way, and pushes nothing.

The image is Ubuntu 26.04 LTS (its base pinned by digest) with `ca-certificates`, `git` and
`tini`, the libraries and the font the browser engine needs (below), the `purlis` binary and
purlis's plugin beside it, and one user, `purlis` (10001), who is not root. Its source is [`runner-image/Dockerfile`](../runner-image/Dockerfile).

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
| A home | `HOME` is an absolute path to a folder the user can write. purlis keeps its config and data under it, by the XDG base directory rules. On a Docker pool a volume of the runner's own is mounted there; on a cluster pool the runner's home is made on its pod's volume from it ([below](#on-a-cluster-pool)), so `HOME` is not `/` and does not hold `/var/lib/purlis`, through a link or otherwise. |
| `/tmp` | Writable. The session host makes its socket's folder there (`/tmp/purlis-<uid>/`, closed to every other user) unless `XDG_RUNTIME_DIR` names a folder of the user's own. With a read-only root file system, mount a writable volume at `/tmp` and at `HOME`. A Docker pool runs every runner that way: a read-only root, its home on a volume of its own, and a `tmpfs` at `/tmp` mounted `noexec`, so nothing in the image may need to run a program from `/tmp`. |
| The default command | `purlis serve`, run under an init that reaps processes (`tini`, `dumb-init`, `catatonit` or `docker-init`) as PID 1: `ENTRYPOINT ["/usr/bin/tini", "--", "/usr/local/bin/purlis"]` and `CMD ["serve"]`. It prints `ready` within a minute. A Docker pool runs it as `purlis serve --suspends-unlinked`, so a runner left a day without a link from its desktop stops itself, keeping its home (#2051); a pool with its own time (`suspendAfter`, #3471) runs it as `purlis serve --suspends-unlinked=<duration>`, such as `=12h`, which only a purlis since #3471 takes. Its standard error is its log: purlis's own `info` lines and up, and a library's warnings, each redacted as the app's log is, kept by whatever keeps the container's log. |
| Tools | `git` and `bash` on `PATH`, and a certificate bundle at `/etc/ssl/certs/ca-certificates.crt`. |
| No harness | No `claude`, `codex` or `opencode` on `PATH`. A runner gets exactly the harness version its profile pins (spec #2030). |

### What the team keeps

The check does not read these rows. The team building the image keeps them.

| What | Why |
|---|---|
| The same version as the app | `/usr/local/bin/purlis` is the version the person's app runs. Its `--version` line ends in `(dev-release profile)` or `(release profile)`. Take the binary from purlis's image of that version, by digest. purlis never moves a pool of a team's own image to another version: when the app updates, the pool's row in the window says its runner is behind, and the team builds the image again from the new version and adds the pool again. |
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
  calls. Docker's default profile and Kubernetes' `RuntimeDefault` allow them;
- for a chat's server to listen, `pidfd_open` and `pidfd_getfd` too: the supervisor makes a
  chat's `listen` on the chat's own socket, which it takes with them. Docker's default profile
  allows `pidfd_getfd` only with `CAP_SYS_PTRACE`, so a Docker pool makes each runner with its
  own profile, Docker's default and that one call besides, with the browser engine's calls
  (below; `crates/purlis-core/src/dockerpool/seccomp.json`). Where a runtime refuses it, no server a
  chat starts listens, and the refusal is told to the person as a blocked bind. That reaches
  further than servers: every program that listens on a socket of its own stops there too,
  unix sockets included, such as a `tmux` server, an `ssh` ControlMaster, `gpg-agent` or a
  language server. A runtime refuses it under Docker's or containerd's default profile,
  Kubernetes' `RuntimeDefault` (#3413), Yama's `ptrace_scope` 3, and for a program that is not
  dumpable. A Docker pool runner made by a purlis from before this profile still runs Docker's
  default: it is still linked, and its chats' servers listen once it is removed and added
  again.

On a kernel without it, a sandboxed chat is refused and the refusal says which version is
needed.

### What a browser on a runner needs

A runner chat's browser runs on its runner, beside it (#2050). purlis's host downloads the
pinned browser engine into its home on the first browser, checked, as on any machine, so the
engine is not in the image. To start, the engine needs:

- **its libraries and a font.** purlis's image carries them: the libraries the engine's own
  `deb.deps` lists (NSS, ATK and AT-SPI, CUPS, D-Bus, DRM, GBM, GLib, Pango, Cairo, udev, X11
  and XCB's client libraries, xkbcommon and ALSA) and `fonts-liberation`. A team's image adds
  them too, or its runners' chats get no browser, and the person is told why.
- **its own sandbox.** purlis never runs the engine without it. The sandbox makes a user, a
  process and a network namespace and changes its root, so a Docker pool makes each runner
  with a profile that allows exactly those calls besides Docker's default
  (`crates/purlis-core/src/dockerpool/seccomp.json`, ruled 2026-10-28). Only the browser engine
  may make namespaces on a runner: the runner's host starts everything else unable to. A
  pool gives that profile only to an image whose `purlis` does so, asked of the image before
  its runner is made; an image of an earlier `purlis` gets the earlier profile, and its
  runners start no browser. A runner a chat ran on without the sandbox starts no browser and
  is filled no login until it is made again. A runner made by a purlis from before this profile still links; its browsers are
  refused, and the person is told to remove its pool and add it again.
- **shared memory.** A Docker pool gives each runner a `/dev/shm` a quarter of its memory: the
  engine shares its pages' memory there, and Docker's own 64 MiB is too little for a busy page.

## On a Docker pool

A Docker pool on the person's machine checks each runner container before it is paired and
before every link. One of the checks is that the container runs on a standard runtime: `runc`
or `crun`. Docker Desktop's Enhanced Container Isolation runs every container on the Sysbox
runtime instead, so while it is turned on a Docker pool refuses its runners, and the refusal
says the container does not run on a standard container runtime.

## On a cluster pool

In a pool of purlis's [cluster operator](cluster-operator.md), the runner's pod runs the image
twice: first as its `link-key` step, `purlis bridge --make-key`, with the image's own
entrypoint, before anything else in the pod, which makes the runner's link key, and then as the
runner. So the image's entrypoint is `purlis` itself, under its init, and takes the arguments
the pod gives it, as `ENTRYPOINT [..., "/usr/local/bin/purlis"]` with `CMD ["serve"]` does. The
key's private half is handed to the runner in `/run/purlis-link/`, a folder in memory the pod
mounts there, so nothing of the image may be needed at that path; its public half is written
to `/dev/termination-log`, for the operator to vouch for (#2056). That step is the one
`purlis` command a cluster pod runs before `purlis serve`: it mints no device id.

**The runner's home is on the pod's volume** (#2057). A cluster pod mounts a volume at
`/var/lib/purlis` (a kept claim's own, or a folder of the node's for a fresh one) and sets the
runner's `HOME` to `/var/lib/purlis/home`, so nothing of the image may be needed at those paths.
The first step makes that home, the first time the volume is used, as a copy of the image's own
`HOME`: its folders, files and links, as the image holds them. So the image's `HOME` is still
the runner's first contents, as on a Docker pool. On a kept claim's volume the home is left as
it is after that, and when it already keeps the runner's pairing, the step publishes the key the
runner was paired with instead of making one. Keep `XDG_CONFIG_HOME` and `PURLIS_CONFIG_HOME`
unset in the image, or under `HOME`: purlis keeps the runner's pairing in its config home, and a
config home off the volume is lost with the pod. The pod's processes are in group 10001, which
may write the volume.

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
