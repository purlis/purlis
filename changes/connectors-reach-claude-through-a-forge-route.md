### Fixed

- **MCP servers and claude.ai connectors (GitLab, Slack, Atlassian) work again in a chat that has a forge route.** Once a chat's GitLab sign-in turned its forge route on, they failed with "Proxy refused to open a tunnel: 403", because Claude Code's own hosts and the claude.ai MCP proxy are served from a CDN that shares addresses with forge hosts. Those hosts are now carried at any address; a forge host is still refused a direct tunnel.
