# Copyright 2026 Purlis
# SPDX-License-Identifier: Apache-2.0

# The runner image Pool setup's scenario run makes its pool of (#4985, `wdio.pool.conf.ts`):
# purlis's own runner image of this build (`runner-image/Dockerfile`, its `purlis` the one this job
# built), with a `claude` that stands in for Claude Code. Built and used by the pool leg of
# `scenario-linux` in `.github/workflows/ci.yml` alone, and pushed nowhere but a registry of that
# job's own.

# The runner image of this build, as the job tagged it.
ARG RUNNER
FROM ${RUNNER}

# The Claude Code version purlis supports, as `claude --version` answers it
# (`tests/fixtures/harness-versions/claude.txt`): `2.1.300 (Claude Code)`.
ARG VERSION

# The Claude Code the profile pins, where a runner keeps a pinned harness it has installed
# (`purlis_core::profilestore::installs`, `<data>/harnesses/claude/<version>/claude`): a runner
# image carries no harness on `PATH` (`docs/runner-image.md`, and Pool setup's check of a team's
# image refuses one that does). The runner's home is a volume, which Docker fills from the image
# the first time it is mounted, so the runner finds this version installed and fetches nothing.
#
# Asked `auth status`, it exits 0, as Claude Code's own does for a profile signed in, so the
# profile Pool setup keeps here is signed in with no account. Asked its version, it gives the one
# purlis supports. Started as a chat, it reports its session's start as Claude Code's
# `SessionStart` hook does: the session id purlis started it under, through the runner's
# `purlis hook`, from its own process. Then it waits, as a chat waits for its first prompt. No
# model is called.
USER 0
RUN set -eu; \
    at="/home/purlis/.local/share/purlis/harnesses/claude/${VERSION%% *}"; \
    mkdir -p "$at"; \
    printf '%s\n' '#!/bin/sh' \
      '[ "$1" = auth ] && [ "$2" = status ] && exit 0' \
      "for word in \"\$@\"; do [ \"\$word\" = --version ] && { echo '${VERSION}'; exit 0; }; done" \
      'id=' \
      'while [ $# -gt 0 ]; do [ "$1" = --session-id ] && id=$2; shift; done' \
      'exec sleep 3600' > "$at/claude"; \
    chmod 0755 "$at/claude"; \
    chown -R 10001:10001 /home/purlis/.local
USER 10001:10001
