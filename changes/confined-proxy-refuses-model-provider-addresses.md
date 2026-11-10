### Security

- **A studio extension's proxy never connects where a model provider is served.** Besides the
  hosts that serve a model, a confined extension's proxy now refuses every address their names
  resolve to, looked up as it connects, so a host it declares cannot be pointed at a provider's
  address (#3094).
