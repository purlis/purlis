### Security

- **A project's `purlis.toml` is read only as a plain file.** When the Settings tab reads or
  saves it, a file that is a link, or a FIFO, is refused at once with a sentence. Before, a
  link's target was shown and rewritten in its place, and a FIFO held the read until something
  wrote into it (#1100).
