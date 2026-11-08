#!/usr/bin/env bash
# Fetches the packages runner-image/Dockerfile's `forge-programs-<arch>` stage pins, into <dir>,
# each checked against the stage's own sum, for
#   docker build --build-context forge-programs-<arch>=<dir> ...
# A build given that context does not run the stage's ADDs. The Dockerfile stays the one place a
# URL and its sum are written; this reads them from it.
#
# Why: from 2026-11-08 09:11Z, Actions runners fetching gh's release URL were served other bytes
# (sha256 bef140a1…) than the release lists and the API serves (e3bafef9…), so every build of
# the image failed its ADD. A package on github.com comes through the API, with the job's token.
# Usage: runner-image/fetch-forge-programs.sh <amd64|arm64> <dir>   (needs GH_TOKEN)
set -euo pipefail
arch=${1:?arch}
dir=${2:?dir}
here=$(cd "$(dirname "$0")" && pwd)
mkdir -p "$dir"
# Each ADD of the stage as "<sum> <url> <dest>", its continuation lines joined.
pins=$(awk -v stage="FROM scratch AS forge-programs-$arch" '
  $0 == stage { on = 1; next }
  /^FROM/ { on = 0 }
  on { line = line " " $0; if ($0 !~ /\\$/) { print line; line = "" } }
' "$here/Dockerfile" | tr -d '\\' | sed -nE 's/^ *ADD +--checksum=sha256:([0-9a-f]{64}) +([^ ]+) +\/([^ ]+) *$/\1 \2 \3/p')
[ "$(printf '%s\n' "$pins" | grep -c .)" -ge 2 ] \
  || { echo "::error file=runner-image/Dockerfile::read no ADDs from forge-programs-$arch"; exit 1; }
while read -r sum url dest; do
  for try in 1 2 3 4; do
    if [[ "$url" =~ ^https://github\.com/([^/]+/[^/]+)/releases/download/([^/]+)/([^/]+)$ ]]; then
      gh release download "${BASH_REMATCH[2]}" --repo "${BASH_REMATCH[1]}" \
        --pattern "${BASH_REMATCH[3]}" --output "$dir/$dest" --clobber || true
    else
      curl --fail --silent --show-error --location --retry 3 --output "$dir/$dest" "$url" || true
    fi
    got=$(sha256sum "$dir/$dest" 2>/dev/null | cut -d' ' -f1 || true)
    [ "$got" = "$sum" ] && break
    [ "$try" = 4 ] && { echo "::error::$url is ${got:-missing}, the Dockerfile pins $sum"; exit 1; }
    echo "::warning::try $try of 4: $url is ${got:-missing}, the Dockerfile pins $sum"
    sleep $((try * 15))
  done
  echo "$dest $sum"
done <<< "$pins"
