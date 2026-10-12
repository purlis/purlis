### Security

- **purlis's git turns off hooks a repo names in its config.** Newer git (2.54 and later) can
  run a hook named in a repo's config, which the runner's turned-off hooks folder did not cover.
  Each such hook is now turned off by its name on every git call that could fire it, including
  one that only writes a branch or a ref (#1805).
