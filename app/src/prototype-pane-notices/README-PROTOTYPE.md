# PROTOTYPE: minimised and expanded pane notices (#4584)

Throwaway. Branch `prototype/pane-notices`, never merged. Map #4575, decision D7 of #4576.

Open it, from any clone of purlis:

```bash
git fetch origin prototype/pane-notices && git show FETCH_HEAD:app/src/prototype-pane-notices/PaneNotices.prototype.html > "${TMPDIR:-/tmp}/pane-notices.html" && open "${TMPDIR:-/tmp}/pane-notices.html"
```

The dashed bar switches variant A / B / C, loads states 1-6, and makes a note or an ask arrive.
URL params: `?variant=A&s=4&m=min&t=dark`. `screenshots/` holds each variant in state 4, minimised
and expanded, plus A in state 5 (overflow), state 6 (with the #4561 strip) and light.
Stand-in data in memory; nothing calls the core.
