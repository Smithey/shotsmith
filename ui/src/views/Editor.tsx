import { useEffect, useRef, useState } from "react";
import { api, assetUrl, onRefresh, uid, SWATCHES, type AnnotationObject, type Document, type EditorState, type Preset } from "../api";
import { bounds, drawDocument, hitTest, idOf, whenBitmapsLoad } from "./draw";

type Tool = "select" | "crop" | "arrow" | "rect" | "ellipse" | "line" | "freehand" | "text" | "counter" | "highlight" | "obfuscate";

const KEYS: Record<string, Tool> = { v: "select", c: "crop", a: "arrow", r: "rect", e: "ellipse", l: "line", f: "freehand", t: "text", n: "counter", h: "highlight", o: "obfuscate" };

export function Editor() {
  const [state, setState] = useState<EditorState | null>(null);
  const [doc, setDoc] = useState<Document | null>(null);
  const [tool, setTool] = useState<Tool>("arrow");
  const [selected, setSelected] = useState<string[]>([]);
  const [stroke, setStroke] = useState("#FF3B30");
  const [strokeWidth, setStrokeWidth] = useState(4);
  const [fill, setFill] = useState("none");
  const [presets, setPresets] = useState<Preset[]>([]);
  const [presetId, setPresetId] = useState("builtin-docs-png");
  const [zoom, setZoom] = useState(1);
  const [pan, setPan] = useState({ x: 40, y: 40 });
  const [error, setError] = useState("");
  const [askClose, setAskClose] = useState(false);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const imageRef = useRef<HTMLImageElement | null>(null);
  const drag = useRef<{ x: number; y: number; object: AnnotationObject | null; points: { x: number; y: number }[] } | null>(null);

  async function load() {
    const [next, list] = await Promise.all([api.editor(), api.presets()]);
    setState(next);
    setDoc(next.document);
    setPresets(list);
    const image = new Image();
    image.src = assetUrl(next.imagePath);
    image.onload = () => { imageRef.current = image; paint(next.document); };
  }

  function paint(document = doc, ids = selected) {
    const canvas = canvasRef.current;
    if (!canvas || !document) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    const rect = canvas.getBoundingClientRect();
    canvas.width = rect.width * devicePixelRatio;
    canvas.height = rect.height * devicePixelRatio;
    ctx.setTransform(devicePixelRatio, 0, 0, devicePixelRatio, 0, 0);
    drawDocument(ctx, imageRef.current, document, zoom, pan.x, pan.y, ids);
  }

  useEffect(() => {
    load().catch((err: Error) => setError(err.message));
    let stop = () => {};
    onRefresh(() => load().catch((err: Error) => setError(err.message))).then((un) => { stop = un; });
    return () => stop();
  }, []);

  useEffect(() => {
    whenBitmapsLoad(() => paint());
    paint();
  }, [doc, zoom, pan, selected]);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      const target = event.target as HTMLElement;
      if (target.tagName === "INPUT" || target.tagName === "TEXTAREA" || target.tagName === "SELECT") return;
      const next = KEYS[event.key.toLowerCase()];
      if (next && !event.metaKey && !event.ctrlKey) setTool(next);
      if (event.key === "Escape") setTool("select");
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "z") {
        event.preventDefault();
        (event.shiftKey ? api.redo() : api.undo()).then(applyRemote).catch((err: Error) => setError(err.message));
      }
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "s") {
        event.preventDefault();
        save(event.shiftKey);
      }
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "c") api.copyEditor();
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "0") setZoom(fit());
      if ((event.metaKey || event.ctrlKey) && event.key === "1") setZoom(1);
      if ((event.key === "Delete" || event.key === "Backspace") && selected.length && doc) {
        const next = { ...doc, objects: doc.objects.filter((object) => !selected.includes(idOf(object))) };
        renumber(next);
        commit(next);
        setSelected([]);
      }
      if (doc && selected.length && ["ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight"].includes(event.key)) {
        const step = event.shiftKey ? 10 : 1;
        const dx = event.key === "ArrowLeft" ? -step : event.key === "ArrowRight" ? step : 0;
        const dy = event.key === "ArrowUp" ? -step : event.key === "ArrowDown" ? step : 0;
        commit(moveSelected(doc, selected, dx, dy));
      }
    };
    window.addEventListener("keydown", onKey);
    window.addEventListener("beforeunload", (event) => { if (state?.dirty) event.preventDefault(); });
    return () => window.removeEventListener("keydown", onKey);
  }, [doc, selected, state, zoom]);

  function applyRemote(next: EditorState) {
    setState(next);
    setDoc(next.document);
  }

  function commit(next: Document) {
    setDoc(next);
    api.commit(next).then(setState).catch((err: Error) => setError(err.message));
  }

  function localPoint(event: React.PointerEvent) {
    const bounds = canvasRef.current!.getBoundingClientRect();
    return { x: (event.clientX - bounds.left - pan.x) / zoom, y: (event.clientY - bounds.top - pan.y) / zoom };
  }

  function onDown(event: React.PointerEvent) {
    if (!doc) return;
    const p = localPoint(event);
    if (tool === "select") {
      const hit = hitTest(doc, p.x, p.y);
      setSelected(hit ? (event.shiftKey ? [...selected, hit] : [hit]) : []);
      drag.current = { x: p.x, y: p.y, object: null, points: [] };
      return;
    }
    if (tool === "text") {
      const object: AnnotationObject = { type: "text", id: uid(), x: p.x, y: p.y, text: "Text", size: 18, color: stroke, fill, bold: true };
      commit({ ...doc, objects: [...doc.objects, object] });
      setSelected([object.id]);
      return;
    }
    if (tool === "counter") {
      const number = doc.objects.reduce((max, object) => object.type === "counter" ? Math.max(max, object.number) : max, 0) + 1;
      const object: AnnotationObject = { type: "counter", id: uid(), x: p.x, y: p.y, number, style: "circle", fill: stroke, size: 28 };
      commit({ ...doc, objects: [...doc.objects, object] });
      return;
    }
    const object = startObject(tool, p, stroke, strokeWidth, fill);
    drag.current = { x: p.x, y: p.y, object, points: [p] };
  }

  function onMove(event: React.PointerEvent) {
    const active = drag.current;
    if (!active || !doc) return;
    const p = localPoint(event);
    if (tool === "select" && selected.length) {
      const dx = p.x - active.x;
      const dy = p.y - active.y;
      active.x = p.x;
      active.y = p.y;
      setDoc(moveSelected(doc, selected, dx, dy));
      return;
    }
    if (tool === "freehand" && active.object?.type === "freehand") {
      active.points.push(p);
      active.object = { ...active.object, points: active.points };
      setDoc({ ...doc, objects: [...doc.objects.filter((item) => item.id !== active.object!.id), active.object] });
      return;
    }
    if (!active.object) return;
    const w = p.x - active.x;
    const h = event.shiftKey ? Math.sign(p.y - active.y || 1) * Math.abs(w) : p.y - active.y;
    active.object = resizeObject(active.object, active.x, active.y, w, h, event.altKey);
    setDoc({ ...doc, objects: [...doc.objects.filter((item) => item.id !== active.object!.id), active.object] });
  }

  function onUp() {
    const active = drag.current;
    drag.current = null;
    if (!active?.object || !doc) {
      if (tool === "select" && doc) commit(doc);
      return;
    }
    const box = bounds(active.object);
    if (box && box.w < 2 && box.h < 2 && active.object.type !== "freehand") return;
    commit({ ...doc, objects: [...doc.objects.filter((item) => item.id !== active.object!.id), active.object] });
    if (tool === "crop") {
      const cropBox = bounds(active.object);
      if (cropBox) {
        api.crop(Math.round(cropBox.x), Math.round(cropBox.y), Math.round(cropBox.w), Math.round(cropBox.h))
          .then((next) => { applyRemote(next); imageRef.current = null; load(); })
          .catch((err: Error) => setError(err.message));
      }
    }
  }

  async function save(saveAs: boolean) {
    try {
      if (saveAs) {
        const path = await api.pickSave("capture.png");
        if (!path) return;
        await api.saveEditor(null, path);
      } else {
        await api.saveEditor(presetId, null);
      }
      const next = await api.editor();
      setState(next);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Save failed.");
    }
  }

  function fit() {
    const canvas = canvasRef.current;
    if (!canvas || !doc) return 1;
    return Math.min(canvas.clientWidth / doc.width, canvas.clientHeight / doc.height) * 0.9;
  }

  const selectedObject = doc?.objects.find((object) => selected.includes(idOf(object))) ?? null;
  const preset = presets.find((item) => item.id === presetId) ?? presets[0];

  return (
    <div className="editor">
      <header className="topbar">
        <h1>Capture</h1>
        <span className="mono">{state?.dirty ? "Edited" : "Saved to History"}</span>
        <div className="grow" />
        <div className="tools">
          {(Object.keys(KEYS) as Array<keyof typeof KEYS>).map((key) => (
            <button key={key} className={`tool ${tool === KEYS[key] ? "active" : ""}`} onClick={() => setTool(KEYS[key])} title={KEYS[key]}>{key.toUpperCase()}</button>
          ))}
        </div>
        <div className="grow" />
        <button className="tool" onClick={() => api.undo().then(applyRemote)}>Undo</button>
        <button className="tool" onClick={() => api.copyEditor()}>Copy</button>
        <div className="split">
          <button onClick={() => save(false)}>Save · {preset?.name ?? "Docs PNG"}</button>
          <button onClick={() => save(true)}>▾</button>
        </div>
      </header>
      <div className="stage">
        <div className="canvas-wrap">
          <canvas ref={canvasRef} onPointerDown={onDown} onPointerMove={onMove} onPointerUp={onUp} />
          <div className="zoom">
            <button onClick={() => setZoom((z) => Math.max(0.1, z / 1.15))}>−</button>
            <span className="mono">{Math.round(zoom * 100)}%</span>
            <button onClick={() => setZoom((z) => Math.min(8, z * 1.15))}>+</button>
            <button onClick={() => setZoom(fit())}>Fit</button>
            <button onClick={() => { setZoom(1); setPan({ x: 40, y: 40 }); }}>100%</button>
          </div>
        </div>
        <aside className="inspector">
          <h2>{selectedObject ? selectedObject.type : tool}</h2>
          <label>Colour</label>
          <div className="row">
            {SWATCHES.map((color) => <button key={color} className="swatch" style={{ background: color }} onClick={() => { setStroke(color); if (selectedObject && doc) commit(recolor(doc, selected, color)); }} />)}
          </div>
          <label>Stroke {strokeWidth}</label>
          <input className="field" type="range" min={1} max={24} value={strokeWidth} onChange={(event) => setStrokeWidth(Number(event.target.value))} />
          <label>Fill</label>
          <select className="field" value={fill} onChange={(event) => setFill(event.target.value)}>
            <option value="none">None</option>
            <option value="#FFFFFF">White</option>
            <option value={stroke}>Colour</option>
          </select>
          {selectedObject?.type === "text" && (
            <>
              <label>Text</label>
              <textarea className="field" value={selectedObject.text} onChange={(event) => doc && commit(updateText(doc, selectedObject.id, event.target.value))} />
            </>
          )}
          {selectedObject?.type === "obfuscate" && <p>Flattened on export. Original pixels are not written to the file.</p>}
          {tool === "obfuscate" && <p>Pixelate, blur, and solid are flattened in Rust when you copy or save.</p>}
          {error && <p className="error">{error}</p>}
          <label>Preset</label>
          <select className="field" value={presetId} onChange={(event) => setPresetId(event.target.value)}>
            {presets.map((item) => <option key={item.id} value={item.id}>{item.name}</option>)}
          </select>
        </aside>
      </div>
      <footer className="statusbar">
        <span className="mono">{doc ? `${doc.width} × ${doc.height} px` : ""}</span>
        <span>{doc?.objects.length ?? 0} objects</span>
        <span className="grow" />
        <span>{tool} · Esc selects</span>
        <button className="tool" onClick={() => state?.dirty ? setAskClose(true) : window.close()}>Close</button>
      </footer>
      {askClose && (
        <div className="dialog">
          <form onSubmit={(event) => { event.preventDefault(); save(false).then(() => window.close()); }}>
            <p>This capture has unsaved edits.</p>
            <div className="row">
              <button type="submit">Save</button>
              <button type="button" onClick={() => window.close()}>Close anyway</button>
              <button type="button" onClick={() => setAskClose(false)}>Keep editing</button>
            </div>
          </form>
        </div>
      )}
    </div>
  );
}

function startObject(tool: Tool, p: { x: number; y: number }, stroke: string, strokeWidth: number, fill: string): AnnotationObject | null {
  const id = uid();
  if (tool === "rect" || tool === "crop") return { type: "rect", id, x: p.x, y: p.y, w: 1, h: 1, stroke: tool === "crop" ? "#0A84FF" : stroke, strokeWidth: 2, fill: "none", radius: 0, opacity: 1, shadow: false };
  if (tool === "ellipse") return { type: "ellipse", id, x: p.x, y: p.y, w: 1, h: 1, stroke, strokeWidth, fill, opacity: 1, shadow: false };
  if (tool === "line") return { type: "line", id, x1: p.x, y1: p.y, x2: p.x, y2: p.y, stroke, strokeWidth, shadow: false };
  if (tool === "arrow") return { type: "arrow", id, x1: p.x, y1: p.y, x2: p.x, y2: p.y, cx: null, cy: null, heads: "end", stroke, strokeWidth, shadow: true };
  if (tool === "freehand") return { type: "freehand", id, points: [p], stroke, strokeWidth };
  if (tool === "highlight") return { type: "highlight", id, x: p.x, y: p.y, w: 1, h: 1, mode: "marker", color: stroke, opacity: 0.35 };
  if (tool === "obfuscate") return { type: "obfuscate", id, x: p.x, y: p.y, w: 1, h: 1, mode: "pixelate", blockSize: 12, color: "#000000" };
  return null;
}

function resizeObject(object: AnnotationObject, x: number, y: number, w: number, h: number, fromCenter: boolean): AnnotationObject {
  const left = fromCenter ? x - w : w < 0 ? x + w : x;
  const top = fromCenter ? y - h : h < 0 ? y + h : y;
  const width = fromCenter ? Math.abs(w) * 2 : Math.abs(w);
  const height = fromCenter ? Math.abs(h) * 2 : Math.abs(h);
  if (object.type === "rect" || object.type === "ellipse" || object.type === "highlight" || object.type === "obfuscate") {
    return { ...object, x: left, y: top, w: width, h: height };
  }
  if (object.type === "line" || object.type === "arrow") return { ...object, x2: x + w, y2: y + h };
  return object;
}

function moveSelected(doc: Document, ids: string[], dx: number, dy: number): Document {
  return {
    ...doc,
    objects: doc.objects.map((object) => ids.includes(idOf(object)) ? shift(object, dx, dy) : object),
  };
}

function shift(object: AnnotationObject, dx: number, dy: number): AnnotationObject {
  if (object.type === "line" || object.type === "arrow") return { ...object, x1: object.x1 + dx, y1: object.y1 + dy, x2: object.x2 + dx, y2: object.y2 + dy };
  if (object.type === "freehand") return { ...object, points: object.points.map((p) => ({ x: p.x + dx, y: p.y + dy })) };
  if (object.type === "text") return { ...object, x: object.x + dx, y: object.y + dy };
  if ("x" in object && "y" in object) return { ...object, x: object.x + dx, y: object.y + dy };
  return object;
}

function recolor(doc: Document, ids: string[], color: string): Document {
  return {
    ...doc,
    objects: doc.objects.map((object) => {
      if (!ids.includes(idOf(object))) return object;
      if (object.type === "highlight") return { ...object, color };
      if (object.type === "text") return { ...object, color };
      if (object.type === "counter") return { ...object, fill: color };
      if ("stroke" in object) return { ...object, stroke: color };
      return object;
    }),
  };
}

function updateText(doc: Document, id: string, text: string): Document {
  return { ...doc, objects: doc.objects.map((object) => object.type === "text" && object.id === id ? { ...object, text } : object) };
}

function renumber(doc: Document) {
  let n = 1;
  for (const object of doc.objects) {
    if (object.type === "counter") object.number = n++;
  }
}
