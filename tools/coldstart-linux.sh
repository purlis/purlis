#!/usr/bin/env bash
# Copyright 2026 Purlis
# SPDX-License-Identifier: Apache-2.0

# Linux cold start against the spec's 2 s limit, on the desktops charter-app#24 is about.
# After `npx tauri build --debug --no-bundle`, from anywhere:
#
#   tools/coldstart-linux.sh bare     # X with no window manager, on the session bus as found
#   tools/coldstart-linux.sh i3       # i3 under X11, on the session bus as found
#   tools/coldstart-linux.sh i3-nobus # i3 under X11 with no session bus, as `startx` into i3
#   tools/coldstart-linux.sh restore  # X with no window manager: a launch that puts 1, 10 and
#                                     # 40 chats back (row L8), evidence only, never a gate
#
# On a GitHub runner "the session bus as found" is charter-app#24's machine: a bus that is up,
# with a desktop portal it can activate and that cannot start.
#
# **Against main** (a pull request, #1866): with `BASELINE_APP`, main's build of the app made in
# the same job, each case launches each build once, discarded, then seven rounds of one launch of
# each in alternating order, each on a fresh HOME and XDG directories. It fails only when this
# build is past 1.15 times main's (the median of the rounds' ratios) in a pass and in a second pass
# that confirms it. The 2 s limit is reported, not gated: with the launch unchanged the Ubuntu
# runners' medians went from 874 to 2537 ms, while main and the change launched in turn on one
# runner share its speed (ADR 0086 as amended for #1866; `tools/coldstart-gate.mjs`).
#
# **Without a baseline** (a push to main, a nightly, or a pull request whose main did not build),
# each case launches the app once, discarded, and then five times, each on a fresh HOME and XDG
# directories. It fails when the median of the five is past the 2 s limit, or two or more of them
# are past a 2.5 s ceiling; one launch past it is reported, not gated (V39, ADR 0086 as amended;
# `tools/coldstart-gate.mjs`). With `EVIDENCE_ONLY=1` a miss is a warning and the case passes:
# the pull request already held the same code to main. With `NO_BASELINE` (a pull request whose
# main did not build, saying why) the same holds and the warning says nothing was gated: the
# runners missed the limit in a few percent of cases on unchanged launch code (#2388). The
# limit's shape is not "every one under 2 s": this is a debug build on a
# shared runner, where one launch in five went 63 ms over (main, run 36789430687) while the
# others sat near 1.35 s. A release build would be the one users run, but it costs a full
# `lto = true`, `codegen-units = 1` build — 6.5 min on this runner — where the debug build CI
# already makes takes about 1 min. Measured on PR #753: a person's first launch pays two costs
# that are not charter's.
#   - Once per user profile, Mesa compiles WebKit's shaders into ~/.cache/mesa_shader_cache
#     (about 0.45 s in a container, about 1 s on a 2-core runner). A fresh profile per launch
#     makes every held launch pay it, as a new user's first launch does.
#   - Once per boot or install, the binary and GTK/WebKitGTK's libraries come off a cold disk
#     (about 2.3 s on the runner, for a 500 MB debug binary). The discarded launch pays it, and
#     its time is printed as "cold disk, once per boot or install: reported, not gated".
# On CI each case also leaves one annotation on the run (a diagnostic, `--launch-steps`): every
# launch's time, the part before the app's own clock and the runner's load, and one more launch
# after the held ones, not gated, with the app's launch steps (`PURLIS_LAUNCH_LOG`), so a red run
# says where its time went without anyone reading the log.
# CI measures the debug build `npx tauri build --debug` makes; a release build is smaller, so
# its cold-disk cost is likely smaller too.
#
# **No measurement** (#1876). A case whose launches could not be TAKEN has no number for a gate
# to judge: the X server did not come up, i3 did not, a window never appeared, or main's build
# left before its first frame. Each leaves one annotation on the run that names the cause and the
# case, and the step's summary says "no measurement". With `EVIDENCE_ONLY=1` it is a warning and
# the case passes; without, an error and it fails. What is not the runner's still fails either
# way, with an annotation of its own: an unknown case, a build that is not where the job put it,
# and this change's own build leaving before its first frame (`tools/coldstart-gate.mjs`).
set -euo pipefail

case="${1:-}"

# One annotation on the run (a workflow command), its title and text escaped.
annotate() {
  local level="$1" title="$2" text="$3"
  title="${title//%/%25}"
  title="${title//:/%3A}"
  title="${title//,/%2C}"
  text="${text//%/%25}"
  text="${text//$'\r'/%0D}"
  text="${text//$'\n'/%0A}"
  echo "::$level title=$title::$text"
}

# The script itself cannot run: never softened.
broken() {
  echo "coldstart-linux.sh: $1" >&2
  annotate error "Cold start, ${case:-no case}: the script could not run" "$1"
  exit "${2:-1}"
}

# The case's launches could not be taken: the cause, then what the program that failed said.
no_measurement() {
  local cause="$1" detail="${2:-}" title="Cold start, $case: no measurement"
  echo "coldstart-linux.sh: $cause; nothing was measured" >&2
  if [[ -n "$detail" ]]; then
    echo "$detail" >&2
  fi
  if [[ -n "${GITHUB_STEP_SUMMARY:-}" ]]; then
    printf '### %s\n\n%s\n\n' "$title" "$cause" >>"$GITHUB_STEP_SUMMARY"
  fi
  if [[ "${EVIDENCE_ONLY:-}" == 1 || (-z "${BASELINE_APP:-}" && -n "${NO_BASELINE:-}") ]]; then
    annotate warning "$title" \
      "$cause; reported, and the run does not fail for it (evidence only)${detail:+$'\n'$detail}"
    exit 0
  fi
  annotate error "$title" "$cause${detail:+$'\n'$detail}"
  exit 1
}

case "$case" in
bare | i3 | i3-nobus | restore) ;;
"") broken "which case: bare, i3, i3-nobus or restore" 2 ;;
*) broken "unknown case: $case" 2 ;;
esac

# `APP` as the caller named it, then the repository root for everything else.
app="$(realpath -m "${APP:-$(dirname "$0")/../target/debug/charter-app}")"
baseline="${BASELINE_APP:+$(realpath -m "$BASELINE_APP")}"
cd "$(dirname "$0")/.."
[[ -x "$app" ]] || broken "the app to launch is not at $app: the build before this step makes it"
[[ -z "$baseline" || -x "$baseline" ]] ||
  broken "main's app is not at $baseline: the step that built it said it was"
limit="${LIMIT_MS:-2000}"
ceiling="${CEILING_MS:-2500}"
# Every launch is a person's first, with a theme of theirs that brings its fonts (#2326): the
# window draws as a person with a theme folder's fonts does, and the gate holds that.
bench=(node tools/bench.mjs --skip-build --only coldstart --app "$app" --limit "$limit"
  --ceiling "$ceiling" --warm-up --fresh-profile --theme-fonts --launch-steps --case "$case")
if [[ -n "$baseline" ]]; then
  bench+=(--baseline-app "$baseline" --cold-starts 7)
else
  bench+=(--cold-starts 5)
  # A pull request whose main did not build (#2388): why, for the bench to report the limit
  # and gate nothing. A baseline, where there is one, is held to whatever this says.
  if [[ -n "${NO_BASELINE:-}" ]]; then
    bench+=(--no-baseline "$NO_BASELINE")
  fi
fi
if [[ "${EVIDENCE_ONLY:-}" == 1 ]]; then
  bench+=(--evidence)
fi

if [[ -z "${DISPLAY:-}" ]]; then
  # A display of its own for the run, and this script again inside it. `xvfb-run` starts the
  # command only once Xvfb has said it accepts connections, so nothing here waits for the
  # display. When this script never ran inside, the X server is what did not come up, and
  # `xvfb-run` says only "exit 1": the mark tells the two apart, and Xvfb's own words are kept.
  outer="$(mktemp -d)"
  inside="$outer/inside"
  xlog="$outer/xvfb.log"
  status=0
  APP="$app" BASELINE_APP="$baseline" COLDSTART_INSIDE="$inside" \
    xvfb-run -a -e "$xlog" -s "-screen 0 1280x800x24" tools/coldstart-linux.sh "$@" || status=$?
  ran="$(cat "$inside" 2>/dev/null || true)"
  said="$(tail -n 12 "$xlog" 2>/dev/null || true)"
  rm -rf "$outer"
  if [[ -n "$ran" ]]; then
    exit "$status"
  fi
  no_measurement "the X server did not come up: xvfb-run ended with $status before the case ran in it" "$said"
fi
if [[ -n "${COLDSTART_INSIDE:-}" ]]; then
  echo ran >"$COLDSTART_INSIDE"
fi

# What this run made, and the window manager it started: gone however the run ends.
scratch="$(mktemp -d)"
wm=""
nobus=""
# The run's verdict is the script's: the cleanup keeps the status it was called with, and what it
# cannot remove is said, never failed on.
cleanup() {
  local status=$?
  if [[ -n "$wm" ]]; then
    kill "$wm" 2>/dev/null || true
    wait "$wm" 2>/dev/null || true
  fi
  # In the no-bus case the app starts on the X display's bus, whose document portal mounts FUSE
  # at $XDG_RUNTIME_DIR/doc — a directory `rm` cannot remove while it is mounted (#746, CI).
  local doc="$scratch/runtime/doc"
  if [[ -d "$doc" ]]; then
    fusermount3 -uz "$doc" 2>/dev/null || fusermount -uz "$doc" 2>/dev/null || true
  fi
  if [[ -n "$nobus" ]]; then
    the_x_displays_bus_ends
  fi
  rm -rf "$scratch" 2>/dev/null || echo "coldstart-linux.sh: could not remove $scratch" >&2
  exit "$status"
}
trap cleanup EXIT

# The bus `dbus-launch --autolaunch` started for the X display in the no-bus case, and the
# portals the app woke on it, ended by the process ids the bus itself gives — never by name.
# Asking `dbus-launch` again hands back the bus the display holds, with its daemon's pid.
the_x_displays_bus_ends() {
  local machine printed address daemon name pid
  machine="$(cat /etc/machine-id 2>/dev/null || true)"
  [[ -n "$machine" ]] || machine="$(cat /var/lib/dbus/machine-id 2>/dev/null || true)"
  [[ -n "$machine" ]] || return 0
  printed="$(dbus-launch --autolaunch="$machine" --sh-syntax --close-stderr 2>/dev/null)" || return 0
  address="$(sed -n "s/^DBUS_SESSION_BUS_ADDRESS='\(.*\)';$/\1/p" <<<"$printed")"
  daemon="$(sed -n 's/^DBUS_SESSION_BUS_PID=\([0-9]*\);$/\1/p' <<<"$printed")"
  [[ -n "$address" && -n "$daemon" ]] || return 0
  for name in org.freedesktop.portal.Documents org.freedesktop.portal.Desktop \
    org.freedesktop.impl.portal.desktop.gtk; do
    pid="$(dbus-send --bus="$address" --print-reply --dest=org.freedesktop.DBus \
      /org/freedesktop/DBus org.freedesktop.DBus.GetConnectionUnixProcessID "string:$name" \
      2>/dev/null | awk '/uint32/ { print $2 }')" || true
    if [[ -n "$pid" ]]; then
      kill "$pid" 2>/dev/null || true
    fi
  done
  kill "$daemon" 2>/dev/null || true
}

# How many times i3 is started, and how long each start may take to manage the display.
i3_starts=3
i3_tenths="${I3_WAIT_TENTHS:-50}"

with_i3() {
  local config="$scratch/i3.config"
  # i3's packaged config starts its first-run wizard; one line is all it needs.
  printf 'font pango:monospace 8\n' >"$config"
  # i3 owns the root window once it is managing it, and only then names its IPC socket on it;
  # nothing is measured before that, and a case that says i3 and measured no i3 would be a green
  # run about the wrong desktop. A start that leaves at once is started again, a bounded number
  # of times: twice on 2026-10-13 this step ended within a second of the case before it (#1876).
  local start up=""
  for start in $(seq "$i3_starts"); do
    # Whatever made a start leave is given a moment to pass before the next.
    [[ "$start" == 1 ]] || sleep "${I3_RESTART_PAUSE:-0.5}"
    echo "--- start $start" >>"$scratch/i3.log"
    i3 -c "$config" >>"$scratch/i3.log" 2>&1 &
    wm=$!
    for _ in $(seq "$i3_tenths"); do
      kill -0 "$wm" 2>/dev/null || break
      if i3 --get-socketpath >/dev/null 2>&1; then
        up=yes
        break 2
      fi
      sleep 0.1
    done
    kill "$wm" 2>/dev/null || true
    wait "$wm" 2>/dev/null || true
    wm=""
  done
  if [[ -z "$up" ]]; then
    no_measurement \
      "i3 did not come up in $i3_starts starts (each left, or took more than $((i3_tenths / 10)) s to manage the display)" \
      "$(tail -n 20 "$scratch/i3.log" 2>/dev/null || true)"
  fi
  if [[ "$start" != 1 ]]; then
    # Measured, and said: what the starts before this one printed is the cause to read.
    annotate warning "Cold start, $case: i3 came up on start $start" \
      "$(tail -n 20 "$scratch/i3.log" 2>/dev/null || true)"
  fi
  "$@"
}

case "$case" in
bare) "${bench[@]}" ;;
restore)
  # When the window could be used and when the chats had started, for each size
  # (`tools/restore-case.mjs`). Reported: it fails only where a launch could not be made at all.
  node tools/bench.mjs --skip-build --only restore --app "$app" --evidence
  ;;
i3) with_i3 "${bench[@]}" ;;
i3-nobus)
  # No address and a runtime directory with no `bus` in it: the standard lookup finds nothing.
  # GIO would autolaunch a bus on the X display, and the app now looks for that one the same
  # way and starts on it (`portal.rs`).
  mkdir "$scratch/runtime"
  chmod 700 "$scratch/runtime"
  nobus=yes
  with_i3 env -u DBUS_SESSION_BUS_ADDRESS XDG_RUNTIME_DIR="$scratch/runtime" "${bench[@]}"
  ;;
esac
