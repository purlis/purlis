### Added

- **A chat blocked three times on the same thing is paused instead of looping.** When one of
  purlis's guards or the sandbox refuses the same call (the same tool with the same arguments)
  three times with no other call succeeding in between, the chat is held still and Asks says it
  may be looping, as it does for six identical failures or six of your Don't allows. An Allow,
  another call that succeeds, or your answer that it may go on starts the count again. The
  harness's own permission check never counts, nor does a report purlis cannot verify (a chat
  whose hooks run inside its own sandbox), nor a merge waiting on you in Asks. A reworded
  retry is a different call, so it starts a count of its own.
