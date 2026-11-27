### Fixed

- **The new-chat picker opens quickly in a project with many personas.** It read the project's
  policy again for every persona, two git runs and a check of what you accepted each time, so
  with 38 personas its first answer could take 45 seconds. It now reads it once per answer.
