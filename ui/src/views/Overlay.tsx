import { useEffect, useRef, useState } from "react";
import { api, assetUrl, onRefresh, type DisplayInfo, type Settings, type WindowInfo } from "../api";
import { isMac } from "../keys";

type Rect = { x: number; y: number; w: number; h: number };
type CaptureMode = "region" | "window" | "screen" | "text" | "scroll" | "measure";

export function Overlay() {
  const [displays, setDisplays] = useState<DisplayInfo[]>([]);
  const [settings, setSettings] = useState<Settings | null>(null);
  const [windows, setWindows] = useState<WindowInfo[]>([]);
  const [mode, setMode] = useState<CaptureMode>("region");
  const [mark, setMark] = useState<{ x: number; y: number } | null>(null);
  const [markB, setMarkB] = useState<{ x: number; y: number } | null>(null);
  const [element, setElement] = useState<Rect | null>(null);
  const [rect, setRect] = useState<Rect | null>(null);
  const [draft, setDraft] = useState<Rect | null>(null);
  const [cursor, setCursor] = useState({ x: 0, y: 0 });
  const [tracking, setTracking] = useState(false);
  const [loupeOn, setLoupeOn] = useState(true);
  const [includeCursor, setIncludeCursor] = useState(false);
  const [error, setError] = useState("");
  const drag = useRef<{ x: number; y: number; rect: Rect; handle: string } | null>(null);
  const draftRef = useRef<Rect | null>(null);
  const confirming = useRef(false);
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
    const captureMode = await api.captureMode().catch(() => "region");
    if (captureMode === "window" || captureMode === "screen" || captureMode === "region" || captureMode === "text" || captureMode === "scroll" || captureMode === "measure") setMode(captureMode);
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

  function resetCapture() {
    // The overlay window is reused, so each capture has to start clean.
    drag.current = null;
    draftRef.current = null;
    confirming.current = false;
    setRect(null);
    setDraft(null);
    setTracking(false);
    setError("");
  }

  useEffect(() => {
    resetCapture();
    load().catch((err: Error) => setError(err.message));
    let stop = () => {};
    onRefresh((payload) => {
      if (payload === "history" || payload === "status-menu") return;
      resetCapture();
      load().catch((err: Error) => setError(err.message));
    }).then((un) => { stop = un; });
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
    if (!display) return;
    if ((event.target as HTMLElement).closest("button")) return;
    const p = point(event);
    if (mode === "measure") {
      const scale = display.scale || 1;
      api.measureAt(display.x + p.x / scale, display.y + p.y / scale).then((hit) => {
        setElement({ x: (hit.x - display.x) * scale, y: (hit.y - display.y) * scale, w: hit.w * scale, h: hit.h * scale });
      }).catch(() => setElement(null));
      if (!mark || markB) {
        setMark(p);
        setMarkB(null);
      } else {
        setMarkB(p);
      }
      return;
    }
    if (mode === "screen") {
      confirm({ x: 0, y: 0, w: display.width, h: display.height });
      return;
    }
    if (mode === "window") {
      const scale = display.scale || 1;
      const gx = display.x + p.x / scale;
      const gy = display.y + p.y / scale;
      const hit = windows.find((win) => gx >= win.x && gy >= win.y && gx <= win.x + win.width && gy <= win.y + win.height);
      if (hit) {
        confirm({
          x: (hit.x - display.x) * scale,
          y: (hit.y - display.y) * scale,
          w: hit.width * scale,
          h: hit.height * scale,
        }, hit.id, hit.appName, hit.title);
      }
      return;
    }
    (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
    confirming.current = false;
    draftRef.current = null;
    setRect(null);
    setDraft(null);
    drag.current = { x: p.x, y: p.y, rect: { x: p.x, y: p.y, w: 0, h: 0 }, handle: "new" };
  }

  function onMove(event: React.PointerEvent) {
    const p = point(event);
    setTracking(true);
    setCursor(p);
    setHex(hexAt(p));
    drawLoupe(p);
    const active = drag.current;
    const picking = mode === "region" || mode === "text" || mode === "scroll";
    if (!active || !picking) return;
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
    draftRef.current = next;
    setDraft(next);
  }

  function onUp() {
    const next = draftRef.current ?? draft;
    draftRef.current = null;
    const drew = drag.current?.handle === "new";
    setDraft(null);
    drag.current = null;
    const picking = mode === "region" || mode === "text" || mode === "scroll";
    if (picking && drew && next && next.w > 8 && next.h > 8) {
      setRect(next);
      confirm(next);
    }
  }

  function confirm(target: Rect, windowId?: number, sourceApp?: string, windowTitle?: string) {
    if (!display || confirming.current) return;
    confirming.current = true;
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
      mode: windowId ? "window" : mode,
    }).then(() => {
      confirming.current = false;
      drag.current = null;
      draftRef.current = null;
      setRect(null);
      setDraft(null);
    }).catch((err: Error) => {
      confirming.current = false;
      setError(err.message);
    });
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
    ? windows.find((win) => {
        const gx = display.x + cursor.x / scale;
        const gy = display.y + cursor.y / scale;
        return gx >= win.x && gy >= win.y && gx <= win.x + win.width && gy <= win.y + win.height;
      })
    : undefined;
  const loupeLeft = display ? Math.min(Math.max(12, cursor.x / scale + 28), display.width / scale - 168) : 16;
  const loupeTop = display ? Math.min(Math.max(12, cursor.y / scale + 28), display.height / scale - 190) : 16;

  return (
    <div className="overlay" onPointerDown={onDown} onPointerMove={onMove} onPointerUp={onUp} onDoubleClick={() => rect && confirm(rect)}>
      {display && <img key={display.imagePath} ref={imgRef} className="frozen" src={assetUrl(display.imagePath)} alt="" />}
      {hot && display && (
        <div className="window-hot" style={{ left: hot.x - display.x, top: hot.y - display.y, width: hot.width, height: hot.height }} />
      )}
      {shown && (
        <div className="shade" style={{ left: shown.x / scale, top: shown.y / scale, width: shown.w / scale, height: shown.h / scale }}>
          <div className="size-label" style={{ top: shown.y / scale < 40 ? 8 : -36, left: 0 }}>
            {Math.round(shown.w / scale)} × {Math.round(shown.h / scale)} pt
            <br />
            {Math.round(shown.w)} × {Math.round(shown.h)} px @{scale}x
          </div>
        </div>
      )}
      {loupeOn && tracking && mode !== "window" && (
        <div className="loupe" style={{ left: loupeLeft, top: loupeTop }}>
          <canvas ref={loupeRef} />
          <p>x {Math.round(cursor.x)} y {Math.round(cursor.y)}<br />{hex || "-"} · {isMac() ? "⌘C" : "Ctrl+C"} copies</p>
        </div>
      )}
      {element && <div className="shade" style={{ left: element.x / scale, top: element.y / scale, width: element.w / scale, height: element.h / scale }} />}
      {mark && markB && (
        <svg className="measure-line" aria-hidden="true">
          <line x1={mark.x / scale} y1={mark.y / scale} x2={markB.x / scale} y2={markB.y / scale} />
          <text x={(mark.x + markB.x) / 2 / scale} y={(mark.y + markB.y) / 2 / scale}>{Math.round(Math.hypot(markB.x - mark.x, markB.y - mark.y))} px</text>
        </svg>
      )}
      {error && <p className="error" style={{ position: "absolute", top: 12, left: 12 }}>{error}</p>}
      {settings?.overlayToolbar !== false && (
        <>
          <div className="hint">{hintFor(mode)}</div>
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

function hintFor(mode: CaptureMode): string {
  if (mode === "window") return "Click a window to capture it · Space for a region · Esc cancel";
  if (mode === "screen") return "Click to capture this display · Esc cancel";
  if (mode === "text") return "Drag a region. Its text is copied when you release.";
  if (mode === "scroll") return "Drag the region to scroll. Release to start.";
  if (mode === "measure") return "Click an element for its size. Click two points for the distance.";
  return "Release to capture · Shift locks aspect · Space for windows · Esc cancel";
}

function normalize(rect: Rect): Rect {
  return {
    x: rect.w < 0 ? rect.x + rect.w : rect.x,
    y: rect.h < 0 ? rect.y + rect.h : rect.y,
    w: Math.abs(rect.w),
    h: Math.abs(rect.h),
  };
}

let shutter: AudioContext | null = null;

function beep() {
  if (!shutter) shutter = new AudioContext();
  const ctx = shutter;
  if (ctx.state === "suspended") void ctx.resume();
  const osc = ctx.createOscillator();
  const gain = ctx.createGain();
  osc.frequency.value = 880;
  gain.gain.value = 0.04;
  osc.connect(gain);
  gain.connect(ctx.destination);
  osc.start();
  osc.stop(ctx.currentTime + 0.04);
}
