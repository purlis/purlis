### Security

- **A brokered tool runs with an environment of its own.** A tool a chat runs with a vault's
  value starts from only the chat's language, terminal, time zone, ssh agent, certificate
  bundles and the plain values a client reads for where it connects (a database's host, port and
  name, a cloud's region). Its `HOME` and `XDG_*` folders are in the run's own temp folder. A
  setting the tool read from another variable of the chat, or from your home folder, is now
  given as one of its arguments. A persona's approved credentialed MCP server starts the same
  way, with the variables its approved entry sets (#2047).

- **A brokered run serves only chats of this machine.** A vault's tool is run on this machine
  only for a chat this machine runs (#2047).
