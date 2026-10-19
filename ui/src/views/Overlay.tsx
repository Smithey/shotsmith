import { useEffect, useRef, useState } from "react";
import { api, assetUrl, onRefresh, type DisplayInfo, type Settings, type WindowInfo } from "../api";

type Rect = { x: number; y: number; w: number; h: number };

export function Overlay() {
  const [displays, setDisplays] = useState<DisplayInfo[]>([]);
  const [settings, setSettings] = useState<Settings | null>(null);
  const [windows, setWindows] = useState<WindowInfo[]>([]);
  const [mode, setMode] = useState<"region" | "window" | "screen">("region");
  const [rect, setRect] = useState<Rect | null>(null);
  const [draft, setDraft] = useState<Rect | null>(null);
  const [cursor, setCursor] = useState({ x: 0, y: 0 });
  const [loupeOn, setLoupeOn] = useState(true);
  const [includeCursor, setIncludeCursor] = useState(false);
  const [error, setError] = useState("");
  const drag = useRef<{ x: number; y: number; rect: Rect; handle: string } | null>(null);
  const imgRef = useRef<HTMLImageElement>(null);
  const loupeRef = useRef<HTMLCanvasElement>(null);
  const pixels = useRef<ImageData | null>(null);
  const [hex, setHex] = useState("");
  const display = displays[0];

  async function load() {
    const [next, prefs] = await Promise.all([api.overlays(), api.settings()]);
    const mine = new URLSearchParams(window.location.search).get("display");
    const list = mine ? next.filter((item) => String(item.id) === mine) : next;
    setDisplays(list.length ? [list[0]] : next.slice(0, 1));
    setSettings(prefs);
    setLoupeOn(prefs.loupe);
    setIncludeCursor(prefs.includeCursor);
    setWindows(await api.windows().catch(() => []));
  }

  useEffect(() => {
    if (!display?.imagePath) return;
    let cancel = false;
    pixels.current = null;
    fetch(assetUrl(display.imagePath))
      .then((response) => response.blob())
      .then((blob) => createImageBitmap(blob))
      .then((bitmap) => {
        if (cancel) return;
        const canvas = document.createElement("canvas");
        canvas.width = bitmap.width;
        canvas.height = bitmap.height;
        const ctx = canvas.getContext("2d", { willReadFrequently: true });
        if (!ctx) return;
        ctx.drawImage(bitmap, 0, 0);
        pixels.current = ctx.getImageData(0, 0, canvas.width, canvas.height);
      })
      .catch(() => {
        pixels.current = null;
      });
    return () => {
      cancel = true;
    };
  }, [display?.imagePath]);

  useEffect(() => {
    load().catch((err: Error) => setError(err.message));
    let stop = () => {};
    onRefresh(() => { load().catch((err: Error) => setError(err.message)); }).then((un) => { stop = un; });
    return () => stop();
  }, []);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") api.cancel();
      if (event.key === "Enter" && rect) confirm(rect);
      if (event.key === " ") { event.preventDefault(); setMode((m) => (m === "window" ? "region" : "window")); }
      if (event.key === "Tab") { event.preventDefault(); setLoupeOn((v) => !v); }
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "c" && loupeOn) {
        if (hex) api.copyHex(hex);
      }
      if (rect && ["ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight"].includes(event.key)) {
        event.preventDefault();
        const step = (event.shiftKey ? 10 : 1) * (display?.scale ?? 1);
        const next = { ...rect };
        if (event.key === "ArrowLeft") next.x -= step;
        if (event.key === "ArrowRight") next.x += step;
        if (event.key === "ArrowUp") next.y -= step;
        if (event.key === "ArrowDown") next.y += step;
        setRect(next);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [rect, loupeOn, display, includeCursor, mode, hex]);

  function point(event: React.PointerEvent): { x: number; y: number } {
    const bounds = (event.currentTarget as HTMLElement).getBoundingClientRect();
    const scaleX = (display?.width ?? bounds.width) / bounds.width;
    const scaleY = (display?.height ?? bounds.height) / bounds.height;
    return { x: (event.clientX - bounds.left) * scaleX, y: (event.clientY - bounds.top) * scaleY };
  }

  function onDown(event: React.PointerEvent) {
    if (!display || mode === "screen") return;
    const p = point(event);
    if (mode === "window") {
      const hit = windows.find((win) => p.x + display.x >= win.x && p.y + display.y >= win.y && p.x + display.x <= win.x + win.width && p.y + display.y <= win.y + win.height);
      if (hit) {
        confirm({
          x: hit.x - display.x,
          y: hit.y - display.y,
          w: hit.width,
          h: hit.height,
        }, hit.id, hit.appName, hit.title);
      }
      return;
    }
    (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
    drag.current = { x: p.x, y: p.y, rect: rect ?? { x: p.x, y: p.y, w: 0, h: 0 }, handle: "new" };
  }

  function onMove(event: React.PointerEvent) {
    const p = point(event);
    setCursor(p);
    setHex(hexAt(p));
    drawLoupe(p);
    const active = drag.current;
    if (!active || mode !== "region") return;
    let w = p.x - active.x;
    let h = p.y - active.y;
    if (event.shiftKey) {
      const side = Math.max(Math.abs(w), Math.abs(h));
      w = Math.sign(w || 1) * side;
      h = Math.sign(h || 1) * side;
    }
    let x = active.x;
    let y = active.y;
    if (event.altKey) {
      x = active.x - w;
      y = active.y - h;
      w *= 2;
      h *= 2;
    }
    const next = normalize({ x, y, w, h });
    setDraft(next);
  }

  function onUp() {
    if (draft && draft.w > 2 && draft.h > 2) setRect(draft);
    setDraft(null);
    drag.current = null;
  }

  function confirm(target: Rect, windowId?: number, sourceApp?: string, windowTitle?: string) {
    if (!display) return;
    if (settings?.sound) beep();
    api.confirm({
      displayId: display.id,
      x: Math.round(target.x),
      y: Math.round(target.y),
      width: Math.round(target.w),
      height: Math.round(target.h),
      windowId: windowId ?? null,
      sourceApp: sourceApp ?? null,
      windowTitle: windowTitle ?? null,
      includeCursor,
      cursorX: cursor.x,
      cursorY: cursor.y,
      mode: windowId ? "window" : "region",
    }).catch((err: Error) => setError(err.message));
  }

  function drawLoupe(p: { x: number; y: number }) {
    const canvas = loupeRef.current;
    const image = imgRef.current;
    if (!canvas || !image) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    canvas.width = 132;
    canvas.height = 132;
    ctx.imageSmoothingEnabled = false;
    ctx.drawImage(image, p.x - 5, p.y - 5, 11, 11, 0, 0, 132, 132);
    ctx.strokeStyle = "rgba(255,255,255,.8)";
    ctx.strokeRect(60, 0, 12, 132);
    ctx.strokeRect(0, 60, 132, 12);
  }

  function hexAt(p: { x: number; y: number }): string {
    const data = pixels.current;
    if (!data) return "";
    const x = Math.max(0, Math.min(data.width - 1, Math.round(p.x)));
    const y = Math.max(0, Math.min(data.height - 1, Math.round(p.y)));
    const index = (y * data.width + x) * 4;
    const parts = [data.data[index], data.data[index + 1], data.data[index + 2]];
    return `#${parts.map((value) => value.toString(16).padStart(2, "0")).join("")}`;
  }

  const shown = draft ?? rect;
  const scale = display?.scale || 1;
  const hot = mode === "window" && display
    ? windows.find((win) => cursor.x + display.x >= win.x && cursor.y + display.y >= win.y && cursor.x + display.x <= win.x + win.width && cursor.y + display.y <= win.y + win.height)
    : undefined;

  return (
    <div className="overlay" onPointerDown={onDown} onPointerMove={onMove} onPointerUp={onUp} onDoubleClick={() => rect && confirm(rect)}>
      {display && <img ref={imgRef} className="frozen" src={assetUrl(display.imagePath)} alt="" />}
      {hot && display && (
        <div className="window-hot" style={{ left: hot.x - display.x, top: hot.y - display.y, width: hot.width, height: hot.height }} />
      )}
      {shown && (
        <div className="shade" style={{ left: shown.x / scale, top: shown.y / scale, width: shown.w / scale, height: shown.h / scale }}>
          <div className="size-label" style={{ top: -28, left: 0 }}>
            {Math.round(shown.w / scale)} × {Math.round(shown.h / scale)} pt
            <br />
            {Math.round(shown.w)} × {Math.round(shown.h)} px @{scale}x
          </div>
        </div>
      )}
      {loupeOn && (
        <div className="loupe" style={{ left: 16, top: 16 }}>
          <canvas ref={loupeRef} />
          <p>x {Math.round(cursor.x)} y {Math.round(cursor.y)}<br />{hex || "-"} · ⌘C copies</p>
        </div>
      )}
      {error && <p className="error" style={{ position: "absolute", top: 12, left: 12 }}>{error}</p>}
      {settings?.overlayToolbar !== false && (
        <>
          <div className="hint">Space window · Shift lock aspect · Alt from centre · Arrows nudge · Tab loupe · Esc cancel</div>
          <div className="toolbar">
            {(["region", "window", "screen"] as const).map((item) => (
              <button key={item} className={mode === item ? "active" : ""} onClick={(event) => { event.stopPropagation(); setMode(item); if (item === "screen" && display) confirm({ x: 0, y: 0, w: display.width, h: display.height }); }}>{item[0].toUpperCase() + item.slice(1)}</button>
            ))}
            <button onClick={(event) => { event.stopPropagation(); api.countdown(); }}>Timer {settings?.timedSeconds ?? 5}s</button>
            <button className={includeCursor ? "active" : ""} onClick={(event) => { event.stopPropagation(); setIncludeCursor((v) => !v); }}>Cursor</button>
            <button className="active" onClick={(event) => { event.stopPropagation(); if (rect) confirm(rect); }}>Capture</button>
          </div>
        </>
      )}
    </div>
  );
}

function normalize(rect: Rect): Rect {
  return {
    x: rect.w < 0 ? rect.x + rect.w : rect.x,
    y: rect.h < 0 ? rect.y + rect.h : rect.y,
    w: Math.abs(rect.w),
    h: Math.abs(rect.h),
  };
}

function beep() {
  const ctx = new AudioContext();
  const osc = ctx.createOscillator();
  const gain = ctx.createGain();
  osc.frequency.value = 880;
  gain.gain.value = 0.04;
  osc.connect(gain);
  gain.connect(ctx.destination);
  osc.start();
  osc.stop(ctx.currentTime + 0.04);
}
