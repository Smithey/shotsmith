import { useEffect, useState } from "react";
import { api, assetUrl, currentWindow, onRefresh, type DonutState } from "../api";

export function Donut() {
  const [state, setState] = useState<DonutState | null>(null);
  const [hover, setHover] = useState(0);
  const [error, setError] = useState("");

  useEffect(() => {
    const load = () => api.donut().then(setState).catch((err: Error) => setError(err.message));
    load();
    let stop = () => {};
    onRefresh(load).then((un) => { stop = un; });
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") currentWindow().hide();
      if (event.key === "Enter") act(state?.slots[0]?.id ?? "copy", event.shiftKey);
      const index = Number(event.key) - 1;
      if (index >= 0 && state && index < state.slots.length) act(state.slots[index].id, event.shiftKey);
    };
    window.addEventListener("keydown", onKey);
    return () => { stop(); window.removeEventListener("keydown", onKey); };
  }, [state?.captureId]);

  function act(action: string, keepOpen: boolean) {
    api.donutAct(action, keepOpen).catch((err: Error) => setError(err.message));
  }

  if (!state) return <p className="error">{error || "Opening the donut…"}</p>;
  const count = state.slots.length;
  const cx = 190;
  const cy = 190;
  return (
    <div
      className="donut"
      onMouseMove={(event) => {
        const bounds = (event.currentTarget as HTMLElement).getBoundingClientRect();
        const dx = event.clientX - bounds.left - cx;
        const dy = event.clientY - bounds.top - cy;
        if (Math.hypot(dx, dy) < 50) return;
        let angle = Math.atan2(dy, dx) + Math.PI / 2;
        if (angle < 0) angle += Math.PI * 2;
        setHover(Math.floor((angle / (Math.PI * 2)) * count) % count);
      }}
      onMouseUp={(event) => {
        const start = (event.currentTarget as HTMLElement).dataset.down;
        if (!start) return;
        const [x, y, t] = start.split(",").map(Number);
        const dx = event.clientX - x;
        const dy = event.clientY - y;
        if (Math.hypot(dx, dy) > 40 && performance.now() - t < 300) act(state.slots[hover].id, event.shiftKey);
      }}
      onMouseDown={(event) => { (event.currentTarget as HTMLElement).dataset.down = `${event.clientX},${event.clientY},${performance.now()}`; }}
    >
      <svg viewBox="0 0 380 380" onClick={(event) => act(state.slots[hover].id, event.shiftKey)}>
        {state.slots.map((slot, index) => {
          const gap = 1.6 * Math.PI / 180;
          const slice = (Math.PI * 2) / count;
          const a0 = -Math.PI / 2 + index * slice + gap / 2;
          const a1 = -Math.PI / 2 + (index + 1) * slice - gap / 2;
          const path = ring(cx, cy, 70, 176, a0, a1);
          const labelAt = polar(cx, cy, 122, (a0 + a1) / 2);
          return (
            <g key={slot.id}>
              <path d={path} fill={index === hover ? "#0A84FF" : "rgba(28,28,30,.92)"} stroke="rgba(255,255,255,.12)" />
              <text x={labelAt.x} y={labelAt.y} fill="white" fontSize="13" textAnchor="middle">{slot.label}</text>
              <text x={labelAt.x} y={labelAt.y + 14} fill="rgba(255,255,255,.7)" fontSize="10" textAnchor="middle">{slot.key}{index === 0 ? " · ↩" : ""}</text>
            </g>
          );
        })}
      </svg>
      <div className="donut-center">
        <img src={assetUrl(state.imagePath)} alt="" />
        <span>{state.width}×{state.height}</span>
      </div>
      {error && <p className="error">{error}</p>}
    </div>
  );
}

function polar(cx: number, cy: number, r: number, a: number) {
  return { x: cx + Math.cos(a) * r, y: cy + Math.sin(a) * r };
}

function ring(cx: number, cy: number, r0: number, r1: number, a0: number, a1: number) {
  const large = a1 - a0 > Math.PI ? 1 : 0;
  const p = (r: number, a: number) => `${cx + Math.cos(a) * r} ${cy + Math.sin(a) * r}`;
  return `M ${p(r1, a0)} A ${r1} ${r1} 0 ${large} 1 ${p(r1, a1)} L ${p(r0, a1)} A ${r0} ${r0} 0 ${large} 0 ${p(r0, a0)} Z`;
}
