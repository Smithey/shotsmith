import { useEffect, useState } from "react";
import { api, assetUrl, currentWindow, onRefresh, type HudState } from "../api";

export function Hud() {
  const [state, setState] = useState<HudState | null>(null);
  const [left, setLeft] = useState<number | null>(null);

  useEffect(() => {
    const load = () => api.hud().then((next) => {
      setState(next);
      setLeft(next.countdown);
    }).catch(() => undefined);
    load();
    let stop = () => {};
    onRefresh(load).then((un) => { stop = un; });
    return () => stop();
  }, []);

  useEffect(() => {
    if (left == null) {
      const timer = window.setTimeout(() => currentWindow().hide(), 2500);
      return () => window.clearTimeout(timer);
    }
    if (left <= 0) {
      api.countdownDone();
      return;
    }
    const timer = window.setTimeout(() => setLeft(left - 1), 1000);
    return () => window.clearTimeout(timer);
  }, [left]);

  if (!state) return null;
  return (
    <div className="hud">
      {state.imagePath && <img src={assetUrl(state.imagePath)} alt="" />}
      <div>
        <strong>{left != null ? `Capturing in ${left}` : state.title}</strong>
        <div>{state.detail}</div>
      </div>
      {state.captureId && <button onClick={() => api.openEditor(state.captureId)}>Edit</button>}
    </div>
  );
}
