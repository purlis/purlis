# Amendment, 2026-09-26: purlis's split windows get what the main window gets

A project tab can now be moved into a window of its own (charter#126, ADR 0033 amended the same
day). That window runs the same page, so without a grant it could invoke nothing, and it would
draw an empty window. Both capabilities now name the same two labels:

- `main`, and
- `window-[0-9]*`, the split windows. purlis makes them itself and numbers them
  (`windows.rs:split_label`); the page cannot make a window, because no capability grants it
  window creation.

**Both capabilities, the value-bearing one included.** A split window can have a vault's tab in
front, and a vault whose Reveal and Copy were refused in one window and not in the other would
be a vault that works depending on where its project is. The rule this record set still holds:
reveal and copy are in a capability of their own, and they are granted to purlis's windows and
to no other window. A window with any other label is still refused every command.

The tests change to match. `a_second_window_may_invoke_the_allow_listed_commands` checks that a
split window may invoke every listed command. `no_window_but_charters_own_may_invoke_any_listed_command`
checks that labels near the pattern (`window-`, `window-x`, `windows-1`, `another-window`)
still get nothing. `every_capability_grants_the_main_window_and_the_split_windows_alike` checks
that the two capability files name exactly the same windows, so that a split window never gets
half the app.
