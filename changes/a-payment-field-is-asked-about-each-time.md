### Security

- **A field the page marks for a card or payment is asked about each time.** Where a page marks
  a field `autocomplete="cc-…"` (a card's number, its expiry, its code), filling a secret into
  it raises the notice with Allow once and Keep blocked alone, under Allow for this chat or
  Always allow too, whether or not its key is marked `card = true`.
