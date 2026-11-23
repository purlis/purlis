### Fixed

- At the Full sandbox, one refused `WebFetch` counted twice in Blocked and in the Blocked log, once for the proxy's refusal and once for the web hook's own deny. Both are still in the event log; the count and the list now hold the refusal once. A `WebFetch` the proxy never answered still counts, as the hook's deny is then its only record.
