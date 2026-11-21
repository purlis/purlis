#!/usr/bin/env bash
# Spike T1.1-51 (#6898, Q295): can purlis read seatbelt's violation reports for only a chat's own
# processes, reliably and promptly, without broad log access? macOS only; it needs a runner where
# `sandbox-exec` can apply a profile (CI's macOS runner, not a sandboxed chat). Findings:
# docs/spikes/seatbelt-denial-reports.md. Usage: seatbelt-denial-reports.sh [count] (default 100).
set -u
N=${1:-100}
WORK=$(mktemp -d "${TMPDIR:-/tmp}/seatbelt-spike.XXXXXX") || { echo "mktemp failed" >&2; exit 1; }
WORK=$(cd "$WORK" && pwd -P) || { echo "cannot enter the work dir" >&2; exit 1; }
# Never let the cleanup run on an empty or foreign path: it is an rm -rf.
case "$WORK" in */seatbelt-spike.*) ;; *) echo "refusing work dir [$WORK]" >&2; exit 1 ;; esac
[ -d "$WORK" ] && [ "$WORK" != "$PWD" ] || { echo "refusing work dir [$WORK]" >&2; exit 1; }
trap 'kill $(jobs -p) 2>/dev/null; rm -rf "$WORK"' EXIT

echo "== who: $(id -un) (uid $(id -u)); groups: $(id -Gn); macOS $(sw_vers -productVersion)"
if ! sandbox-exec -p '(version 1)(allow default)' /usr/bin/true 2>/dev/null; then
  echo "== cannot apply a seatbelt profile here; nothing to measure"; exit 2
fi
if ! log stream --style compact --predicate 'eventMessage CONTAINS "x"' >"$WORK/probe" 2>&1 &
then :; fi
sleep 2; kill $! 2>/dev/null
echo "== log stream first lines: $(head -c 300 "$WORK/probe" | tr '\n' '|')"

# The sandboxed child: N refusals of distinct files, then N of one file; one stamp per attempt.
cat >"$WORK/child.pl" <<'PL'
use Time::HiRes qw(time sleep);
my ($dir, $n, $gap) = @ARGV;
for my $i (1 .. $n) { my $t = time; open(my $f, '>', "$dir/d$i") ; printf "D d$i %.6f\n", $t; sleep $gap; }
for my $i (1 .. $n) { my $t = time; open(my $f, '>', "$dir/rep") ; printf "D rep%d %.6f\n", $i, $t; sleep $gap; }
for my $i (1 .. 20) { my $t = time; open(my $f, '>', "$dir/slow") ; printf "D slow%d %.6f\n", $i, $t; sleep 0.5; }
PL
# The receiver stamps each line with its arrival time.
cat >"$WORK/stamp.pl" <<'PL'
use Time::HiRes qw(time); $| = 1;
while (<STDIN>) { printf "%.6f %s", time, $_; }
PL
# The judge: per distinct file the reports seen and the latency; repeats; strays.
cat >"$WORK/judge.pl" <<'PL'
my ($child, $recv, $dir, $n) = @ARGV;
my %at; open my $c, '<', $child or die; while (<$c>) { $at{$1} = $2 if /^D (\S+) (\S+)/ }
my (%seen, %lat, $total, $stray);
open my $r, '<', $recv or die;
while (<$r>) {
  next unless /^(\d+\.\d+) (.*)/s; my ($arr, $line) = ($1, $2);
  next unless $line =~ /deny\(\d+\)\s+file-write/ || $line =~ /\bdeny\b/;
  $total++;
  if ($line =~ m{\Q$dir\E/(d\d+|rep|slow)\b}) { my $k = $1; $seen{$k}++; push @{$lat{$k}}, $arr - ($k eq 'rep' ? $at{rep1} : $at{$k}); }
  else { $stray++ }
}
my @first; my $missing = 0;
for my $i (1 .. $n) { if ($seen{"d$i"}) { my ($min) = sort { $a <=> $b } @{$lat{"d$i"}}; push @first, $min } else { $missing++ } }
@first = sort { $a <=> $b } @first;
my $med = @first ? $first[int(@first / 2)] : -1; my $p95 = @first ? $first[int(@first * 0.95)] : -1; my $max = @first ? $first[-1] : -1;
my $dups = 0; for my $i (1 .. $n) { $dups += $seen{"d$i"} - 1 if ($seen{"d$i"} // 0) > 1 }
printf "  distinct refusals: %d; reported: %d; missing: %d; extra copies: %d\n", $n, $n - $missing, $missing, $dups;
printf "  latency to first report (s): median %.3f p95 %.3f max %.3f\n", $med, $p95, $max;
my @gone = grep { !$seen{"d$_"} } 1 .. $n; print "  missing distinct files: @gone\n";
printf "  identical refusals 0.5 s apart: 20; reports of that one file: %d\n", $seen{slow} // 0;
printf "  identical refusals: %d; reports of that one file: %d\n", $n, $seen{rep} // 0;
printf "  deny lines on the stream not from this chat's dir: %d (of %d)\n", $stray // 0, $total // 0;
PL

# The folded-summary judge: the kernel's "N duplicate report(s) for Sandbox: ..." lines for this
# chat's dir, how many refusals they stand for, and whether they carry the tag.
cat >"$WORK/folded.pl" <<'PL'
my ($recv, $dir, $tag, $wait) = @ARGV;
my ($lines, $folded, $tagged, $tagged_n) = (0, 0, 0, 0);
open my $r, '<', $recv or die;
while (<$r>) {
  next unless /^(\d+\.\d+) (.*)/s; my ($arr, $line) = ($1, $2);
  next unless $line =~ /duplicate reports? for/ && $line =~ m{\Q$dir\E};
  my ($n) = $line =~ /(\d+) duplicate reports? for/; $lines++; $folded += $n;
  if ($line =~ /\Q$tag\E\s*$/) { $tagged++; $tagged_n += $n }
  print "  folded line: ", substr($line, 0, 300), "\n" if $lines <= 3;
}
printf "  folded summary lines for this chat's dir: %d, standing for %d refusals; carrying the tag at the end: %d (%d refusals); waited %s s after the child ended\n", $lines, $folded, $tagged, $tagged_n, $wait;
PL

# case NAME MODIFIER PREDICATE-TEMPLATE [load] [wait-seconds after the child ends, default 12]
#   @TAG@ in the predicate is replaced by this case's tag; the chat's dir name carries the tag only
#   when the predicate says CONTAINS @DIRTAG@.
case_run() {
  local name=$1 mod=$2 pred=$3 load=${4:-} wait_s=${5:-12} tag="spk$RANDOM$RANDOM"
  local dir="$WORK/chat-$tag" other="$WORK/other-$tag"
  mkdir -p "$dir" "$other"
  pred=${pred//@TAG@/$tag}; pred=${pred//@DIRTAG@/chat-$tag}
  mod=${mod//@TAG@/$tag}
  local prof="(version 1)(allow default)(deny file-write* $mod (subpath \"$dir\"))"
  local oprof="(version 1)(allow default)(deny file-write* (with message \"$tag-other\") (subpath \"$other\"))"
  echo "== case $name${load:+ (under load)}: profile modifier [${mod:-none}], predicate [$pred]"
  local pids=()
  if [ -n "$load" ]; then
    for _ in 1 2 3 4; do yes >/dev/null & pids+=($!); done
    ( while :; do logger -t spikeflood "flood $RANDOM"; done ) & pids+=($!)
    ( while :; do logger -t spikeflood "flood $RANDOM"; done ) & pids+=($!)
  fi
  # $! is this case's own `log stream` (the stamper is a process substitution), so it alone is killed.
  log stream --style compact --predicate "$pred" 2>"$WORK/recv.err" > >(perl "$WORK/stamp.pl" >"$WORK/recv.out") &
  local rpid=$!
  sleep 4
  sandbox-exec -p "$oprof" perl "$WORK/child.pl" "$other" "$N" 0.01 >/dev/null 2>&1 &
  sandbox-exec -p "$prof" perl "$WORK/child.pl" "$dir" "$N" 0.01 >"$WORK/child.out" 2>"$WORK/child.err"
  local rc=$?
  wait $! 2>/dev/null
  sleep "$wait_s"
  kill $rpid 2>/dev/null; wait $rpid 2>/dev/null; sleep 1
  [ ${#pids[@]} -gt 0 ] && kill "${pids[@]}" 2>/dev/null
  [ -s "$WORK/recv.err" ] && echo "  receiver stderr: $(head -c 200 "$WORK/recv.err")"
  [ -s "$WORK/child.err" ] && echo "  child stderr: $(head -c 200 "$WORK/child.err")"
  echo "  child exit $rc; child stamps: $(grep -c '^D' "$WORK/child.out")"
  echo "  receiver lines: $(wc -l <"$WORK/recv.out" | tr -d ' '); sample: $(head -n1 "$WORK/recv.out" | cut -c1-240)"
  echo "  other chat's reports on this stream (tag $tag-other): $(grep -c -e "$tag-other" "$WORK/recv.out")"
  echo "  a deny line: $(grep -m1 " deny" "$WORK/recv.out" | cut -c1-300)"
  if [ "$(id -Gn | grep -c -w admin)" = 0 ]; then
    echo "  log show as this user: $(log show --last 3m --style compact --predicate "$pred" 2>&1 | head -c 200 | tr "\n" "|")"
  fi
  perl "$WORK/judge.pl" "$WORK/child.out" "$WORK/recv.out" "$dir" "$N"
  case "$name" in F_*) perl "$WORK/folded.pl" "$WORK/recv.out" "$dir" "$tag" "$wait_s" ;; esac
}

for _ in 1 2 3; do case_run A_message_tag '(with message "@TAG@")' 'eventMessage ENDSWITH "@TAG@"'; done
for _ in 1 2 3; do case_run B_default_report_path_tag '' 'eventMessage CONTAINS "@DIRTAG@"'; done
case_run C_with_report_path_tag '(with report)' 'eventMessage CONTAINS "@DIRTAG@"'
case_run D_no_report_control '(with no-report)' 'eventMessage CONTAINS "@DIRTAG@"'
case_run A_message_tag '(with message "@TAG@")' 'eventMessage ENDSWITH "@TAG@"' load
case_run B_default_report_path_tag '' 'eventMessage CONTAINS "@DIRTAG@"' load
echo "== folded summary lines: does 'N duplicate reports' carry the tag? (waits 60 s: a fold came about 25 s after its process ended)"
for _ in 1 2; do case_run F_folded_summary_tag '(with message "@TAG@")' 'eventMessage ENDSWITH "@TAG@" OR eventMessage CONTAINS "duplicate report"' '' 60; done
echo "== broad-predicate comparison (what 'log stream over everything' would show): sender Sandbox"
case_run E_message_tag_broad_predicate '(with message "@TAG@")' 'sender == "Sandbox" OR process == "kernel"'
echo "== trace directive: does a profile with (trace) compile?"
sandbox-exec -p "(version 1)(allow default)(trace \"$WORK/trace.sb\")" /usr/bin/true 2>&1 | head -c 300; echo "trace rc=$?"
echo "== done"
