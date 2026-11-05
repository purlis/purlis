### Fixed

- **A fresh runner on an image with very many files its user may change can go.** Where the runner's user may change every file in a folder it may not write, that folder is recorded once and its files are looked at at each check, so such an image is looked through whole instead of keeping every fresh runner on it (#5112).
