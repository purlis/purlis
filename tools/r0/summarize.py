#!/usr/bin/env python3
"""Read what one R8 job recorded and write summary.md and summary.json beside it.

usage: summarize.py R0_DIR VARIANT

One cargo run per job (`tauri build --no-bundle`), so one timings report: timings-app.html.
Unit times come from cargo's UNIT_DATA: `start`, `duration` and `rmeta_time` (seconds from the
unit's start until its metadata is ready, which for a rustc unit is the end of its front end).
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


P("# R8 baseline, %s\n" % variant)

steps = tsv("steps.tsv")
P("## Steps\n")
table(["step", "seconds", "minutes", "exit"], [[s[0], s[3], "%.1f" % (float(s[3]) / 60), s[4]] for s in steps])
js["steps"] = {s[0]: float(s[3]) for s in steps}
win = {s[0]: (float(s[5]), float(s[6])) for s in steps if len(s) >= 7}

m = re.search(r"const UNIT_DATA = (\[.*?\n\]);", rd("timings-app.html"), re.S)
us = json.loads(m.group(1)) if m else []
P("## Unit times (cargo --timings, seconds from the start of the cargo run)\n")
if not us:
    P("no timing data\n")
else:
    def pick(name, want):
        """Units of crate `name`, by what they are: lib, bin, build-script (compile) or run."""
        r = []
        for u in us:
            if u["name"] != name:
                continue
            t, mode = u["target"].strip(), u["mode"]
            kind = "run-build-script" if mode == "run-custom-build" else "build-script" if "build-script" in t else "bin" if t.startswith("bin") else "lib"
            if kind == want:
                r.append(u)
        return r

    def end(u):
        return u["start"] + u["duration"]

    def fe_end(u):
        return u["start"] + (u.get("rmeta_time") or u["duration"])

    lib = (pick("purlis-app", "lib") or [None])[0]
    core = (pick("purlis-core", "lib") or [None])[0]
    binu = (pick("purlis-app", "bin") or [None])[0]
    total = max(end(u) for u in us)
    key = {"total_compile_s": total, "units": len(us)}
    if lib:
        key.update(app_lib_start_s=lib["start"], app_lib_frontend_end_s=fe_end(lib), app_lib_frontend_s=lib["rmeta_time"] if lib.get("rmeta_time") else None, app_lib_end_s=end(lib))
    if core:
        key.update(core_start_s=core["start"], core_frontend_end_s=fe_end(core), core_codegen_end_s=end(core))
    if binu:
        key.update(app_bin_start_s=binu["start"], app_bin_end_s=end(binu))
    js["key"] = key
    table(["measure", "seconds", "minutes"], [[k, "%.0f" % v, "%.1f" % (v / 60)] for k, v in key.items() if isinstance(v, (int, float)) and k != "units"])
    P("Units: %d. Rows below are the workspace's own units that matter, then the 15 longest of all.\n" % len(us))
    rows = []
    for u in sorted(us, key=lambda u: -u["duration"])[:15]:
        rows.append([u["name"] + " " + u["version"], u["target"].strip() or "lib", u["mode"], "%.0f" % u["start"], "%.0f" % fe_end(u), "%.0f" % end(u), "%.0f" % u["duration"]])
    table(["unit", "target", "mode", "start", "front end end", "end", "seconds"], rows)

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
    if "app-pass" in win:
        a, b = win["app-pass"]
        js["app_pass"] = {"peak_used_mb": max(col(1, a, b)), "peak_swap_mb": max(col(4, a, b)), "peak_rustc_rss_sum_mb": max(col(8, a, b)), "peak_single_process_mb": max(col(6, a, b))}
    js["swap_at_start_mb"] = col(4, -1, 1e12)[0]
else:
    P("no samples\n")

open(os.path.join(d, "summary.md"), "w").write("\n".join(out) + "\n")
json.dump(js, open(os.path.join(d, "summary.json"), "w"), indent=1)
print("\n".join(out))
