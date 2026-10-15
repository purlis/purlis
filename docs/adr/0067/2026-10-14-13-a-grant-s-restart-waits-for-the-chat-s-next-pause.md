# Amended (2026-10-14, #1986, the operator's ruling Q63; amends section 3): a grant's restart waits for the chat's next pause and says what it will end

1. **The restart a folder, a socket or a service needs waits for the chat's next pause between
   turns**: its turn ended or it handed control back, as its harness's hooks already say
   (`waiting`). Mid-turn it waits; a harness that says nothing of its turns waits for the person.
   A chat already between turns as the person allows is at its pause, and restarts then. Every
   grant made before the pause takes one restart, and a restart still owed when the chat raises
   another ask stays owed. **A chat whose program ends first is restarted by nothing** and owed
   nothing; what was allowed reaches it at its next start. Only the program's end, which the
   core hears, drops what is owed: a hook that says the session ended while its program runs on
   drops nothing. **Restart now** restarts it
   at once, mid-turn or not. The window decides through one pure scheduler
   (`app/src/restartAtPause.ts`); the core drops what an ended chat was owed. The grant itself is
   unchanged.
2. **The Ask says beforehand what the restart will end**: each program below the chat's program
   that listens on a TCP port, by name and port, and how many others its commands started are
   running ("its node on port 5173 will stop"), read from the operating system's process table
   and listening sockets (`purlis_core::restartends`), never from what the chat printed. A
   command's programs are the ones below a shell the chat's program started, so the harness's
   own servers, which start again with it, are not counted. Where purlis cannot read either, the
   Ask says it cannot tell, and guesses nothing; so does one where either takes longer than
   two seconds. The words say whether the chat restarts now, being between turns, or at its next
   pause. They are on the chat's own Ask, on the same ask's row in the Inbox, naming the chat
   ("restarts steward 3 at its next pause"), and, for each task listed, on a folder's question
   for several tasks.
