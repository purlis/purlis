### Changed

- **`[policy.limits]` is read against the table of limits.** A key the table doesn't name, or a
  number that isn't a whole number from 0, makes the project file broken, and the message names
  the key. Raising a cap or lowering a retention is a loosening, so it waits for your press, while
  lowering a cap or keeping longer applies at once. Under `limits = log` an exceeded cap is
  counted, not refused (M75-12, #7021).
