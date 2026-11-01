// PROTOTYPE, throwaway: draws the real window on a mocked core.
import "../../.storybook/handed";
import { install } from "./mock";
install();
import ReactDOM from "react-dom/client";
import App from "../App";
import { defaultCell, themes } from "virtual:purlis-handed";
import { DEFAULT_THEME, drawIn, fetchFacesWith } from "../theme/theme";
fetchFacesWith(async (asset) => {
  const { default: faces } = await import("virtual:purlis-faces");
  const text = faces[asset];
  if (text === undefined) return undefined;
  return Uint8Array.from(atob(text), (c) => c.charCodeAt(0)).buffer;
});
const t = new URLSearchParams(location.hash.slice(1));
const theme = themes[t.get("theme") ?? defaultCell.theme] ?? themes[defaultCell.theme];
drawIn((theme?.[t.get("look") ?? defaultCell.appearance] ?? Object.values(theme ?? {})[0] ?? DEFAULT_THEME) as never);
ReactDOM.createRoot(document.getElementById("root")!).render(<App />);
