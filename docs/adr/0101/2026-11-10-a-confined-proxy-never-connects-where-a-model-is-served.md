# A confined proxy never connects where a model is served

**Amends [ADR 0101](../0101-the-skill-studio-is-built-from-six-extension-capabilities-and-an-extension-that-declares-one-is-confined.md), 2026-11-10**, ticket #3094 (SK-14's follow-up from #3088). The amendment of 2026-10-24 said the proxy holds a tunnel's server name to the host it was asked for and cannot tell what serves that name. This adds the check by address, and says what is left.

## What the proxy checks besides

- **No address a model provider's hosts resolve to.** A confined extension's proxy looks up the names a model is reached by: each domain the studio counts as serving a model, its `api.` host where the domain is two labels, each host of the model-providers preset, and Amazon Bedrock's runtime in its main regions. An address of a declared host that one of those names answered with is not connected to. A declared host with other addresses is reached at those; one with no other is refused, as a Block, saying it resolves only to addresses a model provider is served at.
- **Looked up at the same moment, and remembered a while.** The names are looked up when a connect needs them and their answers are thirty seconds old or more. A connect waits for a lookup that is due, so none is let through on an answer still pending. An address stays refused for ten minutes after the last answer that held it, so a provider whose answers rotate is refused at each address it gave lately. Either spelling of an IPv4 address counts as one.
- **One set for the run.** Every confined extension's proxy asks the same set, so the names are looked up once however many extensions ask. A declared host that does not resolve is refused before any provider name is looked up.

## What it still does not stop

- **A host that relays.** A declared host can pass on what it is sent, to a model or anywhere else. Nothing the proxy sees tells that apart, and nothing short of not carrying the host would.
- **A front end that routes by something other than the server name**, at an address the provider's own names do not answer with here and now. The proxy reads the first TLS record and no more, so what a tunnel asks once encrypted is the client's to say. Where such a front end shares its addresses with a provider's names, the check above refuses it; where its addresses differ, it is not told apart.
- **A provider address its names do not answer with here.** A provider serves from more addresses than any one lookup returns, and its answers differ by place and by time. An address it serves at that no recent answer held is not refused.
- **A provider the list does not name**, or a regional host it does not spell out.
- **It costs some ordinary hosts.** A declared host behind the same shared front end as a provider can be given the same address at the same moment, and is then refused there. The check fails closed: such a host is reached at its other addresses, or not at all.
- **It waits on the lookups.** A resolver that is slow to answer holds up a connect by as much, since a connect is never let through before a due lookup answers.

**Built by #3094.**
