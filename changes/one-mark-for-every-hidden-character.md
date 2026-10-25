### Changed

- **One mark for every character you cannot see.** A control, zero-width, direction-changing,
  blank or other invisible character is now written as `⟦U+XXXX⟧` everywhere purlis shows text
  it did not write: the install review, every table and report on the command line, a brief
  that holds one, and a skill version's changes, where some used to show `‮` or `<U+202E>`.
  The same list of characters is checked everywhere, and it is longer: variation selectors,
  Hangul fillers, Arabic number signs and wide or no-break spaces count now. A joiner that
  holds an emoji, a Persian word or a Devanagari conjunct together reads as text and is not
  marked, so `👨‍👩‍👧` shows as one family (#3172). Every emoji Unicode names is one here, so
  `©️`, `▶️` and `㊗️` read as text too, as do a keycap such as `#️⃣` and the flags of England,
  Scotland and Wales; a black flag with any other tags after it is still marked (#3219).
