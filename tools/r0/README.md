# R0 measurement helpers (branch `release-r0-*`, never merged)

Used only by this branch's `release.yml` (ticket #7379, spec #7324, train R). Every file writes
into `$R0_DIR`, which the workflow uploads whole as the artifact `r0-<variant>`.

| file | what it does |
| --- | --- |
| `step.sh NAME cmd…` | runs a command, prints and records its start, end and seconds (`steps.tsv`), and tells the shims which step is running |
| `ts.py` | prefixes every line of stdin with `[+SSSS.s]`, the seconds since it started, so a log reads as a timeline |
| `sample.sh` | background sampler: memory, swap and the biggest process, every 2 s (`samples.tsv`) |
| `cc-shim.py` | the linker: runs the real `cc` and records each link's seconds (`links.tsv`) |
| `dsymutil-shim.py` | `dsymutil` on PATH: runs the real one, records each run's seconds (`dsymutil.tsv`) |
| `summarize.py` | reads all of the above, the cargo timing HTMLs and the filtered logs; writes `summary.md` and `summary.json` |
