# Amendment, 2026-11-28: the other machines' images are pulled by digest

The CI seam of Testing (the dockerised sshd beside dind, rootless Podman's sshd, and, for RR-14
and RR-15, the machine without Docker and the one that boots systemd) was built in every run of
`remote-parity`, from Alpine's CDN and Ubuntu's mirror. On 2026-11-28 one `apk add` that could not
reach the CDN failed the job, and the systemd machine it then never started made eleven fault tests
fail as "the fixture is missing" (#7983): a mirror's hiccup read as eleven trust failures.

It amends ADR 0068's runner machines too: the machine without Docker and the one that boots
systemd are where RR-14's bootstrap and RR-15's supervision are tested, and their images are
pinned and pulled the same way.

**Each of the four images is built once, from main, and pinned by digest** (#7983).
`.github/workflows/parity-fixture-images.yml` builds a fixture whose pin is missing or behind its
files, pushes it to `ghcr.io/purlis/purlis-parity-<fixture>` as a public package, checks that it
pulls by digest without signing in, and pushes the new pins to the branch
`parity-fixture-images/pins`, whose pull request a person opens from the run's notice and merges
(#8009): GitHub Actions may not open pull requests in this repository, and one it opened would
start no checks.
`tests/parity/fixture-images.json` holds each pin beside its source: the Dockerfile, the files it
copies and its build arguments, as one digest. `remote-parity` pulls each pinned digest
(`tools/fixture-images.mjs prepare-all`) and asks no mirror for anything. A pull request that
changes a fixture builds that fixture in its own run, as before, and warns that it did. Nothing a
pull request starts can push, and the jobs that write run only on main.

**A fixture that cannot be had is one cause.** `prepare-all` tries every fixture and names each
that cannot be had, with its image, in one error (`fixture image <name> (<ref>) unavailable: …`,
or `could not be built from … after 3 tries`, which says a package mirror could not be reached
only when that is what apk or apt printed). Then **all four machines and both suites are skipped**,
not only what needs that fixture, and the job is red on that one error. The fault suite and the
behaviour suite run after a failed test step as before, but only on machines that came up, so a
missing machine never shows as its tests failing. `PURLIS_PARITY` still makes a missing fixture
fail a test that runs.

**Why the gate is one step and not one per fixture** (#8009). The job is red either way, and its
cause is the one error naming the fixture. Gating each machine on its own image would run what
remains, but the fault suite needs the pool's machine and the systemd machine together, rootless
Podman's machine signs in with the pool machine's key, and the usual cause, a registry or mirror
that does not answer, takes every fixture at once: four gates would add four steps to read for a
partial run that is red anyway.
