# Amendment, 2026-11-28: the other machines' images are pulled by digest

The CI seam of Testing (the dockerised sshd beside dind, rootless Podman's sshd, and, for RR-14
and RR-15, the machine without Docker and the one that boots systemd) was built in every run of
`remote-parity`, from Alpine's CDN and Ubuntu's mirror. On 2026-11-28 one `apk add` that could not
reach the CDN failed the job, and the systemd machine it then never started made eleven fault tests
fail as "the fixture is missing" (#7983): a mirror's hiccup read as eleven trust failures.

**Each of the four images is built once, from main, and pinned by digest** (#7983).
`.github/workflows/parity-fixture-images.yml` builds a fixture whose pin is missing or behind its
files, pushes it to `ghcr.io/purlis/purlis-parity-<fixture>` as a public package, checks that it
pulls by digest without signing in, and proposes the pin in one pull request a person merges.
`tests/parity/fixture-images.json` holds each pin beside its source: the Dockerfile, the files it
copies and its build arguments, as one digest. `remote-parity` pulls each pinned digest
(`tools/fixture-images.mjs prepare-all`) and asks no mirror for anything. A pull request that
changes a fixture builds that fixture in its own run, as before, and warns that it did. Nothing a
pull request starts can push, and the jobs that write run only on main.

**A fixture that cannot be had is one cause.** `prepare-all` names the fixture and its image in
one error (`fixture image <name> (<ref>) unavailable: …`, or `could not be built from …`), and
every machine and suite that needs it is skipped. The fault suite and the behaviour suite run after
a failed test step as before, but only on machines that came up, so a missing machine never shows
as its tests failing. `PURLIS_PARITY` still makes a missing fixture fail a test that runs.
