### Security

- **A Node package's `exports` and `module` are protected as its `main` is.** Where a project's
  hook loads a package from `node_modules` first (`node -r <package>`), the files its manifest
  maps through `exports`, under every condition, and its `module` are kept from a sandboxed chat
  as the file its `main` names already was (#1425).
