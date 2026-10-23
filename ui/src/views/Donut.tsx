import { useEffect, useRef, useState } from "react";
import { api, assetUrl, currentWindow, onRefresh, type DonutState } from "../api";

const SIZE = 300;
const CENTER = 150;
const OUTER = 150;
const INNER = 58;

export function Donut() {
  const [state, setState] = useState<DonutState | null>(null);
  const [hover, setHover] = useState(0);
  const [error, setError] = useState("");
  const stateRef = useRef<DonutState | null>(null);
  const busy = useRef(false);
  stateRef.current = state;

  useEffect(() => {
    const load = () => api.donut().then(setState).catch((err: Error) => setError(err.message));
    load();
    let stop = () => {};
    onRefresh(load).then((un) => { stop = un; });
    const onKey = (event: KeyboardEvent) => {
      const current = stateRef.current;
      if (event.key === "Escape") currentWindow().hide();
      if (!current) return;
      if (event.key === "Enter") act(current.slots[0]?.id ?? "copy", event.shiftKey);
      const index = Number(event.key) - 1;
      if (index >= 0 && index < current.slots.length) act(current.slots[index].id, event.shiftKey);
    };
    window.addEventListener("keydown", onKey);
    return () => { stop(); window.removeEventListener("keydown", onKey); };
  }, []);

  function act(action: string, keepOpen: boolean) {
    if (!action || busy.current) return;
    busy.current = true;
    api.donutAct(action, keepOpen).catch((err: Error) => {
      setError(err.message);
      busy.current = false;
    });
    if (keepOpen) busy.current = false;
  }

  if (!state) return <p className="error donut-status">{error || "Opening the donut…"}</p>;
  const count = state.slots.length;
  return (
    <div
      className="donut"
      onMouseMove={(event) => {
        const bounds = (event.currentTarget as HTMLElement).getBoundingClientRect();
        const dx = event.clientX - bounds.left - CENTER;
        const dy = event.clientY - bounds.top - CENTER;
        if (Math.hypot(dx, dy) < 50) return;
        let angle = Math.atan2(dy, dx) + Math.PI / 2;
        if (angle < 0) angle += Math.PI * 2;
        setHover(Math.floor((angle / (Math.PI * 2)) * count) % count);
      }}
      onMouseDown={(event) => { (event.currentTarget as HTMLElement).dataset.down = `${event.clientX},${event.clientY},${performance.now()}`; }}
      onMouseUp={(event) => {
        const start = (event.currentTarget as HTMLElement).dataset.down;
        if (!start) return;
        const [x, y, t] = start.split(",").map(Number);
        const dx = event.clientX - x;
        const dy = event.clientY - y;
        if (Math.hypot(dx, dy) > 40 && performance.now() - t < 300) act(state.slots[hover].id, event.shiftKey);
      }}
    >
      <svg viewBox={`0 0 ${SIZE} ${SIZE}`} width={SIZE} height={SIZE}>
        {state.slots.map((slot, index) => {
          const gap = 1.6 * Math.PI / 180;
          const slice = (Math.PI * 2) / count;
          const a0 = -Math.PI / 2 + index * slice + gap / 2;
          const a1 = -Math.PI / 2 + (index + 1) * slice - gap / 2;
          const path = ring(CENTER, CENTER, INNER, OUTER - 2, a0, a1);
          const labelAt = polar(CENTER, CENTER, 102, (a0 + a1) / 2);
          return (
            <g key={slot.id} onClick={(event) => { event.stopPropagation(); act(slot.id, event.shiftKey); }}>
              <path d={path} fill={index === hover ? "var(--accent)" : "rgba(28,28,30,0.92)"} stroke="rgba(255,255,255,0.10)" />
              <text x={labelAt.x} y={labelAt.y} fill="white" fontSize="13" textAnchor="middle">{slot.label}</text>
              <text x={labelAt.x} y={labelAt.y + 14} fill="rgba(255,255,255,0.72)" fontSize="11" textAnchor="middle">{slot.key}{index === 0 ? "  ↩" : ""}</text>
            </g>
          );
        })}
      </svg>
      <div className="donut-center">
        {state.imagePath && <img src={assetUrl(state.imagePath)} alt="" />}
        <span>{state.width}×{state.height}</span>
      </div>
      {error && <p className="error donut-status">{error}</p>}
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
