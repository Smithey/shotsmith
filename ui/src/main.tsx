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

function Shell() {
  const [ready, setReady] = useState(false);
  useEffect(() => {
    api.platform().then((platform) => {
      document.documentElement.dataset.platform = platform;
      setReady(true);
    }).catch(() => setReady(true));
  }, []);
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

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <Shell />
  </StrictMode>,
);
