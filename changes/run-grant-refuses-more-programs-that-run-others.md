### Security

- **More programs that run others get no Allow from a run's notice.** These each run a command
  that a flag, an address, a template or their own words name: `xcodebuild`, `msgexec`, `leaks`,
  `pyenv`, `erb`, `helm`, `socat`, `ncat`, `nc`, `netcat`, `tar`, `bsdtar` and `msgfilter`; the
  secret injectors `op`, `aws-vault`, `doppler`, `infisical`, `sops`, `chamber`, `envconsul`,
  `summon`, `envchain`, `dotenvx`, `teller`, `berglas`, `aws-sso`, `granted` and `assume`; the
  environment managers and project runners `rbenv`, `asdf`, `mise`, `rtx`, `direnv`, `pipenv`,
  `conda`, `mamba`, `micromamba`, `volta`, `goenv`, `jenv`, `fnm`, `nix`, `nix-shell`, `devbox`,
  `pixi`, `hatch`, `pdm`, `rye`, `tox`, `nox` and `helmfile`; and `xctrace`, `valgrind` and
  `hyperfine`. A chat's run of one with a vault's key is refused at once, with no notice. A
  persona can still declare one as a tool (#5619).
