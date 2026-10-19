import { StrictMode, useEffect, useState } from "react";
import { createRoot } from "react-dom/client";
import { api, viewName } from "./api";
import { Donut } from "./views/Donut";
import { Editor } from "./views/Editor";
import { History } from "./views/History";
import { Hud } from "./views/Hud";
import { Onboarding } from "./views/Onboarding";
import { Overlay } from "./views/Overlay";
import { Pin } from "./views/Pin";
import { Settings } from "./views/Settings";
import "./tokens/shared.css";
import "./tokens/mac.css";
import "./tokens/windows.css";
import "./styles.css";

function inDesktop() {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

function Shell() {
  const [ready, setReady] = useState(false);
  const desktop = inDesktop();
  useEffect(() => {
    if (!desktop) return;
    api.platform().then((platform) => {
      document.documentElement.dataset.platform = platform;
      setReady(true);
    }).catch(() => setReady(true));
  }, [desktop]);
  if (!desktop) return <BrowserNotice />;
  if (!ready) return null;
  switch (viewName()) {
    case "overlay": return <Overlay />;
    case "donut": return <Donut />;
    case "editor": return <Editor />;
    case "history": return <History />;
    case "hud": return <Hud />;
    case "pin": return <Pin />;
    case "onboarding": return <Onboarding />;
    default: return <Settings />;
  }
}

function BrowserNotice() {
  return (
    <div className="pane sheet" style={{ background: "var(--panel)", minHeight: "100%" }}>
      <h1>Open the Shotsmith window</h1>
      <p>This address only serves the interface. Capture, the tray, and shortcuts run in the desktop app, so there is no command bridge in a browser tab.</p>
      <p>If Shotsmith is already running, open History or Settings from its tray icon. Otherwise start it with <span className="mono">pnpm tauri dev</span>.</p>
    </div>
  );
}

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <Shell />
  </StrictMode>,
);
