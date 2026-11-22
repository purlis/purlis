#!/usr/bin/env python3
"""Read what one R0 variant recorded and write summary.md and summary.json beside it.

usage: summarize.py R0_DIR VARIANT
"""
import collections, glob, json, os, re, shlex, sys

d, variant = sys.argv[1], sys.argv[2]
out = []
js = {"variant": variant}


def P(*a):
    out.append(" ".join(str(x) for x in a))


def rd(name):
    p = os.path.join(d, name)
    return open(p, errors="replace").read() if os.path.exists(p) else ""


def tsv(name):
    return [l.rstrip("\n").split("\t") for l in rd(name).splitlines() if l.strip()]


def table(head, rows):
    P("| " + " | ".join(head) + " |")
    P("|" + "|".join(["---"] * len(head)) + "|")
    for r in rows:
        P("| " + " | ".join(str(x) for x in r) + " |")
    P()


def mins(s):
    return "%.1f min" % (float(s) / 60)


P("# R0 — %s\n" % variant)

# ---- steps
steps = tsv("steps.tsv")
P("## Steps (UTC, as the runner's clock read it)\n")
table(["step", "start", "end", "seconds", "minutes", "exit"],
      [[s[0], s[1], s[2], s[3], "%.1f" % (float(s[3]) / 60), s[4]] for s in steps])
js["steps"] = {s[0]: float(s[3]) for s in steps}
step_win = {s[0]: (float(s[5]), float(s[6])) for s in steps if len(s) >= 7}

# ---- the machine, and what running a new executable costs it
P("## Developer mode and the cost of running a new executable\n")
for f in ("machine.txt", "devmode.txt"):
    if rd(f).strip():
        P("```\n" + rd(f).strip() + "\n```\n")
probe = tsv("exec-probe.tsv")
if probe:
    table(["probe", "compile 60 programs, s", "run 60 new programs, s"], [[r[0], r[1], r[2]] for r in probe])
    js["exec_probe"] = {r[0]: [float(r[1]), float(r[2])] for r in probe}

# ---- timings per pass
ws_names = set()
try:
    md = json.loads(rd("metadata.json"))
    ws_names = {p["name"] for p in md["packages"]}
except Exception:
    P("(no metadata.json: every crate counts as third-party)\n")

UNIT = re.compile(r"const UNIT_DATA = (\[.*?\n\]);", re.S)


def units(html):
    m = UNIT.search(html)
    return json.loads(m.group(1)) if m else []


def category(u):
    if u["mode"] == "run-custom-build":
        return "build scripts, run"
    return "workspace crates" if u["name"] in ws_names else "third-party crates"


def attribute(us):
    """Split each wall-clock second among the units running in it."""
    ev = sorted({u["start"] for u in us} | {u["start"] + u["duration"] for u in us})
    wall = collections.Counter()
    for a, b in zip(ev, ev[1:]):
        run = [u for u in us if u["start"] <= a and u["start"] + u["duration"] >= b]
        if not run:
            wall["idle (nothing compiling)"] += b - a
            continue
        for u in run:
            wall[category(u)] += (b - a) / len(run)
    return wall


passes = {}
for tag, label in (("cli", "CLI pass (purlis-cli, persona-statistics)"), ("app", "App pass (tauri build)")):
    us = units(rd("timings-%s.html" % tag))
    passes[tag] = us
    P("## Timings: %s\n" % label)
    if not us:
        P("no timing data\n")
        continue
    total = max(u["start"] + u["duration"] for u in us)
    cpu = collections.Counter()
    for u in us:
        cpu[category(u)] += u["duration"]
    wall = attribute(us)
    P("cargo wall: **%s** (%d units)\n" % (mins(total), len(us)))
    table(["category", "summed unit seconds", "share of summed", "wall seconds attributed", "share of wall"],
          [[k, "%.0f" % cpu[k], "%.0f%%" % (100 * cpu[k] / sum(cpu.values())),
            "%.0f" % wall.get(k, 0), "%.0f%%" % (100 * wall.get(k, 0) / total)] for k in sorted(set(cpu) | set(wall))])
    P("Longest units:\n")
    table(["unit", "target", "mode", "start s", "seconds"],
          [[u["name"] + " " + u["version"], u["target"].strip(), u["mode"], "%.0f" % u["start"], "%.0f" % u["duration"]]
           for u in sorted(us, key=lambda u: -u["duration"])[:15]])
    js["timings_" + tag] = {"wall_s": total, "units": len(us), "summed_by_category": dict(cpu), "wall_by_category": dict(wall)}

# ---- crates compiled in both passes
LOGLINE = re.compile(r"^\[\+\s*[\d.]+\]\s+(.*)$")


def log_lines(name):
    return [LOGLINE.sub(r"\1", l) if LOGLINE.match(l) else l for l in rd(name).splitlines()]


def compiled(lines):
    s = set()
    for l in lines:
        m = re.match(r"\s*Compiling (\S+) (v\S+)", l)
        if m:
            s.add((m.group(1), m.group(2)))
    return s


cli_l, app_l = log_lines("cli.log"), log_lines("app.log")
c1, c2 = compiled(cli_l), compiled(app_l)
both = c1 & c2
P("## Crates compiled in both passes\n")
P("CLI pass compiled %d crate versions, app pass %d, **%d in both**.\n" % (len(c1), len(c2), len(both)))
dur2 = collections.defaultdict(float)
for u in passes.get("app", []):
    if u["mode"] != "run-custom-build":
        dur2[(u["name"], "v" + u["version"])] += u["duration"]
dboth = sum(dur2[k] for k in both)
dall = sum(dur2.values()) or 1
P("Those %d crates took %.0f s of the app pass's %.0f summed unit seconds (%.0f%%): that is what a second compile costs.\n" % (len(both), dboth, dall, 100 * dboth / dall))
ws_both = [k for k in both if k[0] in ws_names]
P("Of them %d are workspace crates and %d are third-party.\n" % (len(ws_both), len(both) - len(ws_both)))
js["both_passes"] = {"cli": len(c1), "app": len(c2), "both": len(both), "second_compile_s": dboth, "app_summed_s": dall}


# ---- why: compare the rustc arguments of a crate in each pass
def rustc_runs(lines):
    runs = {}
    for l in lines:
        if "--crate-name" not in l or "Running" not in l:
            continue
        body = l[l.index("`") + 1:l.rindex("`")] if "`" in l else l
        try:
            t = shlex.split(body)
        except ValueError:
            continue
        if "--crate-name" not in t:
            continue
        i = t.index("--crate-name")
        name = t[i + 1]
        src = next((x for x in t[i + 2:] if x.endswith(".rs") and not x.startswith("-")), "")
        ct = t[t.index("--crate-type") + 1] if "--crate-type" in t else ""
        feats, rest, skip = set(), [], False
        for j, x in enumerate(t):
            if skip:
                skip = False
                continue
            if x in ("--out-dir", "-L", "-C", "--emit", "--error-format", "--json", "--extern", "--check-cfg", "--cfg") and j + 1 < len(t):
                nxt = t[j + 1]
                if x == "--cfg" and nxt.startswith("feature="):
                    feats.add(nxt.split("=", 1)[1].strip('"'))
                elif x == "-C" and not (nxt.startswith("metadata=") or nxt.startswith("extra-filename=") or nxt.startswith("incremental=")):
                    rest.append("-C " + nxt)
                elif x == "--cfg":
                    rest.append("--cfg " + nxt)
                skip = True
                continue
            if x.startswith("-") or x.endswith(".rs"):
                rest.append(x)
        runs[(name, src, ct)] = (feats, sorted(rest))
    return runs


r1, r2 = rustc_runs(cli_l), rustc_runs(app_l)
P("## Why the second pass recompiles: rustc's arguments, pass 1 vs pass 2\n")
if not r1 or not r2:
    P("no `Running rustc` lines found in the logs (was -v on?)\n")
else:
    feat_diff, flag_diff, same = [], [], []
    for k in r1.keys() & r2.keys():
        f1, a1 = r1[k]
        f2, a2 = r2[k]
        if f1 != f2:
            feat_diff.append((k[0], sorted(f2 - f1), sorted(f1 - f2)))
        elif a1 != a2:
            flag_diff.append((k[0], sorted(set(a2) - set(a1)), sorted(set(a1) - set(a2))))
        else:
            same.append(k[0])
    P("Units rustc ran in both passes: %d. Feature set differs: **%d**. Same features, other flags differ: **%d**. Arguments identical (the cause is outside the command line: a different `-C metadata` from the dependency's own features, or env): **%d**.\n" % (len(r1.keys() & r2.keys()), len(feat_diff), len(flag_diff), len(same)))
    if feat_diff:
        P("Feature differences (crate: added in pass 2 / only in pass 1), first 25:\n")
        table(["crate", "added in app pass", "only in CLI pass"], [[n, ", ".join(a) or "-", ", ".join(b) or "-"] for n, a, b in sorted(feat_diff)[:25]])
    if flag_diff:
        P("Other flag differences, first 15:\n")
        table(["crate", "added in app pass", "only in CLI pass"], [[n, " ".join(a)[:80] or "-", " ".join(b)[:80] or "-"] for n, a, b in sorted(flag_diff)[:15]])
    if same:
        P("Identical arguments (first 25): " + ", ".join(sorted(set(same))[:25]) + "\n")
    js["why"] = {"feature_differs": len(feat_diff), "flags_differ": len(flag_diff), "identical_args": len(same)}

# ---- dirty / fresh
P("## Rebuild reasons (cargo's own fingerprint lines)\n")
dirty = [l for l in cli_l + app_l if re.search(r"\bDirty\b|dirty:|fingerprint dirty", l)]
fresh = [l for l in cli_l + app_l if re.match(r"\s*Fresh ", l)]
reasons = collections.Counter(re.sub(r"^\s*Dirty \S+ \S+( \([^)]*\))?: ", "", l)[:100] for l in dirty)
P("Dirty lines: **%d**; Fresh lines: **%d** (the full filtered list is `rebuild-reasons.txt` in the artifact).\n" % (len(dirty), len(fresh)))
if reasons:
    table(["reason (first 100 chars)", "count"], [[r.replace("|", "/"), n] for r, n in reasons.most_common(12)])
else:
    P("None: on a cold checkout a crate has no earlier fingerprint to be dirty against, so a rebuild is a *new unit*, see the section above.\n")
js["dirty"] = len(dirty); js["fresh"] = len(fresh)

# ---- front end
P("## Front end\n")


def first_t(lines, pat):
    for l in lines:
        if re.search(pat, l):
            m = re.match(r"^\[\+\s*([\d.]+)\]", l)
            return float(m.group(1)) if m else None
    return None


raw_app = rd("app.log").splitlines()
tb, tc = first_t(raw_app, r"beforeBuildCommand"), first_t(raw_app, r"\bCompiling\b|\bFresh\b")
if tb is not None and tc is not None:
    P("`npm run build` (tsc + vite) ran from +%.0f s to the first cargo unit at +%.0f s: **%.0f s** before the app pass compiled anything.\n" % (tb, tc, tc - tb))
    js["front_end_s"] = tc - tb
else:
    P("could not find the beforeBuildCommand and first Compiling lines in app.log\n")

# ---- links and dsymutil
P("## Links and dsymutil\n")
links = [(l[0], float(l[1]), float(l[2]), l[3]) for l in tsv("links.tsv")]
dsym = [(l[0], float(l[1]), float(l[2]), l[3]) for l in tsv("dsymutil.tsv")]
for stepname in sorted({l[0] for l in links} | {l[0] for l in dsym}):
    ls = [l for l in links if l[0] == stepname]
    ds = [l for l in dsym if l[0] == stepname]
    P("### during `%s`\n" % stepname)
    P("%d links, **%.0f s** in total (build scripts and proc macros included); %d dsymutil runs, **%.0f s** in total.\n" % (len(ls), sum(l[2] for l in ls), len(ds), sum(l[2] for l in ds)))
    table(["longest links", "seconds"], [[l[3], "%.1f" % l[2]] for l in sorted(ls, key=lambda l: -l[2])[:10]])
    if ds:
        table(["dsymutil on", "seconds"], [[l[3], "%.1f" % l[2]] for l in sorted(ds, key=lambda l: -l[2])[:10]])
js["links_s"] = sum(l[2] for l in links); js["dsymutil_s"] = sum(l[2] for l in dsym)
if not dsym:
    P("No dsymutil run was seen through the PATH shim (Linux, or rustc did not find the shim).\n")

# ---- memory
P("## Memory and swap\n")
rows = [r for r in tsv("samples.tsv")[1:] if len(r) >= 9]
if rows:
    def col(i, lo=None, hi=None):
        return [float(r[i]) for r in rows if (lo is None or float(r[0]) >= lo) and (hi is None or float(r[0]) <= hi)]

    def window(lo, hi):
        u = col(1, lo, hi)
        if not u:
            return None
        sw = col(4, lo, hi); so = col(5, lo, hi); rs = col(8, lo, hi); top = col(6, lo, hi)
        topi = [r for r in rows if lo <= float(r[0]) <= hi and float(r[6]) == max(top)][0][7] if top else "?"
        return [max(u), max(sw), max(so) - min(so), max(rs), max(top), topi]
    base_sw = min(col(4)) if col(4) else 0
    wins = [("whole job", None, None)] + [(n, a, b) for n, (a, b) in step_win.items() if (b - a) > 120]
    table(["window", "peak used MB", "peak swap used MB", "swapouts during (pages)", "peak sum of rustc RSS MB", "peak single process MB", "which"],
          [[n] + (window(a if a is not None else -1, b if b is not None else 1e12) or ["-"] * 6) for n, a, b in wins])
    js["peak_used_mb"] = max(col(1)); js["peak_swap_mb"] = max(col(4)); js["swap_baseline_mb"] = base_sw
    P("Swap at the start of the job: %.0f MB. Peak swap used: **%.0f MB**.\n" % (base_sw, max(col(4))))
else:
    P("no samples\n")

open(os.path.join(d, "summary.md"), "w").write("\n".join(out) + "\n")
json.dump(js, open(os.path.join(d, "summary.json"), "w"), indent=1)
print("\n".join(out))
