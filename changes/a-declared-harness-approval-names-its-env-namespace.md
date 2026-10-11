### Security

- **Approving a harness a project declares shows the variables it would pass.** The approval
  line now ends with the declaration's `env` names and prefixes ("it passes `MYTOOL_*` from your
  environment"), beside the words it runs and the bytes it types. An approval is tied to the
  declaration's exact contents, which already covered `env`, so one you gave before still holds
  (#969).
