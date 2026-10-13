// `./home.ts` and this file differ only in case: the extensions keep a case-insensitive disk
// from handing one back for the other.
import { grouped, needsYou, projectOf, stateWord, titleOf, type Listed } from "./home.ts";
import type { Chat } from "./ipc.ts";
import { Notice } from "./Notice.tsx";

/**
 * Home (iOS design §6.1): what needs you first, across every desktop, then each desktop's
 * chats by workspace. While a desktop is away its last list stays, marked stale (§6.3).
 */
export function Home({
  listed,
  onOpen,
  onRetry,
  onSettings,
  onPair,
}: {
  listed: Listed[];
  onOpen: (device: string, chat: Chat) => void;
  onRetry: (device: string) => void;
  onSettings: () => void;
  onPair: () => void;
}) {
  const queue = needsYou(listed);
  return (
    <main className="screen home">
      <header className="bar">
        <h1>purlis</h1>
        <button type="button" onClick={onPair}>
          Pair
        </button>
        <button type="button" onClick={onSettings}>
          Settings
        </button>
      </header>
      {queue.length > 0 && (
        <section className="needs-you" aria-label="Needs you">
          <h2>Needs you</h2>
          <ul>
            {queue.map(({ device, desktop, chat }) => (
              <li key={`${device}/${chat.chat}`}>
                <button type="button" onClick={() => onOpen(device, chat)}>
                  <span className="title">{titleOf(chat)}</span>
                  <span className="where">
                    {desktop} · {projectOf(chat)}
                  </span>
                </button>
              </li>
            ))}
          </ul>
        </section>
      )}
      {grouped(listed).map((group) => {
        const stale = group.desktop.standing !== "connected";
        return (
          <section
            key={group.device}
            className={stale ? "desktop stale" : "desktop"}
            aria-label={group.desktop.name}
          >
            <h2>{group.desktop.name}</h2>
            <Notice desktop={group.desktop} onRetry={() => onRetry(group.device)} />
            {group.workspaces.length === 0 ? (
              <p className="empty">No chats are open on {group.desktop.name}.</p>
            ) : (
              group.workspaces.map((workspace) => (
                <div key={workspace.name ?? ""} className="workspace">
                  {workspace.name && <h3>{workspace.name}</h3>}
                  <ul>
                    {workspace.chats.map((chat) => (
                      <li key={chat.chat}>
                        <button type="button" onClick={() => onOpen(group.device, chat)}>
                          <span className="title">{titleOf(chat)}</span>
                          <span className="where">{projectOf(chat)}</span>
                          <span className={`state state-${chat.state}`}>
                            {stateWord(chat.state)}
                          </span>
                        </button>
                      </li>
                    ))}
                  </ul>
                </div>
              ))
            )}
          </section>
        );
      })}
    </main>
  );
}
