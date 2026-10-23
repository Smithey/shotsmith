import { useEffect, useRef, useState } from "react";
import { api, assetUrl, onRefresh, uid, SWATCHES, type AnnotationObject, type Document, type EditorState, type Preset } from "../api";
import type { Effect } from "../bindings/Effect";
import { bounds, drawDocument, hitTest, idOf, whenBitmapsLoad } from "./draw";

type Tool = "select" | "crop" | "arrow" | "rect" | "ellipse" | "line" | "freehand" | "text" | "callout" | "counter" | "highlight" | "obfuscate" | "magnify" | "eyedropper";

const KEYS: Record<string, Tool> = {
  v: "select", c: "crop", a: "arrow", r: "rect", e: "ellipse", l: "line", f: "freehand", t: "text", b: "callout", n: "counter", h: "highlight", o: "obfuscate", m: "magnify", i: "eyedropper",
};
const TOOL_LABEL: Record<Tool, string> = {
  select: "Select", crop: "Crop", arrow: "Arrow", rect: "Rectangle", ellipse: "Ellipse", line: "Line", freehand: "Freehand", text: "Text", callout: "Callout", counter: "Step counter", highlight: "Highlight", obfuscate: "Obfuscate", magnify: "Magnify", eyedropper: "Eyedropper",
};

export function Editor() {
  const [state, setState] = useState<EditorState | null>(null);
  const [doc, setDoc] = useState<Document | null>(null);
  const [tool, setTool] = useState<Tool>("arrow");
  const [selected, setSelected] = useState<string[]>([]);
  const [stroke, setStroke] = useState("#FF3B30");
  const strokeWidth = 4;
  const [fill, setFill] = useState("#FFFFFF");
  const [textSize, setTextSize] = useState(18);
  const [blockSize, setBlockSize] = useState(12);
  const [presets, setPresets] = useState<Preset[]>([]);
  const [presetId, setPresetId] = useState("builtin-docs-png");
  const [zoom, setZoom] = useState(1);
  const [pan, setPan] = useState({ x: 40, y: 40 });
  const [error, setError] = useState("");
  const [askClose, setAskClose] = useState(false);
  const [saveMenu, setSaveMenu] = useState(false);
  const saveWrap = useRef<HTMLDivElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const imageRef = useRef<HTMLImageElement | null>(null);
  const drag = useRef<{ x: number; y: number; object: AnnotationObject | null; points: { x: number; y: number }[] } | null>(null);

  async function load() {
    const [next, list, settings] = await Promise.all([api.editor(), api.presets(), api.settings()]);
    setState(next);
    setDoc(next.document);
    setPresets(list);
    setPresetId(settings.defaultPresetId);
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
    if (tool === "text" || tool === "callout") {
      const object: AnnotationObject = { type: "text", id: uid(), x: p.x, y: p.y, text: tool === "callout" ? "Note" : "Text", size: textSize, color: stroke, fill, bold: true };
      commit({ ...doc, objects: [...doc.objects, object] });
      setSelected([object.id]);
      return;
    }
    if (tool === "magnify" || tool === "eyedropper") return;
    if (tool === "counter") {
      const number = doc.objects.reduce((max, object) => object.type === "counter" ? Math.max(max, object.number) : max, 0) + 1;
      const object: AnnotationObject = { type: "counter", id: uid(), x: p.x, y: p.y, number, style: "circle", fill: stroke, size: 28 };
      commit({ ...doc, objects: [...doc.objects, object] });
      return;
    }
    const object = startObject(tool, p, stroke, strokeWidth, fill, blockSize);
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

  useEffect(() => {
    if (!saveMenu) return;
    const onPointer = (event: PointerEvent) => {
      const wrap = saveWrap.current;
      if (wrap && event.target instanceof Node && !wrap.contains(event.target)) setSaveMenu(false);
    };
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") setSaveMenu(false);
    };
    window.addEventListener("pointerdown", onPointer);
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("pointerdown", onPointer);
      window.removeEventListener("keydown", onKey);
    };
  }, [saveMenu]);

  async function save(saveAs: boolean, id = presetId) {
    setSaveMenu(false);
    try {
      if (saveAs) {
        const path = await api.pickSave(`${preset?.name ?? "capture"}.png`);
        if (!path) return;
        await api.saveEditor(null, path);
      } else {
        await api.saveEditor(id, null);
      }
      const next = await api.editor();
      setState(next);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Save failed.");
    }
  }

  function savePng() {
    void save(false, pngPreset?.id ?? presetId);
  }

  function fit() {
    const canvas = canvasRef.current;
    if (!canvas || !doc) return 1;
    return Math.min(canvas.clientWidth / doc.width, canvas.clientHeight / doc.height) * 0.9;
  }

  const selectedObject = doc?.objects.find((object) => selected.includes(idOf(object))) ?? null;
  const preset = presets.find((item) => item.id === presetId) ?? presets[0];
  const pngPreset = presets.find((item) => item.id === "builtin-docs-png") ?? presets.find((item) => item.format === "png" && item.folder);
  const section = selectedObject?.type === "text" ? "Text · selected" : selectedObject ? `${TOOL_LABEL[selectedObject.type as Tool] ?? selectedObject.type} · selected` : TOOL_LABEL[tool];
  const activeFill = selectedObject?.type === "text" ? selectedObject.fill : fill;
  const activeSize = selectedObject?.type === "text" ? selectedObject.size : textSize;
  const obfuscate = selectedObject?.type === "obfuscate" ? selectedObject : null;

  function paintFill(next: string) {
    setFill(next);
    if (selectedObject?.type === "text" && doc) commit(updateTextFill(doc, selectedObject.id, next));
  }

  function paintSize(next: number) {
    setTextSize(next);
    if (selectedObject?.type === "text" && doc) {
      commit({ ...doc, objects: doc.objects.map((object) => object.type === "text" && object.id === selectedObject.id ? { ...object, size: next } : object) });
    }
  }

  function setObfuscateMode(mode: "pixelate" | "blur" | "solid") {
    if (!obfuscate || !doc) return;
    commit({ ...doc, objects: doc.objects.map((object) => object.type === "obfuscate" && object.id === obfuscate.id ? { ...object, mode } : object) });
  }

  function setObfuscateBlock(next: number) {
    setBlockSize(next);
    if (!obfuscate || !doc) return;
    commit({ ...doc, objects: doc.objects.map((object) => object.type === "obfuscate" && object.id === obfuscate.id ? { ...object, blockSize: next } : object) });
  }

  function toggleEffect(effect: Effect) {
    if (!preset) return;
    const effects = preset.effects.includes(effect) ? preset.effects.filter((item) => item !== effect) : [...preset.effects, effect];
    api.savePreset({ ...preset, effects }).then(setPresets).catch((err: Error) => setError(err.message));
  }

  return (
    <div className="editor">
      <header className="ed-bar">
        <div className="ed-title">
          <strong>Capture</strong>
          <span>{state?.dirty ? "Edited · not saved" : "Saved to History"}</span>
        </div>
        <div className="ed-tools" role="toolbar" aria-label="Tools">
          <ToolButton id="select" tool={tool} setTool={setTool} />
          <ToolButton id="crop" tool={tool} setTool={setTool} />
          <span className="ed-gap" />
          {(["arrow", "rect", "ellipse", "line", "freehand", "text", "callout", "counter"] as Tool[]).map((id) => <ToolButton key={id} id={id} tool={tool} setTool={setTool} />)}
          <span className="ed-gap" />
          {(["highlight", "obfuscate", "magnify", "eyedropper"] as Tool[]).map((id) => <ToolButton key={id} id={id} tool={tool} setTool={setTool} />)}
        </div>
        <div className="ed-actions">
          <button type="button" className="ed-icon" aria-label="Undo" onClick={() => api.undo().then(applyRemote)}><Icon name="undo" /></button>
          <button type="button" className="ed-icon" aria-label="Copy to clipboard" onClick={() => api.copyEditor().catch((err: Error) => setError(err.message))}><Icon name="copy" /></button>
          <button type="button" className="ed-icon" aria-label="Share" onClick={() => api.copyEditor().catch((err: Error) => setError(err.message))}><Icon name="share" /></button>
          <div className="ed-save-wrap" ref={saveWrap}>
            <div className="ed-save">
              <button type="button" onClick={() => savePng()}>Save PNG</button>
              <button type="button" aria-label="More save actions" aria-expanded={saveMenu} onClick={() => setSaveMenu((open) => !open)}><Icon name="chevron" /></button>
            </div>
            {saveMenu && (
              <div className="ed-save-menu" role="menu">
                <button type="button" role="menuitem" onClick={() => { setSaveMenu(false); api.copyEditor().catch((err: Error) => setError(err.message)); }}>Copy</button>
                <button type="button" role="menuitem" onClick={() => savePng()}>Save PNG</button>
                {presets.filter((item) => item.id !== pngPreset?.id && item.id !== "builtin-clipboard").map((item) => (
                  <button key={item.id} type="button" role="menuitem" onClick={() => { setPresetId(item.id); void save(false, item.id); }}>
                    <span>{item.name}</span>
                    <span>{item.format.toUpperCase()}</span>
                  </button>
                ))}
                <button type="button" role="menuitem" className="ed-save-as" onClick={() => save(true)}>Save as…</button>
              </div>
            )}
          </div>
        </div>
      </header>
      <div className="stage">
        <div className="canvas-wrap">
          <canvas ref={canvasRef} onPointerDown={onDown} onPointerMove={onMove} onPointerUp={onUp} />
          <div className="ed-zoom">
            <button type="button" aria-label="Zoom out" onClick={() => setZoom((z) => Math.max(0.1, z / 1.15))}>−</button>
            <span>{Math.round(zoom * 100)}%</span>
            <button type="button" aria-label="Zoom in" onClick={() => setZoom((z) => Math.min(8, z * 1.15))}>+</button>
            <span className="ed-gap" />
            <button type="button" onClick={() => setZoom(fit())}>Fit ⌘0</button>
            <button type="button" onClick={() => { setZoom(1); setPan({ x: 40, y: 40 }); }}>100% ⌘1</button>
          </div>
        </div>
        <aside className="ed-inspector">
          <div className="ed-block">
            <span className="ed-kicker">{section}</span>
            <span>Colour</span>
            <div className="ed-swatches">
              {SWATCHES.map((color) => (
                <button key={color} type="button" aria-label={color} className={stroke.toLowerCase() === color.toLowerCase() ? "on" : ""} style={{ background: color, color }} onClick={() => { setStroke(color); if (selectedObject && doc) commit(recolor(doc, selected, color)); }} />
              ))}
              <button type="button" className="add" aria-label="Custom colour">+</button>
            </div>
            <div className="ed-line"><label htmlFor="ed-font">Font</label><select id="ed-font" value={selectedObject?.type === "text" && selectedObject.bold ? "bold" : "regular"} onChange={(event) => {
              if (selectedObject?.type === "text" && doc) commit({ ...doc, objects: doc.objects.map((object) => object.type === "text" && object.id === selectedObject.id ? { ...object, bold: event.target.value === "bold" } : object) });
            }}><option value="bold">System · Bold</option><option value="regular">System</option></select></div>
            <div className="ed-line"><label htmlFor="ed-size">Size</label><input id="ed-size" type="range" min={10} max={72} value={activeSize} onChange={(event) => paintSize(Number(event.target.value))} /><span>{Math.round(activeSize)}</span></div>
            <div className="ed-line"><span>Fill</span>
              <div className="ed-segment">
                <button type="button" className={activeFill === "none" ? "on" : ""} onClick={() => paintFill("none")}>None</button>
                <button type="button" className={activeFill.toLowerCase() === "#ffffff" ? "on" : ""} onClick={() => paintFill("#FFFFFF")}>White</button>
                <button type="button" className={activeFill.toLowerCase() === stroke.toLowerCase() ? "on" : ""} onClick={() => paintFill(stroke)}>Colour</button>
              </div>
            </div>
            {selectedObject && "shadow" in selectedObject && (
              <label className="ed-check">Shadow <input type="checkbox" checked={selectedObject.shadow} onChange={(event) => doc && commit({ ...doc, objects: doc.objects.map((object) => idOf(object) === idOf(selectedObject) && "shadow" in object ? { ...object, shadow: event.target.checked } : object) })} /></label>
            )}
            {selectedObject?.type === "text" && (
              <textarea className="ed-text" value={selectedObject.text} onChange={(event) => doc && commit(updateText(doc, selectedObject.id, event.target.value))} />
            )}
          </div>
          <div className="ed-rule" />
          <div className="ed-block">
            <span className="ed-kicker">Obfuscate</span>
            <div className="ed-segment">
              {(["pixelate", "blur", "solid"] as const).map((mode) => (
                <button key={mode} type="button" className={(obfuscate?.mode ?? "pixelate") === mode ? "on" : ""} onClick={() => setObfuscateMode(mode)}>{mode === "pixelate" ? "Pixelate" : mode === "blur" ? "Blur" : "Solid"}</button>
              ))}
            </div>
            <div className="ed-line"><label htmlFor="ed-px">Block size</label><input id="ed-px" type="range" min={4} max={40} value={obfuscate?.blockSize ?? blockSize} onChange={(event) => setObfuscateBlock(Number(event.target.value))} /><span>{obfuscate?.blockSize ?? blockSize}</span></div>
            <p>Flattened on export. Original pixels are never written to the file.</p>
          </div>
          <div className="ed-rule" />
          <div className="ed-block">
            <span className="ed-kicker">Image effects</span>
            {([["shadow", "Drop shadow"], ["border", "Border"], ["tornEdges", "Torn edges"], ["rounded", "Rounded corners"], ["grayscale", "Grayscale"], ["invert", "Invert"]] as Array<[Effect, string]>).map(([effect, label]) => (
              <label key={effect} className="ed-check">{label} <input type="checkbox" checked={preset?.effects.includes(effect) ?? false} onChange={() => toggleEffect(effect)} /></label>
            ))}
          </div>
          {error && <p className="error">{error}</p>}
        </aside>
      </div>
      <footer className="ed-status">
        <span>{doc ? `${doc.width} × ${doc.height} px` : ""}</span>
        <span>{doc?.objects.length ?? 0} objects</span>
        <span>{TOOL_LABEL[tool]} tool: {Object.entries(KEYS).find(([, value]) => value === tool)?.[0].toUpperCase() ?? ""} · Esc to select</span>
        <span className="grow">{preset?.folder?.replace(/^\/Users\/[^/]+/, "~") ?? "Clipboard only"}</span>
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

function startObject(tool: Tool, p: { x: number; y: number }, stroke: string, strokeWidth: number, fill: string, blockSize: number): AnnotationObject | null {
  const id = uid();
  if (tool === "rect" || tool === "crop") return { type: "rect", id, x: p.x, y: p.y, w: 1, h: 1, stroke: tool === "crop" ? "#0A84FF" : stroke, strokeWidth: 2, fill: "none", radius: 0, opacity: 1, shadow: false };
  if (tool === "ellipse") return { type: "ellipse", id, x: p.x, y: p.y, w: 1, h: 1, stroke, strokeWidth, fill, opacity: 1, shadow: false };
  if (tool === "line") return { type: "line", id, x1: p.x, y1: p.y, x2: p.x, y2: p.y, stroke, strokeWidth, shadow: false };
  if (tool === "arrow") return { type: "arrow", id, x1: p.x, y1: p.y, x2: p.x, y2: p.y, cx: null, cy: null, heads: "end", stroke, strokeWidth, shadow: true };
  if (tool === "freehand") return { type: "freehand", id, points: [p], stroke, strokeWidth };
  if (tool === "highlight") return { type: "highlight", id, x: p.x, y: p.y, w: 1, h: 1, mode: "marker", color: stroke, opacity: 0.35 };
  if (tool === "obfuscate") return { type: "obfuscate", id, x: p.x, y: p.y, w: 1, h: 1, mode: "pixelate", blockSize, color: "#000000" };
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

function updateTextFill(doc: Document, id: string, fill: string): Document {
  return { ...doc, objects: doc.objects.map((object) => object.type === "text" && object.id === id ? { ...object, fill } : object) };
}

function ToolButton({ id, tool, setTool }: { id: Tool; tool: Tool; setTool: (tool: Tool) => void }) {
  const key = Object.entries(KEYS).find(([, value]) => value === id)?.[0].toUpperCase();
  return (
    <button type="button" className={tool === id ? "on" : ""} aria-label={`${TOOL_LABEL[id]} (${key})`} onClick={() => setTool(id)}>
      <Icon name={id} />
    </button>
  );
}

function Icon({ name }: { name: string }) {
  const common = { fill: "none", stroke: "currentColor", strokeWidth: 1.8, strokeLinecap: "round" as const, strokeLinejoin: "round" as const };
  return (
    <svg width={name === "chevron" ? 10 : 17} height={name === "chevron" ? 10 : 17} viewBox="0 0 24 24" aria-hidden="true">
      {name === "select" && <path {...common} d="M5 3l14 8-6 2-2 6z" />}
      {name === "crop" && <path {...common} d="M6 2v16h16M2 6h16v16" />}
      {name === "arrow" && <path {...common} strokeWidth={2} d="M5 19L19 5M10 5h9v9" />}
      {name === "rect" && <rect {...common} x="4" y="6" width="16" height="12" rx="1.5" />}
      {name === "ellipse" && <ellipse {...common} cx="12" cy="12" rx="8" ry="6" />}
      {name === "line" && <path {...common} d="M5 19L19 5" />}
      {name === "freehand" && <path {...common} d="M3 16c3-7 5 2 8-3s5-6 10-3" />}
      {name === "text" && <path {...common} d="M5 6V4h14v2M12 4v16M9 20h6" />}
      {name === "callout" && <path {...common} d="M4 5h16v11H10l-4 4v-4H4z" />}
      {name === "counter" && <><circle {...common} cx="12" cy="12" r="9" /><path {...common} d="M11 9l2-1v8" /></>}
      {name === "highlight" && <path {...common} d="M9 15l-4 4h6l1-1M9 15l8-8 3 3-8 8z" />}
      {name === "obfuscate" && <g fill="currentColor"><rect x="4" y="4" width="5" height="5" /><rect x="15" y="4" width="5" height="5" /><rect x="9.5" y="9.5" width="5" height="5" /><rect x="4" y="15" width="5" height="5" /><rect x="15" y="15" width="5" height="5" /></g>}
      {name === "magnify" && <><circle {...common} cx="11" cy="11" r="6" /><path {...common} d="M16 16l4 4M11 8v6M8 11h6" /></>}
      {name === "eyedropper" && <path {...common} d="M14 4l6 6-2 2-6-6zM12 6l-8 8v4h4l8-8" />}
      {name === "undo" && <path {...common} d="M9 14L4 9l5-5M4 9h11a5 5 0 010 10h-4" />}
      {name === "copy" && <><rect {...common} x="8" y="8" width="12" height="12" rx="2" /><path {...common} d="M16 8V5a1 1 0 00-1-1H5a1 1 0 00-1 1v10a1 1 0 001 1h3" /></>}
      {name === "share" && <path {...common} d="M12 3v12M8 7l4-4 4 4M5 12v7a1 1 0 001 1h12a1 1 0 001-1v-7" />}
      {name === "chevron" && <path {...common} strokeWidth={3} d="M6 9l6 6 6-6" />}
    </svg>
  );
}

function renumber(doc: Document) {
  let n = 1;
  for (const object of doc.objects) {
    if (object.type === "counter") object.number = n++;
  }
}
