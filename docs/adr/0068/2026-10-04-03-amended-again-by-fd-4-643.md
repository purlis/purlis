# Amended again by FD-4 (#643), 2026-10-04

The first FD-4 amendment's item 1 said there is no priority control lane, and named the fix if
the lane was found waiting too long: a scheduler in `link` that drains the control lane first,
behind the same interface. It was found waiting too long, and the scheduler is built. §4's text,
*"a priority control lane carries commands and events past a busy terminal"*, holds again, and
that item is withdrawn.

1. **Why.** Yamux takes one frame from each stream with something to send, in turn. So a control
   frame waits behind a 16 KiB frame of every busy stream. On a 1 MB/s link, a connector's stdio
   to a remote host, fifty busy streams kept a control frame waiting 908 ms. The scheduler
   brings that to 73 ms, which is the 64 KiB the link itself holds.
2. **How.** The multiplexer writes into the scheduler (`priority` in the crate), not the
   transport. The scheduler cuts the bytes into Yamux's own frames and sends the control lane's
   first: Yamux stream 1, the client's first, which the host checks is stream 1 before it takes
   it as the lane. The session's pings and pongs go with the lane, as Yamux puts a pong ahead of
   the streams, so the round trip Yamux measures is not the scheduler's queue. The other streams
   take turns, one frame each, as Yamux's own streams do, and a go-away goes last.
   - **Two bounds.** The streams' frames are bounded at 16 MiB (256 streams × a view's 64 KiB
     watermark), and the lane's, pings and pongs included, at 2 MiB (two of its largest
     frames). Each frame counts at least 64 bytes against its bound, what a queued frame holds
     of the heap, so a flood of 12-byte pongs is bounded in memory, not only in bytes on the
     wire. Past either bound, the scheduler takes no next frame of any kind until it has made
     room, and that includes a frame with no body. That is the transport's back-pressure,
     passed on: Yamux stops writing, and stops reading once a pong or a window update waits.
     So a peer that grants credit and never reads, or floods pings, cannot grow either queue.
   - **What that costs.** Past a bound, **everything waits, the lane too.** Yamux has one
     frame on its way at a time, so it cannot hand over the lane's frame while another is
     stuck. In use the bounds are not reached, since the windows and watermarks hold what is
     in flight well under them.
   - **A dropped link** writes what it was handed and shuts the transport, for at most
     10 seconds.
3. **The wire is unchanged, and so is the version.** Every frame is Yamux's, whole, and each
   stream's frames keep their order. Only frames of different streams are reordered, and Yamux
   allows that, since each stream has its own window and sequence. A peer that does not schedule
   reads the same bytes. So the protocol stays 1.0, and no capability is negotiated: there is
   nothing for the other end to agree to.
4. **What it cannot jump** is what the transport already holds: a socket's send buffer, a pipe,
   `ssh`'s channel window. That wait is bounded by the windows and watermarks the first FD-4
   amendment lists. Sizing a remote link's watermarks to its round trip is #930.
5. **Measured** on the crate's deterministic link simulator (`tests/common/netsim.rs`): tokio's
   virtual time, seeded loss, a rate, a delay and a buffer. The ends' own work takes no time
   there, so the delays are the protocol's and the link's, and the tests assert them; ADR 0086
   keeps only wall-clock budgets out of `cargo test`. Fifty busy chats on a 150 ms, 2% loss,
   10 MB/s link with 2 MiB in it: a needs-you at a p50 of 227 ms and a worst of 386 ms, where
   Yamux alone gave 262 and 455, within #643's 1 s plus the round trip. The heap peaked at
   57 MB of the 90 MB the limits allow. `charter-session-bench`'s keystroke under ten flooding
   panes costs about 20 µs: a median of about 0.176 ms against main's 0.156 ms (+13%), inside
   the `bench` job's 1.20 gate. The same over TCP under
   kernel netem is still SC-21 (#828).
