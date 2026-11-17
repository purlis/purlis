### Security

- A dispatched task's kept screen no longer keeps purlis's own secrets. Before the screen is
  written, every vault value purlis handed that chat (through `purlis secret exec`, a runner's
  tool, its profile's sign-in, a browser fill or a browser login) and every GitHub App token it
  minted for it is replaced by a named mask, such as `‹secret: github-token›`, so *Read it*
  shows which entry was there and never its value. A value of several lines is masked whole and
  line by line. Values shorter than 4 characters are not masked. A screen whose secrets purlis
  could not know is not kept, and a value purlis could not note for masking is not handed
  (#6309).
