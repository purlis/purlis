### Fixed

- **Keep blocked holds at once.** Keep blocked on a vault, pressed on its Notice, in the Inbox or on
  a run's own notice, takes back the chat's Run grants on it at once, even while another answer
  waits on the session host. From then on no answer gives the chat that vault or lets its run go
  on, and no Always allow gives the persona the vault. The Allowed list reads again when Keep
  blocked takes back a chat's Run grants, or a run's notice gives one. Keep blocked through a
  vault's own notice no longer logs a warning about asks it closed (#5617).
