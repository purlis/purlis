### Security

- **Fetches go only to the remote you saved to.** The window's own fetches and pulls (auto-save's
  fetch, a repo sync, a restore) now use your purlis sign-in only for the remote recorded for
  the clone, as automatic pushes already did. A changed remote is not fetched from, and the
  Saving view asks about it. A remote whose push address differs from its fetch address is
  never signed in for (#1851).
