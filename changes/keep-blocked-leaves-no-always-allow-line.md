### Fixed

- **Keep blocked holds at once, and leaves no tool line behind.** Keep blocked on a vault takes
  back the chat's Run grants on it at once, even while another answer waits on the session host.
  An Always allow pressed after it, or while it is carried out, writes nothing. The Allowed list
  reads again when Keep blocked takes back a chat's Run grants, or a run's notice gives one. Keep
  blocked through a vault's own notice no longer logs a warning about asks it closed (#5617).
