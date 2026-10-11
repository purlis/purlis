### Security

- **A test now fails if purlis starts git anywhere but its one runner.** The runner turns off
  what a repository's own config could make git run, such as hooks and an fsmonitor. Every git
  process purlis starts already goes through it. A new call that skipped it now fails a test,
  and another test plants such programs in a repository's config and checks that none of them
  runs. `crates/purlis-core/docs/git-policy.md` lists the call sites, and what the runner does
  not hold yet (#810).
