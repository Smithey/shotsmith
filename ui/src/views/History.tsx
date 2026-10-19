import { useEffect, useState } from "react";
import { api, assetUrl, onRefresh, type HistoryItem } from "../api";

export function History() {
  const [items, setItems] = useState<HistoryItem[]>([]);
  const [query, setQuery] = useState("");
  const [error, setError] = useState("");

  function load(next = query) {
    api.history(next).then(setItems).catch((err: Error) => setError(err.message));
  }

  useEffect(() => {
    load("");
    let stop = () => {};
    onRefresh(() => load()).then((un) => { stop = un; });
    return () => stop();
  }, []);

  return (
    <div className="history" style={{ flexDirection: "column" }}>
      <input className="field search" placeholder="Search by app or window" value={query} onChange={(event) => { setQuery(event.target.value); load(event.target.value); }} />
      {error && <p className="error">{error}</p>}
      <div className="grid">
        {items.map((item) => (
          <article key={item.id} className="thumb">
            <button onClick={() => api.openEditor(item.id)}>
              <img src={assetUrl(item.originalPath)} alt="" />
            </button>
            <p>{item.sourceApp ?? item.mode} · {item.pixelWidth}×{item.pixelHeight}</p>
            <p className="mono">{item.date.slice(0, 16).replace("T", " ")}</p>
            <div className="row">
              <button onClick={() => api.pin(item.id)}>Pin</button>
              <button onClick={() => api.reveal(item.id)}>Reveal</button>
              <button onClick={() => api.deleteHistory(item.id).then(() => load())}>Delete</button>
            </div>
          </article>
        ))}
        {!items.length && <p>No captures yet. Use the tray menu or the region shortcut.</p>}
      </div>
    </div>
  );
}
