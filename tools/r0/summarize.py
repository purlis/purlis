#!/usr/bin/env python3
"""Read what one R8 job recorded and write summary.md and summary.json beside it.

usage: summarize.py R0_DIR VARIANT

One timings report per cargo run: timings-app.html, and for arm A timings-core.html (the first pass).
Unit times come from cargo's UNIT_DATA: `start`, `duration` and the `frontend` and `codegen`
sections (the front end ends at the unit's metadata point).
"""
import json, os, re, sys

d, variant = sys.argv[1], sys.argv[2]
out, js = [], {"variant": variant}


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


P("# R8 #7500, %s\n" % variant)

steps = tsv("steps.tsv")
P("## Steps\n")
table(["step", "seconds", "minutes", "exit"], [[s[0], s[3], "%.1f" % (float(s[3]) / 60), s[4]] for s in steps])
js["steps"] = {s[0]: float(s[3]) for s in steps}
win = {s[0]: (float(s[5]), float(s[6])) for s in steps if len(s) >= 7}

def units(name):
    m = re.search(r"const UNIT_DATA = (\[.*?\n\]);", rd(name), re.S)
    return json.loads(m.group(1)) if m else []


def pick(us, name, want):
    """Units of crate `name`, by what they are: lib, bin, build-script (compile) or run."""
    r = []
    for u in us:
        if u["name"] != name:
            continue
        t, mode = u["target"].strip(), u["mode"]
        kind = "run-build-script" if mode == "run-custom-build" else "build-script" if "build-script" in t else "bin" if t.startswith("bin") or '"bin"' in t else "lib"
        if kind == want:
            r.append(u)
    return r


def end(u):
    return u["start"] + u["duration"]


def fe(u):
    """Seconds of the unit's front end: cargo's `frontend` section (rustc's metadata point)."""
    for name, sec in u.get("sections") or []:
        if name == "frontend":
            return sec["end"]
    return u.get("rmeta_time")


def measures(us):
    k = {"total_compile_s": max(end(u) for u in us), "units": len(us)}
    lib = (pick(us, "purlis-app", "lib") or [None])[0]
    core = (pick(us, "purlis-core", "lib") or [None])[0]
    binu = (pick(us, "purlis-app", "bin") or [None])[0]
    if core:
        f = fe(core)
        k.update(core_start_s=core["start"], core_frontend_end_s=core["start"] + f if f is not None else None,
                 core_codegen_end_s=end(core), core_unit_s=core["duration"])
    if lib:
        f = fe(lib)
        k.update(app_lib_start_s=lib["start"], app_lib_frontend_s=f,
                 app_lib_frontend_end_s=lib["start"] + f if f is not None else None,
                 app_lib_codegen_s=lib["duration"] - f if f is not None else None, app_lib_end_s=end(lib))
    if binu:
        k.update(app_bin_start_s=binu["start"], app_bin_end_s=end(binu), app_bin_s=binu["duration"])
    return k, lib, core


for label, fname in (("core pass (arm A only)", "timings-core.html"), ("app pass", "timings-app.html")):
    us = units(fname)
    if not us:
        if fname == "timings-app.html":
            P("## Unit times, %s\n\nno timing data\n" % label)
        continue
    P("## Unit times, %s (cargo --timings, seconds from the start of that cargo run)\n" % label)
    key, lib, core = measures(us)
    js["core_pass" if fname == "timings-core.html" else "key"] = key
    table(["measure", "seconds", "minutes"], [[k, "%.0f" % v, "%.1f" % (v / 60)] for k, v in key.items() if isinstance(v, (int, float)) and k != "units"])
    if fname == "timings-app.html" and os.path.exists(os.path.join(d, "timings-core.html")):
        # Arm A's claim rests on this: the app pass found purlis-core built and compiled none of it.
        js["core_rebuilt_in_app_pass"] = core is not None
        P("purlis-core compiled again in the app pass: **%s**\n" % ("yes, arm A is not valid for this run" if core else "no"))
    if lib:
        a, b = lib["start"], end(lib)
        over = [u for u in us if u is not lib and u["start"] < b and end(u) > a]
        busy = sum(min(end(u), b) - max(u["start"], a) for u in over)
        js.setdefault("lib_window", {})[fname] = {"units_overlapping": len(over), "other_unit_seconds_in_window": busy}
        P("Other units running while the app lib compiled: %d, %.0f unit-seconds.\n" % (len(over), busy))
    P("Units: %d. The 15 longest:\n" % len(us))
    rows = []
    for u in sorted(us, key=lambda u: -u["duration"])[:15]:
        f = fe(u)
        rows.append([u["name"] + " " + u["version"], u["target"].strip() or "lib", "%.0f" % u["start"], "%.0f" % (u["start"] + f) if f is not None else "-", "%.0f" % end(u), "%.0f" % u["duration"]])
    table(["unit", "target", "start", "front end end", "end", "seconds"], rows)

P("## Binaries and the smoke check\n")
sizes = tsv("sizes.tsv")
if sizes:
    table(["file", "bytes", "MB"], [[r[0], r[1], "%.1f" % (int(r[1]) / 1e6)] for r in sizes])
    js["sizes"] = {r[0]: int(r[1]) for r in sizes}
smoke = rd("smoke.txt").strip()
P("```\n%s\n```\n" % (smoke or "no smoke check"))
js["smoke"] = smoke

P("## Links and dsymutil\n")
links = [(l[0], float(l[2]), l[3]) for l in tsv("links.tsv")]
dsym = [(l[0], float(l[2]), l[3]) for l in tsv("dsymutil.tsv")]
P("%d links, %.0f s in total (build scripts and proc macros included); %d dsymutil runs, %.0f s in total.\n" % (len(links), sum(l[1] for l in links), len(dsym), sum(l[1] for l in dsym)))
table(["longest links", "seconds"], [[l[2], "%.1f" % l[1]] for l in sorted(links, key=lambda l: -l[1])[:6]])
if dsym:
    table(["dsymutil on", "seconds"], [[l[2], "%.1f" % l[1]] for l in sorted(dsym, key=lambda l: -l[1])[:6]])
js["links_s"], js["dsymutil_s"] = sum(l[1] for l in links), sum(l[1] for l in dsym)

P("## Memory and swap (sampled every 2 s)\n")
rows = [r for r in tsv("samples.tsv")[1:] if len(r) >= 9]
if rows:
    def col(i, lo, hi):
        return [float(r[i]) for r in rows if lo <= float(r[0]) <= hi]

    def peaks(lo, hi):
        u, sw, rs, top = col(1, lo, hi), col(4, lo, hi), col(8, lo, hi), col(6, lo, hi)
        if not u:
            return ["-"] * 5
        return ["%.0f" % max(u), "%.0f" % max(sw), "%.0f" % max(rs), "%.0f" % max(top), "%.0f" % (max(col(5, lo, hi)) - min(col(5, lo, hi)))]
    wins = [("whole job", -1, 1e12)] + [(n, a, b) for n, (a, b) in win.items() if b - a > 120]
    table(["window", "peak used MB", "peak swap MB", "peak sum of rustc RSS MB", "peak single process MB", "swapouts during (pages)"],
          [[n] + peaks(a, b) for n, a, b in wins])
    for step in ("core-pass", "app-pass"):
        if step in win:
            a, b = win[step]
            js[step.replace("-", "_")] = {"peak_used_mb": max(col(1, a, b)), "peak_swap_mb": max(col(4, a, b)), "peak_rustc_rss_sum_mb": max(col(8, a, b)), "peak_single_process_mb": max(col(6, a, b))}
    js["swap_at_start_mb"] = col(4, -1, 1e12)[0]
else:
    P("no samples\n")

open(os.path.join(d, "summary.md"), "w").write("\n".join(out) + "\n")
json.dump(js, open(os.path.join(d, "summary.json"), "w"), indent=1)
print("\n".join(out))
