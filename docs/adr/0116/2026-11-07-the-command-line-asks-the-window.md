# Amendment, 2026-11-07: the command line asks the window, and holds no sign-in

The person's ruling on #5625 (option A of a quiz). Decision 5 says the app adds the key
"through the forge account", and #5607 says every forge call uses the account's own token. ADR
0070's ruling 5 says the `purlis` binary never holds a forge sign-in. That ruling stands, and
here is how the two fit.

- **`purlis signing` holds no sign-in and never runs `gh`.** Its reads (whether the account has
  this device's key, whether a pushed commit shows as Verified) go to the running window over
  the window line (`purlisd-window.sock`, #4141). The window asks the forge with its own sign-in,
  as itself, the person in it, and answers with what the forge said
  (`purlis_core::signingsetup`). Only reads go this way, and what comes back is a yes or no, or
  the forge's verdict on a commit and its reason; never a token. Adding or removing a key goes
  only from the window's own setup, behind the person's click there.
- **From the command line, the key is added by hand**: the steps name the forge's page, the
  title and the key type, and setup goes on by itself once the window hears the forge has the
  key (or on Enter). Only the window's
  own setup adds the key through the account (#5627, #5628).
- **With no window running**, setup gives the by-hand steps and says to open purlis, and
  `status` and `check` say the same. `setup --by-hand` is gone, since by hand is the only way
  from the command line.

So in #5607, "the forge account's token" means the window's.
