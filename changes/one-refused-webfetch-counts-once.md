### Fixed

- At the Full sandbox, one refused `WebFetch` counted twice in Blocked, in the Blocked log and in the Chat log, once for the proxy's refusal and once for the web hook's own deny. Both are still in the event log; the count, the list and the rows now hold the refusal once, and the hook's deny is left out only when the proxy's own block for the same host is recorded beside it. A `WebFetch` whose refusal left no such block (the proxy was at its limit or could not reach the host, the host was kept blocked and retried, or the ask is still open) still counts, once.
