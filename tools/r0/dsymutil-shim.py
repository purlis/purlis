#!/usr/bin/env python3
"""dsymutil, timed: runs the real one (R0_REAL_DSYMUTIL) and records the seconds."""
import os, subprocess, sys, time
args = sys.argv[1:]
t = time.time()
rc = subprocess.call([os.environ["R0_REAL_DSYMUTIL"]] + args)
d = time.time() - t
try:
    step = open(os.path.join(os.environ["R0_DIR"], "current-step")).read().strip()
    target = next((os.path.basename(a) for a in reversed(args) if not a.startswith("-")), "?")
    with open(os.path.join(os.environ["R0_DIR"], "dsymutil.tsv"), "a") as f:
        f.write("%s\t%.3f\t%.3f\t%s\n" % (step, t, d, target))
except Exception:
    pass
sys.exit(rc)
