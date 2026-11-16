### Fixed

- **Connect says whether SSH answered at all before it blames the machine's key.** When nothing
  answers as SSH on the machine's port, Connect now says so and marks the machine's answer as the
  line that failed. Only when SSH did answer and its key couldn't be saved does it name the key,
  in plain words, without the refused sign-in every first contact ends with. Through a jump host
  or proxy, where purlis can't tell what stopped it, Connect names the jump host, the machine and
  saving its key alike and marks no line as failed. See what this does now runs purlis's own first
  contact, with the same settings, so it shows where it stops and why a key wasn't saved.
