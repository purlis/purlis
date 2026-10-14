### Changed

- **The commitment point waits for an ask.** purlis's prompt hook reads the first prompt of a
  harness session for a fork as before, and after that only a prompt that opens by asking for a
  code change ("refactor the importer…", "please add…", "ok, now fix…"). A short reply, the
  answer to a question and a design answer such as "agree at all, but …" no longer get its
  advice, so a design grill is not told to reproduce a failing test on every turn. It is the
  same under every harness that runs the hook (#1988).
