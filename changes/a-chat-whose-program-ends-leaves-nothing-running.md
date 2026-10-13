### Fixed

- **A chat whose program has ended leaves nothing of it running.** Ending, stopping or closing
  a chat always ended everything its program had started, as long as the program itself was
  still running. A program that ended by itself (you quit it, or it left on the interrupt that
  Stop sends first) left the rest to the hang-up the system sends, and whatever ignored that
  ran on with nothing left to end it: a server the chat had started, for one. purlis now ends
  what such a program left in its process group as it finds the program gone: hung up, given
  half a second, then killed. A process that put itself in a session or a group of its own is
  not reached, as before.
