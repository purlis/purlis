#!/usr/bin/env python3
"""Read what one R7 variant recorded and write summary.md and summary.json beside it.

usage: summarize.py R7_DIR VARIANT LTO_LABEL
"""
import collections, glob, json, os, re, sys

d, variant, label = sys.argv[1], sys.argv[2], sys.argv[3]
out, js = [], {"variant": variant, "lto": label}


def P(*a):
    out.append(" ".join(str(x) for x in a))


def rd(name):
    p = os.path.join(d, name)
    return open(p, errors="replace").read() if os.path.exists(p) else ""


def tsv(name):
    return [l.split("\t") for l in rd(name).splitlines() if l.strip()]


def table(head, rows):
    P("| " + " | ".join(head) + " |")
    P("|" + "|".join(["---"] * len(head)) + "|")
    for r in rows:
        P("| " + " | ".join(str(x) for x in r) + " |")
    P()


P("# R7 — %s (lto: %s)\n" % (variant, label))
P("```\n" + rd("machine.txt").strip() + "\n```\n")

steps = tsv("steps.tsv")
P("## Steps\n")
table(["step", "start", "end", "seconds", "minutes", "exit"],
      [[s[0], s[1], s[2], s[3], "%.1f" % (float(s[3]) / 60), s[4]] for s in steps])
js["steps"] = {s[0]: float(s[3]) for s in steps}
win = {s[0]: (float(s[5]), float(s[6])) for s in steps if len(s) >= 7}

# cargo's own account of the one compile
UNIT = re.compile(r"const UNIT_DATA = (\[.*?\n\]);", re.S)
m = UNIT.search(rd("timings.html"))
us = json.loads(m.group(1)) if m else []
P("## Cargo timings (the one compile)\n")
if us:
    total = max(u["start"] + u["duration"] for u in us)
    P("cargo wall: **%.1f min**, %d units.\n" % (total / 60, len(us)))
    big = sorted(us, key=lambda u: -u["duration"])[:12]
    table(["unit", "target", "mode", "start s", "seconds"],
          [[u["name"] + " " + u["version"], u["target"].strip(), u["mode"], "%.0f" % u["start"], "%.0f" % u["duration"]] for u in big])
    js["cargo_wall_s"] = total
    js["units"] = {u["name"] + (":" + u["target"].strip() if u["target"].strip() else "") + ":" + u["mode"]: u["duration"]
                   for u in us if u["name"] in ("purlis-app", "purlis-cli", "purlis-core", "persona-statistics", "purlis-same-user")}
else:
    P("no timing data\n")

# links and dsymutil
links = [(l[0], float(l[1]), float(l[2]), l[3]) for l in tsv("links.tsv")]
dsym = [(l[0], float(l[1]), float(l[2]), l[3]) for l in tsv("dsymutil.tsv")]
P("## Links and dsymutil\n")
P("%d links, **%.0f s** in total; %d dsymutil runs, **%.0f s** in total.\n" % (len(links), sum(l[2] for l in links), len(dsym), sum(l[2] for l in dsym)))
table(["longest links", "seconds"], [[l[3], "%.1f" % l[2]] for l in sorted(links, key=lambda l: -l[2])[:6]])
if dsym:
    table(["dsymutil on", "seconds"], [[l[3], "%.1f" % l[2]] for l in sorted(dsym, key=lambda l: -l[2])[:6]])
js["links_s"] = sum(l[2] for l in links)
js["dsymutil_s"] = sum(l[2] for l in dsym)
js["dsymutil_by_target"] = {l[3]: l[2] for l in dsym}

# sizes
sizes = tsv("sizes.tsv")
P("## Sizes (bytes)\n")
table(["file", "bytes", "MiB"], [[s[0], s[1], "%.1f" % (int(s[1]) / 1048576)] for s in sizes])
js["sizes"] = {s[0]: int(s[1]) for s in sizes}

# cold start
P("## Cold start (tools/bench.mjs, coldstart arm)\n")
cs = None
for f in glob.glob(os.path.join(d, "coldstart*.json")):
    try:
        cs = json.load(open(f))["coldStart"]
    except Exception:
        pass
if cs:
    a, b = cs.get("launchToFirstFrame", {}), cs.get("processStartToFirstFrameAsTheAppSeesIt", {})
    P("launch to first frame: median **%.0f ms**, worst %.0f ms, samples %s\n" % (a.get("p50", 0), a.get("worst", 0), a.get("samples_ms")))
    P("process start to first frame, the app's own clock: median %.0f ms, samples %s\n" % (b.get("p50", 0), b.get("samples_ms")))
    if cs.get("coldDiskWarmUp"):
        P("discarded warm-up launch: %s\n" % cs["coldDiskWarmUp"])
    js["coldstart"] = {"launch_ms": a.get("samples_ms"), "launch_p50": a.get("p50"), "app_ms": b.get("samples_ms"), "app_p50": b.get("p50")}
else:
    P("no cold start measurement was taken (see coldstart.log)\n")

# memory
rows = [r for r in tsv("samples.tsv")[1:] if len(r) >= 9]
if rows:
    P("## Memory\n")
    P("Peak used %d MB, peak swap %d MB, peak sum of rustc RSS %d MB.\n" % (max(float(r[1]) for r in rows), max(float(r[4]) for r in rows), max(float(r[8]) for r in rows)))
    js["peak_used_mb"] = max(float(r[1]) for r in rows)
    js["peak_swap_mb"] = max(float(r[4]) for r in rows)
    js["peak_rustc_rss_mb"] = max(float(r[8]) for r in rows)

open(os.path.join(d, "summary.md"), "w").write("\n".join(out) + "\n")
json.dump(js, open(os.path.join(d, "summary.json"), "w"), indent=1)
print("\n".join(out))
