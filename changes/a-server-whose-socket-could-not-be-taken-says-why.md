### Fixed

- **A server that did not start on Linux says why.** Where purlis's Linux sandbox could not take
  a chat server's socket, because the machine refuses the call that takes it (a runner added by
  an older purlis, or a runner on Kubernetes), the Notice now says so and what to do: remove the
  runner and add it again, or start the chat without the sandbox. A runner on Kubernetes cannot
  be given that call yet (#3440).
