# An extension's template lands as a draft

**Amends [ADR 0101](../0101-the-skill-studio-is-built-from-six-extension-capabilities-and-an-extension-that-declares-one-is-confined.md), 2026-11-09**, ticket #3091 (SK-14's follow-up from #3088). §1 says a SkillTemplate's files become a draft skill the person accepts. #2777 built it before SK-9's drafts, so an extension's template made the skill at once, checked as whole like purlis's own. With drafts in, this does what §1 says.

- **New skill… from an extension's template keeps a draft** (`verbs::new_skill`), in the window and at `purlis skill new --extension-template`: the same draft a chat's write makes (SK-9), in this machine's app state, a new skill, tried and accepted or discarded in Drafts. Nothing in the project is written until Accept, which commits it as it commits a chat's.
- **purlis's own templates still make the skill at once**, in one commit, with its page opened to edit: their text is purlis's.
- **The checks are made before the draft is kept**, as for every new skill: the name, the description, a whole skill that keeps its name, and a level that does not hold it. Accept makes its own again.
- **The draft says which template it came from**: `draft.json` keeps the template as New skill… offered it (`template`), and Drafts says "drafted by the template …" where a chat's draft names its chat. The label is the extension's words, so the window shows it escaped.
- **A chat's write to it makes it the chat's**: Drafts names the chat, and the record no longer names the template. **Accept of one the project changed since says to make it again with New skill…**, as there is no chat to ask.
- **A template never writes over a draft that waits.** Where the skill has a draft already, a chat's or a template's, New skill… is refused and says to accept or discard it first.
- **`verbs::create` refuses an extension's template**, so no path makes one at once.
- **The window says it before it is asked**: with an extension's template picked, New skill…'s summary says the skill waits in Drafts, and its button is **Keep it as a draft**. Once kept, the studio opens the draft in Drafts.
- **`skill-template` stays experimental.** This closes #3091, which the amendment of 2026-11-02 named as what keeps it out of version 1, but joining version 1 is an amendment of its own: the word's shape is unchanged here.
