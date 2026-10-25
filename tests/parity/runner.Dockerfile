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

# The runner's `claude`, as a profile of the runner's own runs it: signed in, as Claude Code's
# own `auth status` answers it, and otherwise the stand-in a behaviour gives, written into the
# runner's home (`/home/purlis/.parity/claude`) as the behaviour starts, since the image's own
# folders are read-only in a pool's container.
USER 0
RUN printf '%s\n' '#!/bin/sh' \
      '[ "$1" = auth ] && [ "$2" = status ] && exit 0' \
      'exec /home/purlis/.parity/claude "$@"' > /usr/local/bin/claude \
 && chmod 0755 /usr/local/bin/claude
USER 10001:10001
