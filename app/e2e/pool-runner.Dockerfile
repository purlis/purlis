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

# The Claude Code version purlis supports (`tests/fixtures/harness-versions/claude.txt`).
ARG VERSION

# The runner's `claude`. Asked `auth status`, it exits 0, as Claude Code's own does for a profile
# signed in, so the profile Pool setup keeps here is signed in with no account. Asked its version,
# it gives the one purlis supports. Started as a chat, it reports its session's start as Claude
# Code's `SessionStart` hook does: the session id purlis started it under, through the runner's
# `purlis hook`, from its own process. Then it waits, as a chat waits for its first prompt. No
# model is called. A behaviour of the parity suite runs the same image with stand-ins of its own
# (`tests/parity/runner.Dockerfile`); this one needs none, since the window never reaches inside.
USER 0
RUN printf '%s\n' '#!/bin/sh' \
      '[ "$1" = auth ] && [ "$2" = status ] && exit 0' \
      "for word in \"\$@\"; do [ \"\$word\" = --version ] && { echo '${VERSION}'; exit 0; }; done" \
      'id=' \
      'while [ $# -gt 0 ]; do [ "$1" = --session-id ] && id=$2; shift; done' \
      'printf '"'"'{"session_id":"%s","hook_event_name":"SessionStart","source":"startup"}'"'"' "$id" \' \
      '  | CLAUDE_PID=$$ PURLIS_HARNESS=claude /usr/local/bin/purlis hook sessionstart >/dev/null' \
      'exec sleep 3600' > /usr/local/bin/claude \
 && chmod 0755 /usr/local/bin/claude
USER 10001:10001
