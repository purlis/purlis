### Security

- **An ask no longer shows a vault value purlis knows.** A value the app read or stored since
  it started (one you revealed or stored in its window, or one it handed a sandboxed chat's
  command) is masked as `[vault value]` in an ask's line, whatever it looks like, and an edit
  that would write one is not drawn. Such an ask offers no Allow in the Inbox: allow it in its
  pane, which shows it whole. A value the app has not read, one a chat read itself for
  instance, is not known to it and is still shown (#1047).
