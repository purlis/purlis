### Added

- **A pool can run its chats on Docker on another machine, reached over SSH.** In Pool setup, choose Another machine with Docker. Then pick the machine from your SSH settings or type its address, check the key it shows, and trust it. purlis signs in with your own SSH agent, opens no port on that machine, and connects to nothing there if its key or its Docker ever changes. A dropped SSH connection reconnects by itself. From a terminal, `purlis pool add <name> --image … --on ssh://you@host --trust SHA256:…` does the same. Your saved logins never reach such a pool yet (#6266).
