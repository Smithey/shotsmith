import { useEffect, useRef, useState } from "react";
import { api, onRefresh, uid, SWATCHES, type AnnotationObject, type Document, type EditorState, type Preset } from "../api";
import type { Effect } from "../bindings/Effect";
import { bounds, drawCrop, drawDocument, hitCenter, hitHandle, hitsBox, hitTest, idOf, whenBitmapsLoad, type Handle } from "./draw";

type Tool = "select" | "crop" | "arrow" | "rect" | "ellipse" | "line" | "freehand" | "text" | "callout" | "counter" | "highlight" | "obfuscate" | "magnify" | "eyedropper";

const COLOR_NAME: Record<string, string> = {
  "#FF3B30": "Red",
  "#FF9500": "Orange",
  "#FFCC00": "Yellow",
  "#34C759": "Green",
  "#007AFF": "Blue",
  "#AF52DE": "Purple",
  "#1D1D1F": "Black",
};

const KEYS: Record<string, Tool> = {
  v: "select", c: "crop", a: "arrow", r: "rect", e: "ellipse", l: "line", f: "freehand", t: "text", b: "callout", n: "counter", h: "highlight", o: "obfuscate", m: "magnify", i: "eyedropper",
};
const TOOL_LABEL: Record<Tool, string> = {
  select: "Select", crop: "Crop", arrow: "Arrow", rect: "Rectangle", ellipse: "Ellipse", line: "Line", freehand: "Freehand", text: "Text", callout: "Text", counter: "Step counter", highlight: "Highlight", obfuscate: "Obfuscate", magnify: "Magnify", eyedropper: "Eyedropper",
};
const LATER = new Set<Tool>(["magnify", "eyedropper"]);
const SHADOW = { shadowBlur: 8, shadowX: 0, shadowY: 3, shadowOpacity: 0.35 };

export function Editor() {
  const [state, setState] = useState<EditorState | null>(null);
  const [doc, setDoc] = useState<Document | null>(null);
  const [tool, setTool] = useState<Tool>("arrow");
  const [selected, setSelected] = useState<string[]>([]);
  const [stroke, setStroke] = useState("#FF3B30");
  const [strokeWidth, setStrokeWidth] = useState(4);
  const [fill, setFill] = useState("#FFFFFF");
  const [textSize, setTextSize] = useState(18);
  const [blockSize, setBlockSize] = useState(12);
  const [shadowBlur, setShadowBlur] = useState(8);
  const [shadowX, setShadowX] = useState(0);
  const [shadowY, setShadowY] = useState(3);
  const [shadowOpacity, setShadowOpacity] = useState(35);
  const [coverMode, setCoverMode] = useState<"pixelate" | "blur" | "solid">("pixelate");
  const [coverColor, setCoverColor] = useState("#1D1D1F");
  const [presets, setPresets] = useState<Preset[]>([]);
  const [presetId, setPresetId] = useState("builtin-docs-png");
  const [zoom, setZoom] = useState(1);
  const [pan, setPan] = useState({ x: 40, y: 40 });
  const [cropBox, setCropBox] = useState<{ x: number; y: number; w: number; h: number } | null>(null);
  const [marquee, setMarquee] = useState<{ x: number; y: number; w: number; h: number } | null>(null);
  const [editing, setEditing] = useState<string | null>(null);
  const [imageRev, setImageRev] = useState(0);
  const [error, setError] = useState("");
  const [askClose, setAskClose] = useState(false);
  const [saveMenu, setSaveMenu] = useState(false);
  const saveWrap = useRef<HTMLDivElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const imageRef = useRef<HTMLImageElement | null>(null);
  const drag = useRef<
    | { mode: "move"; x: number; y: number; ids: string[]; moved: boolean }
    | { mode: "resize"; handle: Handle; object: AnnotationObject; moved: boolean }
    | { mode: "draw"; x: number; y: number; object: AnnotationObject; points: { x: number; y: number }[]; moved: boolean }
    | { mode: "crop"; x: number; y: number; box: { x: number; y: number; w: number; h: number } | null }
    | { mode: "marquee"; x: number; y: number; base: string[] }
    | null
  >(null);
  const editRef = useRef<HTMLTextAreaElement>(null);
  const pending = useRef(Promise.resolve());
  const docRef = useRef(doc);
  const frame = useRef(0);
  const canvasSize = useRef({ w: 0, h: 0 });
  const gesture = useRef(false);
  const arrowing = useRef(false);
  const view = useRef({ zoom, pan, selected, editing, cropBox, marquee, fast: false });
  if (!gesture.current) docRef.current = doc;
  view.current = { ...view.current, zoom, pan, selected, editing, cropBox, marquee };

  async function load() {
    const [next, list, settings] = await Promise.all([api.editor(), api.presets(), api.settings()]);
    setState(next);
    setDoc(next.document);
    setPresets(list);
    setPresetId(settings.defaultPresetId);
    await showPicture();
  }

  async function showPicture() {
    const encoded = await api.editorImage();
    await new Promise<void>((resolve, reject) => {
      const image = new Image();
      image.onload = () => {
        imageRef.current = image;
        setImageRev((rev) => rev + 1);
        resolve();
      };
      image.onerror = () => reject(new Error("Could not reload the image."));
      image.src = `data:image/png;base64,${encoded}`;
    });
  }

  function paint(document = docRef.current, ids = view.current.selected, fast = view.current.fast) {
    const canvas = canvasRef.current;
    if (!canvas || !document) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    const rect = canvas.getBoundingClientRect();
    const width = Math.max(1, Math.round(rect.width * devicePixelRatio));
    const height = Math.max(1, Math.round(rect.height * devicePixelRatio));
    if (canvasSize.current.w !== width || canvasSize.current.h !== height) {
      canvas.width = width;
      canvas.height = height;
      canvasSize.current = { w: width, h: height };
    }
    const { zoom: z, pan: p, editing: editingId, cropBox: crop, marquee: box } = view.current;
    ctx.setTransform(devicePixelRatio, 0, 0, devicePixelRatio, 0, 0);
    ctx.clearRect(0, 0, rect.width, rect.height);
    const image = imageRef.current;
    const ready = image != null && image.complete && image.naturalWidth === document.width && image.naturalHeight === document.height;
    drawDocument(ctx, ready ? image : null, document, z, p.x, p.y, ids, editingId, fast);
    if (crop) drawCrop(ctx, document, crop, z, p.x, p.y);
    if (box) {
      ctx.save();
      ctx.translate(p.x, p.y);
      ctx.scale(z, z);
      ctx.strokeStyle = "#0A84FF";
      ctx.lineWidth = 1 / z;
      ctx.setLineDash([4 / z, 3 / z]);
      ctx.strokeRect(box.x, box.y, box.w, box.h);
      ctx.restore();
    }
  }

  function schedulePaint(fast = false) {
    if (fast) view.current.fast = true;
    if (frame.current) return;
    frame.current = requestAnimationFrame(() => {
      frame.current = 0;
      paint();
    });
  }

  function preview(next: Document, fast = false) {
    docRef.current = next;
    gesture.current = true;
    view.current.fast = fast;
    schedulePaint(fast);
  }

  function endGesture() {
    if (!gesture.current) return;
    gesture.current = false;
    view.current.fast = false;
    if (frame.current) {
      cancelAnimationFrame(frame.current);
      frame.current = 0;
    }
    const current = docRef.current;
    if (!current) return;
    paint(current, view.current.selected, false);
    commit(current);
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
  }, [doc, zoom, pan, selected, cropBox, marquee, editing, imageRev]);

  useEffect(() => {
    const object = doc?.objects.find((item) => item.id === selected[selected.length - 1]);
    if (!object) return;
    if ("strokeWidth" in object) setStrokeWidth(object.strokeWidth);
    if (object.type === "text" || object.type === "counter") setTextSize(object.size);
    if (object.type === "obfuscate") setBlockSize(object.blockSize);
    if ("shadowBlur" in object) {
      setShadowBlur(object.shadowBlur);
      setShadowX(object.shadowX);
      setShadowY(object.shadowY);
      setShadowOpacity(Math.round(object.shadowOpacity * 100));
    }
  }, [doc, selected]);

  useEffect(() => {
    if (!editing) return;
    editRef.current?.focus();
  }, [editing]);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      const target = event.target as HTMLElement;
      if (target.tagName === "INPUT" || target.tagName === "TEXTAREA" || target.tagName === "SELECT") return;
      const next = KEYS[event.key.toLowerCase()];
      if (next && !LATER.has(next) && !event.metaKey && !event.ctrlKey) setTool(next);
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
      const current = docRef.current;
      const ids = view.current.selected;
      if ((event.key === "Delete" || event.key === "Backspace") && ids.length && current) {
        const next = { ...current, objects: current.objects.filter((object) => !ids.includes(idOf(object))) };
        renumber(next);
        commit(next);
        setSelected([]);
      }
      if (current && ids.length && ["ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight"].includes(event.key)) {
        const step = event.shiftKey ? 10 : 1;
        const dx = event.key === "ArrowLeft" ? -step : event.key === "ArrowRight" ? step : 0;
        const dy = event.key === "ArrowUp" ? -step : event.key === "ArrowDown" ? step : 0;
        arrowing.current = true;
        const moved = moveSelected(current, ids, dx, dy);
        preview(moved, liveObfuscate(moved));
      }
    };
    const onKeyUp = (event: KeyboardEvent) => {
      if (!arrowing.current) return;
      if (!["ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight"].includes(event.key)) return;
      arrowing.current = false;
      endGesture();
    };
    window.addEventListener("keydown", onKey);
    window.addEventListener("keyup", onKeyUp);
    window.addEventListener("beforeunload", (event) => { if (state?.dirty) event.preventDefault(); });
    return () => {
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("keyup", onKeyUp);
    };
  }, [state, zoom]);

  function applyRemote(next: EditorState) {
    docRef.current = next.document;
    setState(next);
    setDoc(next.document);
    if (next.pixelsChanged) showPicture().catch((err: Error) => setError(err.message));
  }

  function enqueue(job: () => Promise<unknown>) {
    pending.current = pending.current.then(job).then(() => undefined, () => undefined);
  }

  function commit(next: Document) {
    docRef.current = next;
    setDoc(next);
    enqueue(() => api.commit(next).then(setState).catch((err: Error) => setError(err.message)));
  }

  function localPoint(event: React.PointerEvent) {
    const bounds = canvasRef.current!.getBoundingClientRect();
    return { x: (event.clientX - bounds.left - pan.x) / zoom, y: (event.clientY - bounds.top - pan.y) / zoom };
  }

  function remember(next: Document) {
    docRef.current = next;
    setDoc(next);
  }

  function liveObfuscate(next: Document) {
    return next.objects.some((object) => object.type === "obfuscate" && object.mode !== "solid");
  }

  function onDown(event: React.PointerEvent) {
    const current = docRef.current;
    if (!current) return;
    event.currentTarget.setPointerCapture(event.pointerId);
    const p = localPoint(event);
    if (tool === "crop") {
      setSelected([]);
      setCropBox(null);
      drag.current = { mode: "crop", x: p.x, y: p.y, box: null };
      return;
    }
    const primary = current.objects.find((object) => object.id === selected[selected.length - 1]);
    if (primary) {
      const handle = hitHandle(primary, p.x, p.y, zoom);
      if (handle && primary.type !== "cursor" && primary.type !== "image") {
        drag.current = { mode: "resize", handle, object: primary, moved: false };
        return;
      }
    }
    if (tool === "select") {
      const hit = hitTest(current, p.x, p.y);
      if (!hit) {
        drag.current = { mode: "marquee", x: p.x, y: p.y, base: event.shiftKey ? selected : [] };
        if (!event.shiftKey) setSelected([]);
        setMarquee(null);
        return;
      }
      const ids = event.shiftKey
        ? selected.includes(hit) ? selected.filter((id) => id !== hit) : [...selected, hit]
        : selected.includes(hit) ? selected : [hit];
      setSelected(ids);
      drag.current = { mode: "move", x: p.x, y: p.y, ids, moved: false };
      return;
    }
    if (primary && hitTest(current, p.x, p.y) === primary.id) {
      drag.current = { mode: "move", x: p.x, y: p.y, ids: selected.includes(primary.id) ? selected : [primary.id], moved: false };
      return;
    }
    const center = hitCenter(current, p.x, p.y, zoom);
    if (center) {
      setSelected([center]);
      setTool("select");
      drag.current = { mode: "move", x: p.x, y: p.y, ids: [center], moved: false };
      return;
    }
    if (tool === "text" || tool === "callout") {
      const object: AnnotationObject = {
        type: "text",
        id: uid(),
        x: p.x,
        y: p.y,
        text: "",
        size: textSize,
        color: stroke,
        fill: tool === "callout" ? (fill === "none" ? "#FFFFFF" : fill) : fill,
        bold: true,
      };
      commit({ ...current, objects: [...current.objects, object] });
      setSelected([object.id]);
      setEditing(object.id);
      setTool("select");
      return;
    }
    if (tool === "magnify" || tool === "eyedropper") return;
    if (tool === "counter") {
      const number = current.objects.reduce((max, object) => object.type === "counter" ? Math.max(max, object.number) : max, 0) + 1;
      const object: AnnotationObject = { type: "counter", id: uid(), x: p.x, y: p.y, number, style: "circle", fill: stroke, size: 28 };
      commit({ ...current, objects: [...current.objects, object] });
      setSelected([object.id]);
      return;
    }
    const object = startObject(tool, p, stroke, strokeWidth, fill, blockSize, coverMode, coverColor);
    if (!object) return;
    drag.current = { mode: "draw", x: p.x, y: p.y, object, points: [p], moved: false };
  }

  function onMove(event: React.PointerEvent) {
    const active = drag.current;
    const current = docRef.current;
    if (!active || !current) return;
    const p = localPoint(event);
    if (active.mode === "marquee") {
      const w = p.x - active.x;
      const h = p.y - active.y;
      const box = { x: w < 0 ? active.x + w : active.x, y: h < 0 ? active.y + h : active.y, w: Math.abs(w), h: Math.abs(h) };
      setMarquee(box.w >= 1 && box.h >= 1 ? box : null);
      const hits = hitsBox(current, box);
      setSelected([...active.base, ...hits.filter((id) => !active.base.includes(id))]);
      return;
    }
    if (active.mode === "crop") {
      const w = p.x - active.x;
      const h = event.shiftKey ? Math.sign(p.y - active.y || 1) * Math.abs(w) : p.y - active.y;
      const left = w < 0 ? active.x + w : active.x;
      const top = h < 0 ? active.y + h : active.y;
      const box = clampBox(current, left, top, Math.abs(w), Math.abs(h));
      active.box = box.w >= 1 && box.h >= 1 ? box : null;
      setCropBox(active.box);
      return;
    }
    if (active.mode === "move") {
      if (!active.ids.length) return;
      const dx = p.x - active.x;
      const dy = p.y - active.y;
      if (dx === 0 && dy === 0) return;
      active.x = p.x;
      active.y = p.y;
      active.moved = true;
      const moved = moveSelected(current, active.ids, dx, dy);
      preview(moved, liveObfuscate(moved));
      return;
    }
    if (active.mode === "resize") {
      active.moved = true;
      const next = applyHandle(active.object, active.handle, p);
      const resized = { ...current, objects: current.objects.map((object) => object.id === next.id ? next : object) };
      preview(resized, liveObfuscate(resized));
      return;
    }
    active.moved = true;
    if (active.object.type === "freehand") {
      active.points.push(p);
      active.object = { ...active.object, points: active.points };
    } else {
      const w = p.x - active.x;
      const h = event.shiftKey ? Math.sign(p.y - active.y || 1) * Math.abs(w) : p.y - active.y;
      active.object = resizeObject(active.object, active.x, active.y, w, h, event.altKey);
    }
    const drawn = { ...current, objects: [...current.objects.filter((item) => item.id !== active.object.id), active.object] };
    preview(drawn, liveObfuscate(drawn));
  }

  function onUp() {
    const active = drag.current;
    drag.current = null;
    const current = docRef.current;
    if (!active || !current) return;
    if (active.mode === "marquee") {
      setMarquee(null);
      return;
    }
    if (active.mode === "crop") {
      const box = active.box;
      setCropBox(null);
      if (!box || box.w < 4 || box.h < 4) return;
      const x = Math.round(box.x);
      const y = Math.round(box.y);
      const w = Math.round(box.w);
      const h = Math.round(box.h);
      enqueue(() => api.crop(x, y, w, h).then(applyRemote).catch((err: unknown) => setError(err instanceof Error ? err.message : String(err))));
      return;
    }
    if (active.mode === "move" || active.mode === "resize") {
      if (active.moved) endGesture();
      return;
    }
    const box = bounds(active.object);
    if (!active.moved || (box && box.w < 2 && box.h < 2 && active.object.type !== "freehand")) {
      gesture.current = false;
      view.current.fast = false;
      remember({ ...current, objects: current.objects.filter((item) => item.id !== active.object.id) });
      return;
    }
    docRef.current = { ...current, objects: [...current.objects.filter((item) => item.id !== active.object.id), active.object] };
    setSelected([active.object.id]);
    endGesture();
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

  const selectedObject = doc?.objects.find((object) => object.id === selected[selected.length - 1]) ?? null;
  const preset = presets.find((item) => item.id === presetId) ?? presets[0];
  const pngPreset = presets.find((item) => item.id === "builtin-docs-png") ?? presets.find((item) => item.format === "png" && item.folder);
  const section = selectedObject?.type === "text" ? "Text · selected" : selectedObject ? `${TOOL_LABEL[selectedObject.type as Tool] ?? selectedObject.type} · selected` : TOOL_LABEL[tool];
  const shownColor = colorOf(selectedObject) ?? stroke;
  const shownFill = selectedObject && "fill" in selectedObject && selectedObject.type !== "counter" ? selectedObject.fill : fill;
  const shownWeight = strokeWidth;
  const shownSize = textSize;
  const showText = selectedObject ? selectedObject.type === "text" : tool === "text" || tool === "callout";
  const showFill = showText || (selectedObject ? selectedObject.type === "rect" || selectedObject.type === "ellipse" : tool === "rect" || tool === "ellipse");
  const showWeight = selectedObject ? "strokeWidth" in selectedObject : ["arrow", "rect", "ellipse", "line", "freehand"].includes(tool);
  const showObfuscate = selectedObject ? selectedObject.type === "obfuscate" : tool === "obfuscate";
  const showColor = selectedObject ? selectedObject.type !== "obfuscate" && colorOf(selectedObject) != null : ["arrow", "rect", "ellipse", "line", "freehand", "text", "callout", "counter", "highlight"].includes(tool);
  const showShape = showColor || showText || showWeight || showFill || (selectedObject != null && "shadow" in selectedObject);
  const showEffects = !selectedObject && tool === "select";
  const obfuscate = selectedObject?.type === "obfuscate" ? selectedObject : null;
  const obfuscateMode = obfuscate?.mode ?? coverMode;
  const editingObject = doc?.objects.find((object): object is Extract<AnnotationObject, { type: "text" }> => object.id === editing && object.type === "text");
  const editingBox = editingObject ? bounds(editingObject) : null;

  function paintFill(next: string) {
    setFill(next);
    if (!doc || !selectedObject) return;
    if (selectedObject.type === "text" || selectedObject.type === "rect" || selectedObject.type === "ellipse") {
      commit({ ...doc, objects: doc.objects.map((object) => object.id === selectedObject.id && "fill" in object ? { ...object, fill: next } : object) });
    }
  }

  function paintSize(next: number) {
    setTextSize(next);
    const current = docRef.current;
    if (!current || !selectedObject) return;
    if (selectedObject.type === "text") {
      const nextDoc = { ...current, objects: current.objects.map((object) => object.type === "text" && object.id === selectedObject.id ? { ...object, size: next } : object) };
      preview(nextDoc, liveObfuscate(nextDoc));
    }
    if (selectedObject.type === "counter") {
      const nextDoc = { ...current, objects: current.objects.map((object) => object.type === "counter" && object.id === selectedObject.id ? { ...object, size: next } : object) };
      preview(nextDoc, liveObfuscate(nextDoc));
    }
  }

  function paintWeight(next: number) {
    setStrokeWidth(next);
    const current = docRef.current;
    if (!current || !selected.length) return;
    const nextDoc = {
      ...current,
      objects: current.objects.map((object) => selected.includes(idOf(object)) && "strokeWidth" in object ? { ...object, strokeWidth: next } : object),
    };
    preview(nextDoc, liveObfuscate(nextDoc));
  }

  function setObfuscateMode(mode: "pixelate" | "blur" | "solid") {
    setCoverMode(mode);
    if (!obfuscate || !doc) return;
    commit({ ...doc, objects: doc.objects.map((object) => object.type === "obfuscate" && object.id === obfuscate.id ? { ...object, mode } : object) });
  }

  function setObfuscateColor(color: string) {
    setCoverColor(color);
    if (!obfuscate || !doc) return;
    commit(recolor(doc, [obfuscate.id], color));
  }

  function paintShadow(patch: { shadow?: boolean; shadowBlur?: number; shadowX?: number; shadowY?: number; shadowOpacity?: number }, live = false) {
    const current = docRef.current;
    if (!current || !selected.length) return;
    const next = {
      ...current,
      objects: current.objects.map((object) => selected.includes(idOf(object)) && "shadow" in object ? { ...object, ...patch } : object),
    };
    if (live) preview(next, liveObfuscate(next));
    else commit(next);
  }

  function setObfuscateBlock(next: number) {
    setBlockSize(next);
    const current = docRef.current;
    if (!obfuscate || !current) return;
    preview({ ...current, objects: current.objects.map((object) => object.type === "obfuscate" && object.id === obfuscate.id ? { ...object, blockSize: next } : object) }, obfuscate.mode !== "solid");
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
          <span className="ed-tip" data-tip="Undo">
            <button type="button" className="ed-icon" aria-label="Undo" disabled={!state?.canUndo} onClick={() => api.undo().then(applyRemote)}><Icon name="undo" /></button>
          </span>
          <span className="ed-tip" data-tip="Redo">
            <button type="button" className="ed-icon" aria-label="Redo" disabled={!state?.canRedo} onClick={() => api.redo().then(applyRemote)}><Icon name="redo" /></button>
          </span>
          <button type="button" className="ed-icon" data-tip="Copy" aria-label="Copy to clipboard" onClick={() => api.copyEditor().catch((err: Error) => setError(err.message))}><Icon name="copy" /></button>
          <span className="ed-tip" data-tip="Share is not in this version yet">
            <button type="button" className="ed-icon" aria-label="Share is not in this version yet" disabled><Icon name="share" /></button>
          </span>
          <div className="ed-save-wrap" ref={saveWrap}>
            <div className="ed-save">
              <button type="button" onClick={() => savePng()}>Save PNG</button>
              <button type="button" data-tip="More save actions" aria-label="More save actions" aria-expanded={saveMenu} onClick={() => setSaveMenu((open) => !open)}><Icon name="chevron" /></button>
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
          <canvas ref={canvasRef} style={{ cursor: tool === "select" ? "default" : "crosshair" }} onPointerDown={onDown} onPointerMove={onMove} onPointerUp={onUp} onPointerCancel={onUp} onDoubleClick={(event) => {
            if (!doc) return;
            const bounds = canvasRef.current!.getBoundingClientRect();
            const p = { x: (event.clientX - bounds.left - pan.x) / zoom, y: (event.clientY - bounds.top - pan.y) / zoom };
            const hit = hitTest(doc, p.x, p.y);
            const object = doc.objects.find((item) => item.id === hit);
            if (object?.type !== "text") return;
            setSelected([object.id]);
            setEditing(object.id);
            setTool("select");
          }} />
          {editingObject && editingBox && (
            <textarea
              ref={editRef}
              className="ed-inline"
              value={editingObject.text}
              style={{
                left: pan.x + editingBox.x * zoom,
                top: pan.y + editingBox.y * zoom,
                width: Math.max(96, editingBox.w * zoom),
                height: Math.max(editingObject.size * zoom * 1.3, editingBox.h * zoom),
                fontSize: editingObject.size * zoom,
                fontWeight: editingObject.bold ? 700 : 400,
                color: editingObject.color,
                background: editingObject.fill === "none" ? "transparent" : editingObject.fill,
              }}
              onChange={(event) => doc && remember(updateText(doc, editingObject.id, event.target.value))}
              onBlur={() => {
                const current = docRef.current;
                if (current) commit(current);
                setEditing(null);
              }}
              onKeyDown={(event) => event.stopPropagation()}
            />
          )}
          <div className="ed-zoom">
            <button type="button" data-tip="Zoom out" aria-label="Zoom out" onClick={() => setZoom((z) => Math.max(0.1, z / 1.15))}>−</button>
            <span>{Math.round(zoom * 100)}%</span>
            <button type="button" data-tip="Zoom in" aria-label="Zoom in" onClick={() => setZoom((z) => Math.min(8, z * 1.15))}>+</button>
            <span className="ed-gap" />
            <button type="button" data-tip="Fit" onClick={() => setZoom(fit())}>Fit ⌘0</button>
            <button type="button" data-tip="Actual size" onClick={() => { setZoom(1); setPan({ x: 40, y: 40 }); }}>100% ⌘1</button>
          </div>
        </div>
        <aside className="ed-inspector">
          {!showShape && !showObfuscate && !showEffects && <p className="ed-block">Select a shape to edit it, or choose a tool.</p>}
          {showShape && <div className="ed-block">
            <span className="ed-kicker">{section}</span>
            {showColor && <ColourChoices label="Colour" value={shownColor} onPick={(color) => { setStroke(color); if (selectedObject && doc) commit(recolor(doc, selected, color)); }} />}
            {showText && (
              <div className="ed-line"><label htmlFor="ed-font">Font</label><select id="ed-font" value={selectedObject?.type === "text" && selectedObject.bold ? "bold" : "regular"} onChange={(event) => {
                if (selectedObject?.type === "text" && doc) commit({ ...doc, objects: doc.objects.map((object) => object.type === "text" && object.id === selectedObject.id ? { ...object, bold: event.target.value === "bold" } : object) });
              }}><option value="bold">System · Bold</option><option value="regular">System</option></select></div>
            )}
            {(showText || tool === "counter" || selectedObject?.type === "counter") && (
              <div className="ed-line"><label htmlFor="ed-size">Size</label><input id="ed-size" type="range" min={10} max={72} value={shownSize} onChange={(event) => paintSize(Number(event.target.value))} onPointerUp={endGesture} onKeyUp={endGesture} onBlur={endGesture} /><span>{Math.round(shownSize)}</span></div>
            )}
            {showWeight && (
              <div className="ed-line"><label htmlFor="ed-weight">Weight</label><input id="ed-weight" type="range" min={1} max={24} value={shownWeight} onChange={(event) => paintWeight(Number(event.target.value))} onPointerUp={endGesture} onKeyUp={endGesture} onBlur={endGesture} /><span>{Math.round(shownWeight)}</span></div>
            )}
            {showFill && <ColourChoices label="Fill" value={shownFill} allowNone onPick={paintFill} />}
            {selectedObject && "shadow" in selectedObject && (
              <label className="ed-check">Shadow <input type="checkbox" checked={selectedObject.shadow} onChange={(event) => paintShadow({ shadow: event.target.checked })} /></label>
            )}
            {selectedObject && "shadow" in selectedObject && selectedObject.shadow && (
              <>
                <div className="ed-line"><label htmlFor="ed-shadow-blur">Blur</label><input id="ed-shadow-blur" type="range" min={0} max={40} value={shadowBlur} onChange={(event) => { const next = Number(event.target.value); setShadowBlur(next); paintShadow({ shadowBlur: next }, true); }} onPointerUp={endGesture} onKeyUp={endGesture} onBlur={endGesture} /><span>{Math.round(shadowBlur)}</span></div>
                <div className="ed-line"><label htmlFor="ed-shadow-x">X</label><input id="ed-shadow-x" type="range" min={-40} max={40} value={shadowX} onChange={(event) => { const next = Number(event.target.value); setShadowX(next); paintShadow({ shadowX: next }, true); }} onPointerUp={endGesture} onKeyUp={endGesture} onBlur={endGesture} /><span>{Math.round(shadowX)}</span></div>
                <div className="ed-line"><label htmlFor="ed-shadow-y">Y</label><input id="ed-shadow-y" type="range" min={-40} max={40} value={shadowY} onChange={(event) => { const next = Number(event.target.value); setShadowY(next); paintShadow({ shadowY: next }, true); }} onPointerUp={endGesture} onKeyUp={endGesture} onBlur={endGesture} /><span>{Math.round(shadowY)}</span></div>
                <div className="ed-line"><label htmlFor="ed-shadow-opacity">Opacity</label><input id="ed-shadow-opacity" type="range" min={0} max={100} value={shadowOpacity} onChange={(event) => { const next = Number(event.target.value); setShadowOpacity(next); paintShadow({ shadowOpacity: next / 100 }, true); }} onPointerUp={endGesture} onKeyUp={endGesture} onBlur={endGesture} /><span>{Math.round(shadowOpacity)}</span></div>
              </>
            )}
            {selectedObject?.type === "text" && (
              <textarea className="ed-text" value={selectedObject.text} onChange={(event) => doc && remember(updateText(doc, selectedObject.id, event.target.value))} onBlur={() => { const current = docRef.current; if (current) commit(current); }} />
            )}
          </div>}
          {showObfuscate && <div className="ed-rule" />}
          {showObfuscate && <div className="ed-block">
            <span className="ed-kicker">Obfuscate</span>
            <div className="ed-segment">
              {(["pixelate", "blur", "solid"] as const).map((mode) => (
                <button key={mode} type="button" className={obfuscateMode === mode ? "on" : ""} onClick={() => setObfuscateMode(mode)}>{mode === "pixelate" ? "Pixelate" : mode === "blur" ? "Blur" : "Solid"}</button>
              ))}
            </div>
            {obfuscateMode === "solid" && <ColourChoices label="Colour" value={obfuscate?.color ?? coverColor} onPick={setObfuscateColor} />}
            {obfuscateMode !== "solid" && <div className="ed-line"><label htmlFor="ed-px">Block size</label><input id="ed-px" type="range" min={4} max={40} value={blockSize} onChange={(event) => setObfuscateBlock(Number(event.target.value))} onPointerUp={endGesture} onKeyUp={endGesture} onBlur={endGesture} /><span>{blockSize}</span></div>}
            <p>Flattened on export. Original pixels are never written to the file.</p>
          </div>}
          {showEffects && <div className="ed-rule" />}
          {showEffects && <div className="ed-block">
            <span className="ed-kicker">On export</span>
            {([["shadow", "Drop shadow"], ["border", "Border"], ["tornEdges", "Torn edges"], ["rounded", "Rounded corners"], ["grayscale", "Grayscale"], ["invert", "Invert"]] as Array<[Effect, string]>).map(([effect, label]) => (
              <label key={effect} className="ed-check">{label} <input type="checkbox" checked={preset?.effects.includes(effect) ?? false} onChange={() => toggleEffect(effect)} /></label>
            ))}
          </div>}
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

function startObject(tool: Tool, p: { x: number; y: number }, stroke: string, strokeWidth: number, fill: string, blockSize: number, coverMode: "pixelate" | "blur" | "solid", coverColor: string): AnnotationObject | null {
  const id = uid();
  if (tool === "crop") return null;
  if (tool === "rect") return { type: "rect", id, x: p.x, y: p.y, w: 1, h: 1, stroke, strokeWidth, fill: "none", radius: 0, opacity: 1, shadow: false, ...SHADOW };
  if (tool === "ellipse") return { type: "ellipse", id, x: p.x, y: p.y, w: 1, h: 1, stroke, strokeWidth, fill, opacity: 1, shadow: false, ...SHADOW };
  if (tool === "line") return { type: "line", id, x1: p.x, y1: p.y, x2: p.x, y2: p.y, stroke, strokeWidth, shadow: false, ...SHADOW };
  if (tool === "arrow") return { type: "arrow", id, x1: p.x, y1: p.y, x2: p.x, y2: p.y, cx: null, cy: null, heads: "end", stroke, strokeWidth, shadow: true, ...SHADOW };
  if (tool === "freehand") return { type: "freehand", id, points: [p], stroke, strokeWidth };
  if (tool === "highlight") return { type: "highlight", id, x: p.x, y: p.y, w: 1, h: 1, mode: "marker", color: stroke, opacity: 0.35 };
  if (tool === "obfuscate") return { type: "obfuscate", id, x: p.x, y: p.y, w: 1, h: 1, mode: coverMode, blockSize, color: coverColor };
  return null;
}

function colorOf(object: AnnotationObject | null): string | null {
  if (!object) return null;
  if (object.type === "highlight" || object.type === "text" || object.type === "obfuscate") return object.color;
  if (object.type === "counter") return object.fill;
  if ("stroke" in object) return object.stroke;
  return null;
}

function applyHandle(object: AnnotationObject, handle: Handle, p: { x: number; y: number }): AnnotationObject {
  if (object.type === "line" || object.type === "arrow") {
    if (handle === "start") return { ...object, x1: p.x, y1: p.y };
    if (handle === "end") return { ...object, x2: p.x, y2: p.y };
    return object;
  }
  if (object.type === "freehand") return scalePath(object, handle, p);
  if (object.type === "text") return resizeText(object, handle, p);
  if (object.type === "counter") {
    const next = resizeBox(object.x, object.y, object.size, object.size, handle, p);
    return { ...object, x: next.x, y: next.y, size: Math.max(12, Math.max(next.w, next.h)) };
  }
  if ("x" in object && "w" in object && "h" in object) {
    const next = resizeBox(object.x, object.y, object.w, object.h, handle, p);
    return { ...object, x: next.x, y: next.y, w: next.w, h: next.h };
  }
  return object;
}

function resizeBox(x: number, y: number, w: number, h: number, handle: Handle, p: { x: number; y: number }) {
  let left = x;
  let top = y;
  let right = x + w;
  let bottom = y + h;
  if (handle === "nw" || handle === "w" || handle === "sw") left = p.x;
  if (handle === "ne" || handle === "e" || handle === "se") right = p.x;
  if (handle === "nw" || handle === "n" || handle === "ne") top = p.y;
  if (handle === "sw" || handle === "s" || handle === "se") bottom = p.y;
  return { x: Math.min(left, right), y: Math.min(top, bottom), w: Math.max(1, Math.abs(right - left)), h: Math.max(1, Math.abs(bottom - top)) };
}

function scalePath(object: Extract<AnnotationObject, { type: "freehand" }>, handle: Handle, p: { x: number; y: number }): AnnotationObject {
  const box = bounds(object);
  if (!box || box.w < 1 || box.h < 1) return object;
  const next = resizeBox(box.x, box.y, box.w, box.h, handle, p);
  const sx = next.w / box.w;
  const sy = next.h / box.h;
  return {
    ...object,
    points: object.points.map((point) => ({
      x: next.x + (point.x - box.x) * sx,
      y: next.y + (point.y - box.y) * sy,
    })),
  };
}

function resizeText(object: Extract<AnnotationObject, { type: "text" }>, handle: Handle, p: { x: number; y: number }): AnnotationObject {
  const top = object.y - object.size;
  const south = handle === "s" || handle === "se" || handle === "sw";
  const north = handle === "n" || handle === "ne" || handle === "nw";
  const west = handle === "w" || handle === "nw" || handle === "sw";
  if (south) {
    const size = Math.max(10, Math.min(160, p.y - top));
    return { ...object, size, y: top + size, x: west ? p.x : object.x };
  }
  if (north) {
    const size = Math.max(10, Math.min(160, object.y - p.y));
    return { ...object, size, x: west ? p.x : object.x };
  }
  if (west) return { ...object, x: p.x };
  return object;
}

function clampBox(doc: Document, x: number, y: number, w: number, h: number) {
  const x0 = Math.max(0, Math.min(doc.width, x));
  const y0 = Math.max(0, Math.min(doc.height, y));
  const x1 = Math.max(0, Math.min(doc.width, x + w));
  const y1 = Math.max(0, Math.min(doc.height, y + h));
  return { x: Math.min(x0, x1), y: Math.min(y0, y1), w: Math.abs(x1 - x0), h: Math.abs(y1 - y0) };
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
      if (object.type === "text" || object.type === "obfuscate") return { ...object, color };
      if (object.type === "counter") return { ...object, fill: color };
      if ("stroke" in object) return { ...object, stroke: color };
      return object;
    }),
  };
}

function updateText(doc: Document, id: string, text: string): Document {
  return { ...doc, objects: doc.objects.map((object) => object.type === "text" && object.id === id ? { ...object, text } : object) };
}

function ColourChoices({ label, value, onPick, allowNone = false }: { label: string; value: string; onPick: (color: string) => void; allowNone?: boolean }) {
  const known = SWATCHES.some((color) => color.toLowerCase() === value.toLowerCase());
  const custom = value !== "none" && !known;
  return (
    <>
      <span>{label}</span>
      <div className="ed-swatches">
        {allowNone && <button type="button" className={value === "none" ? "ed-none on" : "ed-none"} data-tip="No fill" onClick={() => onPick("none")}>None</button>}
        {SWATCHES.map((color) => (
          <button key={color} type="button" data-tip={COLOR_NAME[color] ?? color} aria-label={COLOR_NAME[color] ?? color} className={value.toLowerCase() === color.toLowerCase() ? "on" : ""} style={{ background: color, color }} onClick={() => onPick(color)} />
        ))}
        <label className={custom ? "ed-custom on" : "ed-custom"} data-tip="Custom colour" style={{ background: custom ? value : "#ffffff", color: custom ? value : "#6e6e73" }}>
          <span style={{ color: custom ? "#ffffff" : "#6e6e73" }}>+</span>
          <input aria-label={`Custom ${label.toLowerCase()}`} type="color" value={toHex(value)} onChange={(event) => onPick(event.target.value.toUpperCase())} />
        </label>
      </div>
    </>
  );
}

function toHex(color: string) {
  if (/^#[0-9a-fA-F]{6}$/.test(color)) return color;
  return "#ffffff";
}

function ToolButton({ id, tool, setTool }: { id: Tool; tool: Tool; setTool: (tool: Tool) => void }) {
  const later = LATER.has(id);
  const key = Object.entries(KEYS).find(([, value]) => value === id)?.[0].toUpperCase();
  const tip = later ? `${TOOL_LABEL[id]} is not in this version yet` : `${TOOL_LABEL[id]} (${key})`;
  return (
    <span className="ed-tip" data-tip={tip}>
      <button type="button" className={tool === id ? "on" : ""} disabled={later} aria-label={tip} onClick={() => setTool(id)}>
        <Icon name={id} />
      </button>
    </span>
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
      {name === "redo" && <path {...common} d="M15 14l5-5-5-5M20 9H9a5 5 0 000 10h4" />}
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
