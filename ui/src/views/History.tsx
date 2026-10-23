import { useEffect, useMemo, useState } from "react";
import { api, onHistoryChanged, onRefresh, shotUrl, type HistoryItem } from "../api";

export function History() {
  const [items, setItems] = useState<HistoryItem[]>([]);
  const [query, setQuery] = useState("");
  const [when, setWhen] = useState<"all" | "today">("all");
  const [appName, setAppName] = useState("");
  const [error, setError] = useState("");

  function load(next = query) {
    api.history(next).then(setItems).catch((err: Error) => setError(err.message));
  }

  useEffect(() => {
    load("");
    let stop = () => {};
    onRefresh(() => load()).then((un) => { stop = un; });
    let stopHistory = () => {};
    onHistoryChanged(() => load()).then((un) => { stopHistory = un; });
    return () => {
      stop();
      stopHistory();
    };
  }, []);

  const apps = useMemo(() => {
    const names = new Set<string>();
    for (const item of items) {
      if (item.sourceApp) names.add(item.sourceApp);
    }
    return Array.from(names).sort();
  }, [items]);

  const shown = items.filter((item) => {
    if (appName && item.sourceApp !== appName) return false;
    if (when !== "today") return true;
    const date = new Date(item.date);
    const now = new Date();
    return date.getFullYear() === now.getFullYear() && date.getMonth() === now.getMonth() && date.getDate() === now.getDate();
  });

  return (
    <div className="history">
      <header className="hist-bar">
        <strong>History</strong>
        <input
          className="hist-search"
          placeholder="Search"
          value={query}
          onChange={(event) => {
            setQuery(event.target.value);
            load(event.target.value);
          }}
        />
        <div className="segment">
          <button type="button" className={when === "all" ? "on" : ""} onClick={() => setWhen("all")}>All</button>
          <button type="button" className={when === "today" ? "on" : ""} onClick={() => setWhen("today")}>Today</button>
        </div>
        <select className="set-input hist-app" value={appName} onChange={(event) => setAppName(event.target.value)}>
          <option value="">All apps</option>
          {apps.map((name) => <option key={name} value={name}>{name}</option>)}
        </select>
      </header>
      {error && <p className="error hist-error">{error}</p>}
      <div className="hist-scroll">
        {shown.length > 0 ? (
          <div className="hist-grid">
            {shown.map((item) => (
              <article key={item.id} className="hist-card">
                <div className="hist-shot">
                  <button type="button" className="hist-open" onClick={() => api.openEditor(item.id)}>
                    <img key={`${item.id}-${item.previewRev ?? 0}`} src={shotUrl(item)} alt="" />
                  </button>
                  <span className="hist-actions">
                    <button type="button" onClick={() => api.openEditor(item.id)}>Open</button>
                    <button type="button" onClick={() => api.copyHistory(item.id).catch((err: Error) => setError(err.message))}>Copy</button>
                    <button type="button" onClick={() => api.reveal(item.id)}>Reveal</button>
                    <button type="button" onClick={() => api.pin(item.id)}>{item.pinned ? "Pinned" : "Pin"}</button>
                    <button type="button" onClick={() => api.deleteHistory(item.id).then(() => load()).catch((err: Error) => setError(err.message))}>Delete</button>
                  </span>
                </div>
                <span className="hist-cap">{caption(item)}</span>
              </article>
            ))}
          </div>
        ) : (
          <p className="hist-empty">{query ? "No matches." : "No captures yet."}</p>
        )}
      </div>
    </div>
  );
}

function caption(item: HistoryItem): string {
  const date = new Date(item.date);
  const size = item.mode === "window" ? "Window" : `${item.pixelWidth}×${item.pixelHeight}`;
  if (Number.isNaN(date.getTime())) return size;
  const today = new Date();
  const start = new Date(today.getFullYear(), today.getMonth(), today.getDate()).getTime();
  const day = new Date(date.getFullYear(), date.getMonth(), date.getDate()).getTime();
  const diff = Math.round((start - day) / 86400000);
  if (diff === 1) return "Yesterday";
  if (diff > 1) return date.toLocaleDateString(undefined, { month: "short", day: "numeric" });
  const time = date.toLocaleTimeString(undefined, { hour: "numeric", minute: "2-digit" });
  return `${time} · ${size}`;
}
