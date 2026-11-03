### Added

- Any chat can ask you for a credential it lacks, declared or not, with the `secret_ask` tool
  or `purlis secret ask <KEY> --purpose "<what for>"`, and every chat is told to at its start
  instead of asking you to paste one. The ask shows the chat's purpose in its own words, says
  when the persona does not declare the key, and goes to the persona's vault unless the chat
  names another. A key the vault holds already answers at once. The chat hears only filled
  (under the name you stored it as) or declined, never the value.
