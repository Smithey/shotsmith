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

  const recentIds = new Set(recent.map((app) => app.id));
  const rest = apps.filter((app) => !recentIds.has(app.id));

  return (
    <div className="open-in">
      {recent.length > 0 && <div className="menu-label">Recent</div>}
      {recent.map((app) => (
        <button key={app.id} type="button" className="menu-row" onClick={() => choose(app.id)}>{app.name}</button>
      ))}
      {recent.length > 0 && rest.length > 0 && <div className="menu-rule" />}
      <div className="menu-label">Installed</div>
      {rest.map((app) => (
        <button key={app.id} type="button" className="menu-row" onClick={() => choose(app.id)}>{app.name}</button>
      ))}
      {rest.length === 0 && recent.length === 0 && <p className="recent-empty">No apps found.</p>}
      {error && <p className="error">{error}</p>}
    </div>
  );
}
