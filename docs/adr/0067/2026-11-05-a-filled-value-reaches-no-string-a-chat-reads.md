# Amended (2026-11-05, #5310): a filled value reaches no string a chat reads

Amends [A filled page holds its browser to its site](2026-11-04-a-filled-page-holds-its-browser-to-its-site.md).

A page's pending requests were said to the chat by their whole addresses, and a page that sends
what a field holds as it changes put a filled value there. Scrubbing that line alone leaves the next
field to skip it, so:

- **One way out.** Every answer and refusal the agent's calls get leaves the browser through one
  function, whose handling names each field with no catch-all, so a field added later does not
  build until it says what it does. That covers a page's address and title, the requests it waits
  on, what it lists, what a find or a wait reads, downloads and their names, notes and every
  refusal's own strings. A runner's browser is the same engine and takes the same way out (SF-12).
  The window composes its own lines only from those answers and never holds a value to scrub with.
- **A pending request is named by its host and counted, in every browser.** Never its path or
  query: the chat has no use for them, and they can hold other tokens than the ones purlis knows.
- **Whole-value forms, in any string.** A value is taken out as written, percent-encoded with any
  choice of characters and either case of hex, form-encoded, encoded many times over, in hex, in
  base64 of either alphabet with or without padding, inside a larger text's base64 at each
  alignment, and as a file name spells it, in a page's text as in its addresses and in what a
  screenshot paints over. Still never a part of a value (D-5115-13). A value too short to scrub
  keeps its rule, in every form.
- **What the chat wrote is said back as written.** A reference, a page's name, a key or an option
  it gave holds nothing it did not have, and blanking a guess there would tell it the guess was
  right.
- **What can't be scrubbed isn't said.** A site held back whose address holds a value isn't named
  and isn't asked about; a field's origin that holds one is an origin the engine can't name, and
  nothing fills there; a download name as long as names are cut to says `download`.
- **The notice says what the site can do.** The person approves knowing the site receives the
  value and can send it to its servers.

*Decided in implementation:* D-5310-1 to D-5310-6 in the pull request.
