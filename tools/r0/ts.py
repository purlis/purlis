#!/usr/bin/env python3
"""Prefix each stdin line with the seconds since this filter started."""
import sys, time
t0 = time.time()
for line in sys.stdin:
    sys.stdout.write("[+%7.1f] %s" % (time.time() - t0, line))
    sys.stdout.flush()
