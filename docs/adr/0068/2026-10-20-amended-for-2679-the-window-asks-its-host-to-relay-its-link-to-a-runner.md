# Amended for #2679, the window asks its host to relay its link to a runner, 2026-10-20

ADR 0078 §4 says the window reaches a runner through its own `local-ui` connection to its own
host, which relays. This note says how the session protocol carries that. The amendments above
are left as accepted, and this note adds to them. The minor was assigned on 2026-10-20; the rest
is *decided in implementation, to be confirmed* (D-2679-5, D-2679-6).

1. **Session protocol 1.33, a minor.** `relay_to {runner}` is answered `done`. It has a row of
   its own in FD-27's table (`Power::RelayTo`), granted to `local-ui` alone. It is a link's first
   command, before `subscribe`: the desktop's host then serves every later command on that link
   through a relay kept to the runner this machine keeps under that name (`purlis_host::Routed`),
   and a link that sent anything else first is its own and refuses a later `relay_to`
   `not_allowed`. A runner this machine does not keep is refused `not_started`, and the link is
   still undecided. A relaying host, and a host that relays to no runner, refuse it. A host
   before 1.33 refuses it `unknown_command`.
2. **A runner whose key changed is said so.** `link_down` gains `key_changed`, and a command a
   relay cannot send on is refused with the new code `key_changed`, where the runner at the far
   end answered with another key than the one pinned at its pairing: the window offers to remove
   it rather than waiting for it to come back. Both are recorded in the protocol's fixture.
