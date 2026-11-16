### Changed

- **Only a harness purlis ships routes a chat's model calls.** A project's harness declaration
  that sets a key under `[route]` is refused, naming the field, and a project's harness still
  starts on its own sign-in. A `[harnesses.<name>]` table in `charter.local.toml` that sets a key
  under `[route]` is refused, naming the field. One that replaces a harness purlis ships and sets
  `[sign_in] plans`, which worked before, is now refused where refused declarations are shown:
  purlis uses its own plans, and the harness stays available (T1a-37, #6787).
