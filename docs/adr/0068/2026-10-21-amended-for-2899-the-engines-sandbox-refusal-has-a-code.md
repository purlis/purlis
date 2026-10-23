# Amended for #2899, the engine's sandbox refusal has a code of its own, 2026-10-21

This takes the place of D-2835-1's sentence for a host at 1.36 or later.

1. **`no_engine_sandbox` (session protocol 1.36).** A browser whose engine cannot make its sandbox
   on the host's machine is refused `no_engine_sandbox`, its `why` the binary's words for it;
   any other failure to start is refused `not_started`, as before. The host is told which by the
   binary itself: `Browsers::start` answers `BrowserNotStarted::NoEngineSandbox` or `Failed`,
   never a sentence to match. The window reads the code first, and still takes the sentence
   from any host as the same refusal: a host before 1.36 refuses it `not_started` in those words,
   and a relaying host may pass the words on without the code. *D-2899-1, decided in
   implementation, to be confirmed.*
2. **Minor 1.36, gated before 1.34 and 1.35.** The minor is the one given in merge order; the
   two before it are other changes, and this one names no frame they add. *D-2899-2, given by the
   steward.*
