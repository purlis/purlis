# Amended (2026-11-13): a server that refuses the forward is found by asking it for a channel

Records the choice [#6349](https://github.com/purlis/purlis/issues/6349) made. It adds a row to
§7's SSH findings. The rest of this record stands.

## What goes wrong

An SSH server can sign the person in and still refuse to forward the engine's socket:
`DisableForwarding yes`, `AllowStreamLocalForwarding no`, or `no-port-forwarding` on the key in
`authorized_keys`. `AllowTcpForwarding no` does not refuse it, because it applies only to TCP.
The pool's `ssh -T -L` opens all the same, because a local forward binds on this machine alone.
The server refuses only the channel of each connection made through the forward. So Docker's
client meets a closed connection, and Connect said only that Docker didn't answer there.

## How it is found

The check runs only when the engine doesn't answer through a forward that opened: at Connect's
Trust this machine, before a pool's runner is made, checked or linked, and in the look again at a
forward that won't open. It is one more `ssh`, pinned like the others, that asks the server for
a single channel to the engine's socket with `-W <socket>`. `-W` runs nothing there and exits
255 if the channel is refused. Its standard input is empty, so an opened channel ends at once.

ssh also exits 255 when the sign-in is refused. So a 255 counts as a refused forward only when a
sign-in made right after it is let in. What ssh said is never read. The finding is
`forwarding-refused`. It names the server and tells the person to ask its admin to allow
forwarding for their account (`AllowStreamLocalForwarding`). The window shows it on the line
Docker answers there, as Docker's other causes are shown.

A pool whose forward is refused lets go of the forward and waits, with the same back-off and the
same remembered finding as a forward that won't open (`sshroute::pool::Failing`). So a surface
that keeps asking doesn't contact the machine at every ask.
