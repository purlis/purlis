### Security

- **A keyring read in a sandboxed chat is judged by where it runs too.** Besides its environment,
  purlis asks which chat a command runs below, and sends a keyring vault's read through the app
  unless that chat's record says it ran without the sandbox. Where purlis cannot tell, it reads
  nothing and says why. Your own terminal, an unsandboxed chat and a shell tab read as before.
