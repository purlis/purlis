### Fixed

- CI's `recorded behaviour` job installs the mold linker on every run, so a pull request that leaves Rust out of scope (a draft, say) no longer fails its whole-tree guards with `cannot find 'ld'`. A test now holds every job that links with mold to installing it on every run.
