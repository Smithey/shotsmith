import { useEffect, useState } from "react";
import { api, type AppChoice } from "../api";

export function OpenIn() {
  const [apps, setApps] = useState<AppChoice[]>([]);
  const [recent, setRecent] = useState<AppChoice[]>([]);
  const [error, setError] = useState("");

  useEffect(() => {
    api.openInState().then((state) => {
      setApps(state.apps);
      setRecent(state.recent);
    }).catch((err: Error) => setError(err.message));
  }, []);

  function choose(id: string) {
    api.openInChoose(id, null).catch((err: Error) => setError(err.message));
  }

  return (
    <div className="pane sheet">
      <h1>Open in</h1>
      {recent.length > 0 && <h2>Recent</h2>}
      <div className="action-list">
        {recent.map((app) => <button key={app.id} type="button" onClick={() => choose(app.id)}>{app.name}</button>)}
      </div>
      <h2>Installed</h2>
      <div className="action-list">
        {apps.map((app) => <button key={app.id} type="button" onClick={() => choose(app.id)}>{app.name}</button>)}
      </div>
      {error && <p className="error">{error}</p>}
    </div>
  );
}
