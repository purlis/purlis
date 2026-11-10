# Amended 2026-11-10: the intake's organisation is the slug Sentry gave, `purlis-inc-ff`

Found on the routine's first live run (#5701, #5687). The wizard asked for the organisation slug
`purlis`, and Sentry gave the EU organisation `purlis-inc-ff`: the read token's
`/api/0/projects/` lists `purlis-app` there, and the events address with `purlis` answers 404.

**The intake is organisation `purlis-inc-ff`**, project `purlis-app`, on `de.sentry.io`, and its
pages are on `purlis-inc-ff.sentry.io`. `tools/report-intake.mjs` reads it there.

**The setup wizard asks for the slug** (`tools/wizards/report-intake.sh`) instead of assuming
the one it asked Sentry for, since Sentry may add a suffix. It defaults to `purlis-inc-ff`, opens
the organisation's settings pages under it, and sets the `release` environment's `SENTRY_ORG`
secret to it.
