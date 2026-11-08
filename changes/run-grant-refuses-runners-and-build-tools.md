### Security

- **Still more programs that run others get no Allow from a run's notice.** Programs that run
  the command after them or one they name: `consul-template`, `railway`, `aws-runas`, `vals`,
  `gomplate`, `envdir`, `chpst`, `systemd-run`, `run0`, `telepresence`, `firejail`, `flatpak`,
  `flatpak-spawn`, `rr`, `pkgx` and `flox`. Programs that run what a file a chat wrote says:
  `task`, `skaffold`, `tilt`, `dagger`, `earthly`, `act`, `firebase`, `netlify`, `vercel`,
  `wrangler`, `chezmoi`, `mage`, `invoke`, `fab` and `cdk8s`. Build tools and test runners: `sbt`,
  `mix`, `playwright`, `cypress`, `gitlab-runner`, `circleci`, `stack`, `cabal`, `dune`, `zig`
  and `lein`. A chat's run of one with a vault's key is refused at once, with no notice. A
  persona can still declare one as a tool (#5833).
