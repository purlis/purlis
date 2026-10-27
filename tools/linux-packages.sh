#!/usr/bin/env bash
# Copyright 2026 Purlis
# SPDX-License-Identifier: Apache-2.0

# The Linux jobs' package install, bounded and cacheable (#1477).
#
#   tools/linux-packages.sh key [--tauri] PACKAGE...
#   tools/linux-packages.sh install [--cache DIR [--refresh]] [--tauri] PACKAGE...
#
# `--tauri` adds Tauri's Linux build dependencies, which this file names once for every job.
#
# `key` writes `key=...` to $GITHUB_OUTPUT: the package list, in any order, then the runner
# image. The image is in it because the .deb files a run downloads are the ones that image
# lacked. It also writes `prefix=...`, the key without the image, so a run on a new image
# restores the newest cache of the same list from an older image (#3547): runner images roll out
# every week or so, over days, and until `main` had saved a cache on the new one every run asked
# the mirror. Those files mostly still fit; the ones that do not are found by the install below.
# And `previous=...`, the same cache under its name before the image moved to the end (v1).
#
# `install` installs PACKAGE... and says in the job summary how long it took and from where.
#   - With `--cache DIR` holding .deb files (`.github/actions/linux-packages` restores them
#     there), it puts them in apt's archives directory and installs them from there with
#     `--no-download`, so a slow or broken mirror is never asked. If they do not fit, it says so,
#     quoting apt-get, and goes on to the mirror; the files stay in apt's archives directory, so
#     the mirror is asked only for what changed.
#   - From the mirror, `apt-get update` and `apt-get install` each run under a limit of their
#     own (UPDATE_SECONDS, INSTALL_SECONDS), and a failed or stuck attempt is tried once more.
#     On 2026-10-07 the mirror took 1,285 s for an install that takes under 45 s, and a run
#     sat for over an hour; now the step fails in minutes and says why.
#   - With `--refresh` as well (the action passes it on `main` when this image's key missed), the
#     cached files go into apt's archives directory but are not installed as they are: the
#     install asks the mirror, which downloads only the versions that changed and checks every
#     file against the signed index, and the cache saved after holds this image's versions. So a
#     set restored from an older image is renewed on each image's first `main` run, and no
#     cached .deb outlives one image (#3547).
#   - On Ubuntu, the second attempt asks another mirror (#3547): the distribution's own
#     (archive.ubuntu.com and security.ubuntu.com, or ports.ubuntu.com off x86), and those
#     alone, rather than the image's mirror, which is the one that was slow.
#   - With `--cache DIR`, after an install from the mirror it replaces what DIR holds with the
#     .deb files of exactly the versions now installed, and writes `save=true` to $GITHUB_OUTPUT.
#   - Either way, a notice on the run says where the packages came from and how long it took,
#     so a run's annotations say it without its log.
#
# Seams for tools/linux-packages.test.mjs: LINUX_PACKAGES_SUDO (default `sudo`),
# LINUX_PACKAGES_ARCHIVES (default /var/cache/apt/archives), LINUX_PACKAGES_PAUSE (seconds
# between attempts, default 10), LINUX_PACKAGES_OS_RELEASE (default /etc/os-release).
set -euo pipefail

TAURI_PACKAGES="libwebkit2gtk-4.1-dev libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev"
# A normal install is under 45 s in all. Two attempts at the most, plus the try from the cache,
# stay under the 9 minutes the calling step allows, so this script's message is what is read
# and not the runner's "exceeded the maximum execution time": with each limit's 10 s for a
# process that ignores the first signal and the pause between attempts, the worst case is
# (60+10) + 2 x ((75+10) + (120+10)) + 10 = 510 s, under the step's 540.
UPDATE_SECONDS=75
INSTALL_SECONDS=120
OFFLINE_SECONDS=60
ATTEMPTS=2

SUDO=${LINUX_PACKAGES_SUDO-sudo}
ARCHIVES=${LINUX_PACKAGES_ARCHIVES:-/var/cache/apt/archives}
PAUSE=${LINUX_PACKAGES_PAUSE:-10}
OS_RELEASE=${LINUX_PACKAGES_OS_RELEASE:-/etc/os-release}
OUTPUT=${GITHUB_OUTPUT:-/dev/null}
SUMMARY=${GITHUB_STEP_SUMMARY:-/dev/null}

# Every apt-get: retries per download, a stalled connection given up on, and a wait for the
# dpkg lock an image's own unattended upgrade may hold, rather than failing at once.
APT_OPTIONS="-y -q -o Acquire::Retries=3 -o Acquire::http::Timeout=30 -o Acquire::https::Timeout=30 -o DPkg::Lock::Timeout=60"

usage() {
  echo "usage: $0 key [--tauri] PACKAGE... | $0 install [--cache DIR [--refresh]] [--tauri] PACKAGE..." >&2
  exit 2
}

[ $# -gt 0 ] || usage
mode=$1
shift
case "$mode" in key | install) ;; *) usage ;; esac

cache=""
refresh=false
packages=""
while [ $# -gt 0 ]; do
  case "$1" in
    --cache)
      [ $# -gt 1 ] || usage
      cache=$2
      shift 2
      ;;
    --refresh)
      refresh=true
      shift
      ;;
    --tauri)
      packages="$packages $TAURI_PACKAGES"
      shift
      ;;
    -*) usage ;;
    *)
      packages="$packages $1"
      shift
      ;;
  esac
done
# One name per line, sorted, each once: the list the key is made of and apt-get is given.
# shellcheck disable=SC2086 # split into its names on purpose.
packages=$(printf '%s\n' $packages | sort -u | sed '/^$/d')
[ -n "$packages" ] || usage

if [ "$mode" = key ]; then
  if command -v sha256sum > /dev/null; then
    hash=$(printf '%s\n' "$packages" | sha256sum | cut -c1-16)
  else
    hash=$(printf '%s\n' "$packages" | shasum -a 256 | cut -c1-16)
  fi
  os=${ImageOS:-linux}
  image=${ImageVersion:-unknown}
  prefix="linux-packages-v2-$os-$(uname -m)-$hash-"
  {
    echo "key=$prefix$image"
    echo "prefix=$prefix"
    echo "previous=linux-packages-v1-$os-$image-$(uname -m)-$hash"
  } >> "$OUTPUT"
  exit 0
fi

count=$(printf '%s\n' "$packages" | wc -l | tr -d ' ')
start=$(date +%s)

# The job summary, and for an install that worked a notice on the run, which the run's
# annotations carry (a failure is its ::error line already).
say() {
  printf '### Linux packages\n\n%s\n' "$1" >> "$SUMMARY"
}
done_saying() {
  say "$1"
  echo "::notice title=Linux packages::$1"
}

# The distribution's own mirror, for a second attempt when the image's was slow or failed: its
# release, updates and security fixes, signed by its archive key, and nothing else. Written to a
# file of its own and named with `-o`, so the image's sources are left as they are. Ubuntu only:
# elsewhere there is no other mirror to name, and the second attempt asks the same sources.
#
# Sets `mirror` and `sources` (the `-o` options that name the file) when it can, or leaves them
# empty.
mirror=""
sources=()
other_mirror() {
  local codename dir security signed components
  # shellcheck source=/dev/null # the distribution's file, read in a subshell.
  codename=$( (. "$OS_RELEASE" 2> /dev/null && [ "${ID:-}" = ubuntu ] && echo "${VERSION_CODENAME:-}") || true)
  [ -n "$codename" ] || return 0
  dir=$(mktemp -d "${RUNNER_TEMP:-${TMPDIR:-/tmp}}/linux-packages.XXXXXX" 2> /dev/null) || return 0
  [ -n "$dir" ] && [ -d "$dir" ] || return 0
  case "$(uname -m)" in
    x86_64 | i?86)
      mirror=http://archive.ubuntu.com/ubuntu
      security=http://security.ubuntu.com/ubuntu
      ;;
    *)
      mirror=http://ports.ubuntu.com/ubuntu-ports
      security=$mirror
      ;;
  esac
  signed="[signed-by=/usr/share/keyrings/ubuntu-archive-keyring.gpg]"
  components="main restricted universe multiverse"
  {
    echo "deb $signed $mirror $codename $components"
    echo "deb $signed $mirror $codename-updates $components"
    echo "deb $signed $security $codename-security $components"
  } > "$dir/sources.list"
  # An empty parts directory, so the image's own files there are not read.
  mkdir "$dir/parts"
  # And the image's own lists are kept, so a later `apt-get install` in the job still finds them.
  sources=(-o "Dir::Etc::SourceList=$dir/sources.list" -o "Dir::Etc::SourceParts=$dir/parts"
    -o APT::Get::List-Cleanup=0)
}

elapsed() {
  echo $(($(date +%s) - start))
}

# The DEBIAN_FRONTEND goes through `env` so `sudo` keeps it.
apt() {
  local seconds=$1
  shift
  # shellcheck disable=SC2086 # APT_OPTIONS is a list of words on purpose.
  $SUDO timeout -k 10 "$seconds" env DEBIAN_FRONTEND=noninteractive apt-get $APT_OPTIONS "$@"
}

# From the cache: the files themselves, and nothing downloaded.
#
# They are copied into apt's archives directory first and named there. apt takes a .deb named on
# its command line as already downloaded only when it finds the file in that directory; from any
# other path it queues the file as a download like any other, `--no-download` drops it, and the
# install fails with "Unable to fetch some archives". Every warm run failed so until 2026-10-10.
if [ -n "$cache" ] && ls "$cache"/*.deb > /dev/null 2>&1; then
  $SUDO cp "$cache"/*.deb "$ARCHIVES"/
fi
if [ "$refresh" = false ] && [ -n "$cache" ] && ls "$cache"/*.deb > /dev/null 2>&1; then
  debs=()
  for deb in "$cache"/*.deb; do
    debs+=("$ARCHIVES/$(basename "$deb")")
  done
  offline=0
  said=$(apt "$OFFLINE_SECONDS" install --no-download "${debs[@]}" 2>&1) || offline=$?
  printf '%s\n' "$said"
  if [ "$offline" -eq 0 ]; then
    done_saying "$count packages installed in $(elapsed) s from the cache, without asking the mirror."
    exit 0
  fi
  why=$(grep -E '^(E|W):' <<< "$said" | head -n 4 | tr '\n' ' ' || true)
  echo "::warning title=Linux packages::the cached .deb files do not install on this image; installing from the mirror instead. apt-get said: ${why:-nothing on its error lines}"
fi

# From the mirror, twice at the most: the image's, then the distribution's own when there is one.
attempt=1
while :; do
  status=0
  # shellcheck disable=SC2086 # one package name per word.
  apt "$UPDATE_SECONDS" ${sources[@]+"${sources[@]}"} update &&
    apt "$INSTALL_SECONDS" ${sources[@]+"${sources[@]}"} install $packages || status=$?
  if [ "$status" -eq 0 ]; then
    break
  fi
  if [ "$status" -eq 124 ]; then
    why="did not finish in time"
  else
    why="failed (exit $status)"
  fi
  if [ "$attempt" -ge "$ATTEMPTS" ]; then
    if [ -n "$mirror" ]; then
      down="the image's mirror and $mirror are slow or down"
    else
      down="the distribution's mirror is slow or down"
    fi
    echo "::error title=Linux packages::apt-get $why on attempt $attempt of $ATTEMPTS${mirror:+, the last from $mirror}: $down, not the code under test. Re-run the job later."
    say "The install failed after $(elapsed) s: apt-get $why on each of $ATTEMPTS attempts${mirror:+, the last from $mirror}, so $down."
    exit 1
  fi
  other_mirror
  echo "::warning title=Linux packages::apt-get $why on attempt $attempt of $ATTEMPTS; trying once more${mirror:+ from $mirror}"
  attempt=$((attempt + 1))
  sleep "$PAUSE"
done

note=""
if [ -n "$cache" ]; then
  # Only the files of the versions now installed, so a file apt left from something else
  # never rides along into the cache.
  installed=$(dpkg-query -W -f='${Package}_${Version}_${Architecture}\n')
  # And only those: a restored file that did not fit this image goes, or the next run would name
  # it again beside the version that replaced it.
  mkdir -p "$cache"
  rm -f "$cache"/*.deb
  kept=0
  for deb in "$ARCHIVES"/*.deb; do
    [ -e "$deb" ] || continue
    # apt writes an epoch's colon as %3a in the file name.
    name=$(basename "$deb" .deb | sed 's/%3a/:/g')
    # A here-string and not a pipe: under `pipefail`, `grep -q` leaving at its first match can
    # end the writer with SIGPIPE once the list outgrows the pipe's buffer, and a runner image
    # lists thousands of packages, so the match would read as a miss.
    if grep -qxF "$name" <<< "$installed"; then
      cp "$deb" "$cache/"
      kept=$((kept + 1))
    fi
  done
  if [ "$kept" -gt 0 ]; then
    echo "save=true" >> "$OUTPUT"
    note=" $kept .deb files kept for the cache."
  else
    note=" No .deb files were left in $ARCHIVES to keep, so the next run asks the mirror too."
  fi
fi
done_saying "$count packages installed in $(elapsed) s from the mirror, on attempt $attempt of $ATTEMPTS${mirror:+, from $mirror}.$note"
