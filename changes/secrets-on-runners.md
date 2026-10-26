### Added

- **A runner chat's tools get their vault values from your desktop, one run at a time.** When a
  chat on a pool's runner runs one of its persona's declared tools, the runner finds the tool it
  armed, and your desktop checks it is declared and allowed here, that the pool is yours (for now,
  a Docker pool on this machine's own Docker), and asks you once per tool and pool. Your answer
  holds while purlis runs and the pool, its runner and its image stay as they are. The command
  waits for your answer, then runs on the runner with the values your desktop sent for that run;
  the runner keeps none. With your desktop away the command waits up to 15 minutes, says so, and
  you are asked once you are back; the chat carries on meanwhile (#2047).
