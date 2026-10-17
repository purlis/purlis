# Amended for #2445, a relayed plan fits the runner, 2026-10-17

The amendment for #2217 lets a runner's host start a chat's confinement as the desktop's host
relays the window's plan. This note says what the runner holds that plan to. The operator's
rulings of 2026-10-17 on #2217 stand as ruled, and the amendments for #1999, #2217 and #2444
are left as accepted.

The operator ruled two things for this note:

- **2026-10-17.** A runner refuses any private, cluster or link-local address in a relayed
  plan, a name that resolves to one included, unless the runner's own administrator's policy
  lists it. Public hosts work as planned.
- **2026-10-18.** That policy is a key of its own, `sandbox.relayed-internal`, in the runner's
  machine policy. It lists only the internal services relayed chats may reach, and
  `sandbox.hosts` keeps its meaning.

Item 2 is that ruling. The rest of item 1, and items 3 to 5, are **decided in implementation,
to be confirmed** (D-2445-1, D-2445-3 to D-2445-5).

1. **A plan made on another machine fits the runner, or nothing starts.** A `confine` from any
   link but the host's own window (`local-ui`) is checked against the project as the host holds
   it, before anything starts:
   - a Claude Code chat's ssh route goes in the runner's own folder for the project's routes;
   - every service of the runner the plan gives the chat is one the runner's own sandbox for
     the project gives a chat it starts. That means yours for the project on the runner, at
     the scope policy leaves open, with no grant of another machine's;
   - `127.0.0.1` and `::1` are listed only on such a port. The rest of the loopback network,
     `127/8`, is an internal address, below;
   - an internal address is listed only where the runner's policy lists it (item 2), and so is
     a listed name that the runner resolves to one. An internal address is:
     - one of a private network (`10/8`, `172.16/12`, `192.168/16`);
     - a cluster's (`100.64/10`, IPv6 `fc00::/7` and `fec0::/10`);
     - a link's;
     - one of the runner's own interface addresses;
     - one on a network the runner's interfaces are on, an IPv6 pod or node network included;
     - every loopback address but `127.0.0.1` and `::1`.

     An IPv6 address that carries an IPv4 one is judged as that one. That covers mapped,
     SIIT, NAT64 `64:ff9b::/96`, NAT64's local-use `64:ff9b:1::/48` as a `/96` in it,
     compatible, and 6to4 addresses. A cloud metadata service is never carried;
   - where the runner cannot read its own networks, nothing fits.

   The same internal-address rule is applied again at every connect the chat's proxy makes,
   whatever the plan lists, because a name can resolve to another address later. The policy is
   read again at each connection, so a policy narrowed while the chat runs holds its next one.
   A name under `*.` is checked at connect alone. Public hosts, the never-hosts and the
   withdrawn ports are the window's, as planned. *D-2445-1, ruled in part.*
2. **The runner's policy for relayed chats is `sandbox.relayed-internal`** (ruled 2026-10-18).
   It is a list of hosts in the runner's machine policy file, written as `hosts` is, and it
   names the only internal services a relayed chat reaches. It gives nothing to a chat the
   runner plans itself. `sandbox.hosts` keeps its meaning and opens no internal address to a
   relayed chat. A name or a `*.domain` entry trusts DNS: it carries whatever internal address
   that name resolves to on the runner. A list that must not depend on DNS names addresses and
   ports.
3. **One that does not fit is refused `not_allowed`.** The sentence starts "the runner does not
   start the chat's confinement" and names what does not fit. `not_allowed` already says that
   what a link asks is not the host's to give. A chat whose proxy is refused an internal address
   at connect is told it as a local address is. *Rejected:* `not_started`, which reads as a
   start that failed; a refusal code of its own, which is a new word on the wire. *D-2445-3.*
4. **The runner keeps the plan as it is: it does not map it onto its own folders and ports.**
   The runner already chooses every port it listens on for a confinement (its proxy's, answered
   in `confined`) and every folder it makes beside it. A given port names a service that already
   listens on the runner, so there is nothing to map it to. *Rejected:* rewriting the plan's
   ports and routes folder to the runner's, because the window would then show a reach the
   chat does not have. *D-2445-4.*
5. **The window's plan on its own host, and a plan the host makes itself, are started as
   planned.** On its own machine the window plans from the same project files, with the grants
   it keeps for each chat. A confinement the host plans for a chat it composes by its names is
   compiled there already (the amendment for #2171). *D-2445-5.*

**Until #2510:** a service the person allowed for one chat alone, in the window, is not among
the runner's. A runner chat whose plan gives such a port is refused, and nothing starts.

The binary checks the plan through the host's `Confine` seam (`fits`), which every binary
implements, and starts it as planned elsewhere (`ToConfine::planned`). The app's and `purlis
serve`'s implementations both use the core's `sandbox::hosted::fits_here` and
`sandbox::hosted::start`.
