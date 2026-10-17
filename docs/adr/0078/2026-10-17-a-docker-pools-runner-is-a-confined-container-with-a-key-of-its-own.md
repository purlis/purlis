# Ruled 2026-10-17: a Docker pool's runner is a confined container with a key of its own

**Ruled by the operator on 2026-10-17**, on the questions #2044 raised (D-2044-4, D-2044-5 and
D-2044-7 in pull request #2505). The text of §2, §3 and §8 is left as accepted, and this note adds
to them. Item 3 amends §3 and §8 **for runners only**; a desktop's link key stays in the keyring.
Items 1, 2 and 4 apply §2's container preset and spec #2041's identity bullet.

1. **A Docker pool's runner is a container purlis makes, confined the same way every time**
   (`purlis_core::dockerpool`): the runner image by its digest, never a tag; Docker's default
   seccomp and AppArmor profiles, never named, never `--privileged`; every capability dropped
   and none added; no new privileges; the image's own user, by number, never root; a private
   IPC and cgroup namespace; its home on a volume made for it alone; no folder, device, port or
   socket of this machine; its own network on Docker's default bridge, and no other.
2. **The container, its home volume and the daemon are checked before the runner is paired and
   before every link.** What `docker inspect` says of the container must be exactly item 1, for
   the container id, image and user kept when it was made; its home volume must be Docker's own
   `local` volume with no options; the daemon must give containers its built-in seccomp
   profile. Anything else, a field that cannot be read included, refuses the link. A runner is
   not made where a volume of its home's name is already there, and one that fails the check
   as it is made is removed with its volume.
3. **A runner keeps its link key in a file of its own**, `<config>/purlisd/link-key`, `0600`
   and owned by the runner's user, in the session host's `0700` folder that every chat's
   sandbox denies, where §3 and §8 put it in the age-encrypted form V1 gives the device key.
   Each runner, each container of a Docker pool, has its own key.
4. **The connector is §2's container preset with the user and group by number**
   (`docker exec -i --user <uid>:<gid> <container id> /usr/local/bin/purlis bridge`), so no
   name in the image's own files decides who it runs as.
5. **Pairing is `purlis bridge --pair`, on the same connector** (§3: pairing happens once, over
   the connector, whose own login vouches for it). It is the bridge's one act that is not a
   pipe, so §2's *"the bridge holds no key and makes no decision"* reads: **except to pair**.
   It reads the desktop's device id and public link key on its standard input, makes the
   runner's key and its peers file, each only where no file of that name is, and prints the
   runner's device id and public link key, which the desktop keeps. A runner is paired once:
   a home that already holds either file is refused, whichever desktop asks.
6. **`purlis serve` takes links on a socket of their own** (`purlisd.link.sock`, beside its
   socket, `0600`), which `purlis bridge` joins its standard input and output to; the
   handshake ends in the host, as §2 says, and a connection from inside a chat is closed before
   any of it is read.
