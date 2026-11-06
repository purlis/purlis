# Amended for #5413, a test's host judges its engine on a clock of its own, 2026-11-05

An engine is refused 21 days after the newest manifest a host trusts was issued, and a browser's
start reads the published manifest first once its engine is behind. A test's host started on the
shipped pin, so from the day the pin turned 21 days old a test's browser started only when that
read worked: a 504 from GitHub failed pull requests that changed no browser code.

1. **In a debug build, `PURLIS_TEST_ENGINE_NOW` is the time a browser's start judges age by.** The
   test hosts name the moment the pin was issued, so their engine is current, no manifest is read,
   and the date does not matter. A release build reads the real time whatever the variable says,
   and `purlis_core::browsers::engines_clock` holds that in both directions. Nothing a real install
   refuses changes.
2. **The tests run the pinned engine, not the newest published one.** The archive is still
   downloaded and checked against its digest. What a signed update does is tested in
   `purlis-browser-engine`, against a recorded bundle.
3. **The refusal stays pinned twice.** `purlis-core` holds it at the start with neither read
   answering, and `browsers::tests::end_to_end` holds it on a real `purlis serve` 22 days past the
   pin, its network reads failing at a dead proxy.
