### Security

- **More of a profile is filtered before it goes to a runner** (#3866). An agent's or a skill's
  frontmatter no longer carries environment or headers, a URL no longer carries a user or password,
  and a hook, status line or file suggester whose command shows a credential stays behind. What was
  removed is said in the log, by name and never by value.
