# R8 measurement helpers (branch `r8-7499-*`, never merged)

Ported from R0's `tools/r0` (commit 7901dc341) for ticket #7499, spec #7497. Used only by that
branch's `release.yml`. Every file writes into `$R0_DIR`, which the workflow uploads whole as the
artifact `r8-<job>`.

| file | what it does |
| --- | --- |
| `step.sh NAME cmd…` | runs a command, prints and records its start, end and seconds (`steps.tsv`), and tells the shims which step is running |
| `ts.py` | prefixes every line of stdin with `[+SSSS.s]`, the seconds since it started |
| `sample.sh` | background sampler: memory, swap and the biggest process, every 2 s (`samples.tsv`) |
| `cc-shim.py` | the linker: runs the real `cc` and records each link's seconds (`links.tsv`) |
| `dsymutil-shim.py` | `dsymutil` on PATH: runs the real one, records each run's seconds (`dsymutil.tsv`) |
| `summarize.py` | reads the above and `timings-app.html`; writes `summary.md` and `summary.json` |
