# Copyright 2026 Purlis
# SPDX-License-Identifier: Apache-2.0

# purlis's runner image (#2043): `purlis serve`, the session host, on a machine with no window.
# What any runner image has to hold is docs/runner-image.md; this is purlis's own, and
# `tools/runner-image-contract.mjs` checks it against that contract before it is pushed.
#
# Built by `.github/workflows/runner-image.yml`, once per architecture on a runner of that
# architecture, from a build context holding the `purlis` binary that job compiled with the
# repository's pinned toolchain, and purlis's plugin (`app/src-tauri/plugin`) as the commit has
# it. Nothing else of the repository is in the context, so nothing else can reach the image.
#
# The base is Ubuntu 26.04 LTS. The binary is compiled on Ubuntu 24.04 and needs glibc 2.39 or
# later; a newer glibc runs what an older one linked. The base is pinned by the digest of its
# multi-architecture index; Dependabot proposes the next one. The packages are the archive's
# current ones at build time, so every build of `main` carries the security fixes of that day.
FROM ubuntu:26.04@sha256:c098726dd9b9733f3e885f8de0ef375206a311b0a317eadc40d347b1115fb944

# git: a chat's clone and branches. ca-certificates: every TLS connection a chat makes.
# tini: PID 1, which reaps what a chat's programs leave behind when the host is not there to.
# The rest is what the pinned browser engine needs to run (#3298): the libraries it links (its
# own `deb.deps`, measured on this base 2026-10-28) and one font family, so a page's text is
# drawn. The engine itself is not in the image: the runner's host downloads it, checked, on its
# first browser, as on any machine.
# Ubuntu's own `ubuntu` user (1000) is removed, so the image has one user who is not root.
RUN apt-get update \
 && apt-get install --yes --no-install-recommends ca-certificates git tini \
      fonts-liberation libasound2t64 libatk-bridge2.0-0t64 libatk1.0-0t64 libatspi2.0-0t64 \
      libcairo2 libcups2t64 libdbus-1-3 libdrm2 libexpat1 libgbm1 libglib2.0-0t64 libnspr4 \
      libnss3 libpango-1.0-0 libudev1 libx11-6 libxcb1 libxcomposite1 libxdamage1 libxext6 \
      libxfixes3 libxkbcommon0 libxrandr2 \
 && rm -rf /var/lib/apt/lists/* \
 && userdel --remove ubuntu \
 && groupadd --gid 10001 purlis \
 && useradd --uid 10001 --gid 10001 --create-home --home-dir /home/purlis --shell /bin/bash purlis \
 && chmod 0700 /home/purlis

COPY --chmod=0755 purlis /usr/local/bin/purlis
# purlis's plugin, beside the binary, where `purlis serve` finds it: a sandboxed Claude Code
# chat is armed with it (#2447). Read-only to the image's user.
COPY plugin /usr/local/lib/purlis/plugin

USER 10001:10001
ENV HOME=/home/purlis
WORKDIR /home/purlis

ENTRYPOINT ["/usr/bin/tini", "--", "/usr/local/bin/purlis"]
CMD ["serve"]
