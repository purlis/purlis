#!/usr/bin/env python3
"""The linker, timed: runs the real cc and records the seconds. rustc names it with -C linker."""
import os, subprocess, sys, time
args = sys.argv[1:]
def output_of(a):
    for i, x in enumerate(a):
        if x == "-o" and i + 1 < len(a):
            return a[i + 1]
        if x.startswith("@") and os.path.exists(x[1:]):
            try:
                with open(x[1:]) as f:
                    toks = f.read().split()
                for j, t in enumerate(toks):
                    if t.strip('"') == "-o" and j + 1 < len(toks):
                        return toks[j + 1].strip('"')
            except OSError:
                pass
    return "?"
t = time.time()
rc = subprocess.call([os.environ.get("R0_REAL_CC", "cc")] + args)
d = time.time() - t
try:
    step = open(os.path.join(os.environ["R0_DIR"], "current-step")).read().strip()
    with open(os.path.join(os.environ["R0_DIR"], "links.tsv"), "a") as f:
        f.write("%s\t%.3f\t%.3f\t%s\n" % (step, t, d, os.path.basename(output_of(args))))
except Exception:
    pass
sys.exit(rc)
