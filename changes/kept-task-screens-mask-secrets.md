### Security

- A dispatched task's kept screen no longer keeps purlis's own secrets. Before the screen is
  written, every vault value purlis handed that chat (through `purlis secret exec`, a runner's
  tool, its profile's sign-in or a browser fill) and every GitHub App token it minted for it is
  replaced by a named mask, such as `‹secret: github-token›`, so *Read it* shows which entry was
  there and never its value. A screen whose secrets purlis could not know is not kept (#6309).
