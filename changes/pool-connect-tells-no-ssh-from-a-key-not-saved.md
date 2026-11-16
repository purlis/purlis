### Fixed

- **Connect says whether SSH answered at all before it blames the machine's key.** When nothing
  answers as SSH on the machine's port, or through the jump host your SSH settings name, Connect
  now says so and marks the machine's answer as the line that failed. Only when SSH did answer
  and its key couldn't be saved does it name the key, in plain words, without the refused sign-in
  every first contact ends with. See what this does now runs purlis's own first contact, so it
  shows why a key wasn't saved.
