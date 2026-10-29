# Copyright 2026 Purlis
# SPDX-License-Identifier: Apache-2.0

# The runner image the parity suite runs against a Docker pool (#2045, spec #2041): purlis's own
# runner image of this build (`runner-image/Dockerfile`, its `purlis` the one this job built), with
# the repo's fake harness and `purlis` added at the folder this job's build keeps them in. A
# behaviour names its chat's program by that path (`app/src-tauri/src/behaviour.rs`), so the same
# behaviour runs unchanged on this machine and on the runner. Built and used by the `parity` job of
# `.github/workflows/ci.yml` alone, and pushed nowhere but a registry of that job's own.

# The runner image of this build, as the job tagged it.
ARG RUNNER
FROM ${RUNNER}

# The absolute folder this job's test build keeps its binaries in (`target/debug`).
ARG AT
COPY --chmod=0755 fake-harness purlis ${AT}/

# The runner's `claude`, which a profile of the runner's own runs. Asked `auth status`, it exits
# 0, which is how Claude Code's own says a profile is signed in, so the test's profile passes the
# sign-in check without a sign-in. Anything else runs the stand-in a behaviour gives, written into
# the runner's home (`/home/purlis/.parity/claude`) as the behaviour starts, since the image's own
# folders are read-only in a pool's container; where no behaviour wrote one, it says so and fails.
USER 0
RUN printf '%s\n' '#!/bin/sh' \
      '[ "$1" = auth ] && [ "$2" = status ] && exit 0' \
      'stand_in=/home/purlis/.parity/claude' \
      '[ -x "$stand_in" ] || { echo "parity: no stand-in claude at $stand_in: a behaviour writes one before it starts a chat on its profile" >&2; exit 127; }' \
      'exec "$stand_in" "$@"' > /usr/local/bin/claude \
 && chmod 0755 /usr/local/bin/claude
# The runner's `codex`, the same way (#3338): a Codex chat the runner composes runs under
# purlis's own Linux sandbox, so a behaviour sees from inside it what a sandboxed runner chat
# can reach. Asked `login status`, it exits 0, as Codex's own says a profile is signed in.
# Anything else runs the stand-in a behaviour writes at `/home/purlis/.parity/codex`.
RUN printf '%s\n' '#!/bin/sh' \
      '[ "$1" = login ] && [ "$2" = status ] && exit 0' \
      'stand_in=/home/purlis/.parity/codex' \
      '[ -x "$stand_in" ] || { echo "parity: no stand-in codex at $stand_in: a behaviour writes one before it starts a chat on its profile" >&2; exit 127; }' \
      'exec "$stand_in" "$@"' > /usr/local/bin/codex \
 && chmod 0755 /usr/local/bin/codex
USER 10001:10001
