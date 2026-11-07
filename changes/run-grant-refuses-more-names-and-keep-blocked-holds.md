### Security

- **A Run grant is offered for fewer programs that run others.** GNU builds by their `g` or
  `gnu` name (`gm4`, `genv`, `gnumake`), git's own programs (`git-shell`), versioned and
  cross compilers (`gcc-13`, `clang-17`, `x86_64-linux-gnu-gcc`) are refused like the programs
  they are, with no notice raised. `curl` is still offered (#5424).

### Fixed

- **Keep blocked on a refused vault holds against a run already on its way.** A run that
  passed the check just before you pressed it no longer raises a notice after it, and the
  press no longer waits on the disk on the window's thread (#5424).
