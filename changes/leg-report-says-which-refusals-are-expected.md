### Changed

- The sandbox scenario legs' reports say which entries expect a refusal (`refusal_expected`), so the nightly and `stress-x20` comparisons read an escape from a run's own reports and not from the cells' files. A report without the field is still read against the cells, so an old one is never taken for "not an escape".
