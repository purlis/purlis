import vendored from "../icons/vendored.json";

/** Each vendored directory's licence text, by the directory `vendored.json` names it by. */
const LICENCES: Record<string, string> = Object.fromEntries(
  Object.entries(
    import.meta.glob<string>("../icons/*/LICENSE", {
      query: "?raw",
      import: "default",
      eager: true,
    }),
  ).map(([path, text]) => [path.split("/").at(-2) ?? "", text]),
);

/** The window event About sends to open the Licences tab in the project in front (#1719). */
export const OPEN_LICENCES = "purlis-open-licences";

/**
 * **Ask the window for the Licences tab** (#1719): About sits in the title bar, outside any
 * project, so the window opens it in the project in front. Answers whether one took the ask:
 * with no project open there is no strip to open a tab on, and About shows them in place.
 */
export function askLicencesTab(): boolean {
  return !window.dispatchEvent(new Event(OPEN_LICENCES, { cancelable: true }));
}

/** What the Licences tab is called. */
export const LICENCES_TITLE = "Licences";

/**
 * **What purlis ships that others wrote, with their licences** (FM-3, #1106): each vendored
 * asset (`app/icons/vendored.json`) and its licence text, compiled into the bundle so the
 * attribution travels with every copy of the app. Drawn in a view tab of its own (#1719), and
 * in About where no project is open to hold the tab.
 */
export function Licences() {
  return (
    <>
      {vendored.map((asset) => (
        <div key={asset.name}>
          <p className="honest">
            File and folder icons: {asset.name} {asset.version}, under the {asset.licence} licence,
            from {asset.source}.
          </p>
          <pre>{LICENCES[asset.files]}</pre>
        </div>
      ))}
    </>
  );
}

/** The Licences view tab (#1719). */
export function LicencesView() {
  return (
    <section className="licences" aria-label={LICENCES_TITLE} data-testid="licences-view">
      <Licences />
    </section>
  );
}
