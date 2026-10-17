# Amended 2026-10-17: a Docker pool's runner is a confined container with a key of its own

**Proposed for the operator's ruling** (#2044, spec #2041; D-2044-1 to D-2044-6 in its pull
request). The text of §2, §3 and §8 is left as accepted, and this note adds to them. Items 1, 2
and 4 apply the container preset of §2 and the identity bullet of spec #2041; item 3 departs
from §3 and §8 for a headless runner, and is what the ruling is asked for.

1. **A Docker pool's runner is a container purlis makes, confined the same way every time**
   (`purlis_core::dockerpool`): the runner image by its digest, never a tag; Docker's default
   seccomp and AppArmor profiles, never named, never `--privileged`; every capability dropped
   and none added; no new privileges; the image's own user, by number, never root; its home on
   a volume of its own; no folder, device or socket of this machine; its own network on
   Docker's default bridge.
2. **The container is checked before it is paired and before every link.** What
   `docker inspect` says of it must be exactly item 1, for the container id, image and user
   pinned when it was made; anything else, a field that cannot be read included, refuses the
   link, and a container that fails the check as it is made is removed with its volume.
3. **A headless runner keeps its link key in a file of its own** (`<config>/purlisd/link-key`,
   `0600`, in the session host's `0700` folder that chats are denied), made at its first
   pairing and never replaced, where §3 and §8 put it in the age-encrypted form V1 gives the
   device key. Each runner, each container of a Docker pool, has its own key.
4. **The connector is §2's container preset with the user and group by number**
   (`docker exec -i --user <uid>:<gid> <container id> /usr/local/bin/purlis bridge`), so no
   name in the image's own files decides who it runs as; pairing is `purlis bridge --pair` on
   the same connector, which pins the desktop as the runner's one peer in `<config>/peers.json`
   and answers the runner's device id and public key, which the desktop pins.
5. **`purlis serve` takes links on a socket of their own** (`purlisd.link.sock`, beside its
   socket, `0600`), which `purlis bridge` joins its standard input and output to; the
   handshake ends in the host, as §2 says, and a connection from inside a chat is closed unread.
