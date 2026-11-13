# Amended (2026-11-13): a server that refuses the forward is found by asking it for a channel

Records the choice [#6349](https://github.com/purlis/purlis/issues/6349) made. It adds a row to
§7's SSH findings. The rest of this record stands.

## What goes wrong

An SSH server can sign the person in and still refuse to forward the engine's socket:
`AllowTcpForwarding no`, `AllowStreamLocalForwarding no`, `DisableForwarding yes`, or
`no-port-forwarding` on the key in `authorized_keys`. `AllowTcpForwarding no` refuses it although
the socket isn't TCP: sshd turns off all of its local forwards, the socket's too. The remote parity
job sets each of the three.
The pool's `ssh -T -L` opens all the same, because a local forward binds on this machine alone.
The server refuses only the channel of each connection made through the forward. So Docker's
client meets a closed connection, and Connect said only that Docker didn't answer there.

## How it is found

The check runs only when the engine doesn't answer through a forward just opened. That happens
at Connect's Trust this machine, and before a pool's runner is made, checked or linked, once for
each time its forward opens. It is one more `ssh`, pinned like the others, that asks the server
for a single channel to the engine's socket with `-W <socket>`. `-W` runs nothing there and exits
255 if the channel isn't opened. Its standard input is empty, so an opened channel ends at once.

ssh also exits 255 for other reasons:

- a sign-in it was refused
- a lost connection
- a channel the server opened to a socket that took no connection

So a 255 counts as a refused forward only when the probe, run right after it, signs in and finds
the socket there and usable (`PROBE`). What ssh said is never read.

One case still can't be told apart. On a machine that does forward, an engine that ended without
removing its socket leaves a file that passes the probe and takes no connection. That is said as
a refused forward, and Check again asks again.

The finding is `forwarding-refused`. It names the server and tells the person what to ask its
admin for: `AllowTcpForwarding`, `AllowStreamLocalForwarding` and `DisableForwarding` in
sshd_config, or `restrict` and `no-port-forwarding` on their key. The window shows it on the line Docker answers there, as
Docker's other causes are shown.

A pool whose forward is refused lets go of the forward and waits, with the same back-off and the
same remembered finding as a forward that won't open (`sshroute::pool::Failing`). So a surface
that keeps asking doesn't contact the machine at every ask. A forward that won't open at all is
never asked this, because a refusing server doesn't stop it from opening.
