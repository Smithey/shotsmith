import { useEffect, useState } from "react";
import { api, type DonutState } from "../api";

export function Actions() {
  const [state, setState] = useState<DonutState | null>(null);
  const [error, setError] = useState("");

  useEffect(() => {
    api.donut().then(setState).catch((err: Error) => setError(err.message));
  }, []);

  return (
    <div className="pane sheet">
      <h1>Capture actions</h1>
      <div className="action-list" role="menu">
        {state?.slots.map((slot) => (
          <button key={slot.id} type="button" role="menuitem" onClick={() => api.donutAct(slot.id, false).catch((err: Error) => setError(err.message))}>
            {slot.label}
          </button>
        ))}
      </div>
      {error && <p className="error">{error}</p>}
    </div>
  );
}
