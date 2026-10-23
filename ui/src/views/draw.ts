import type { AnnotationObject, Document } from "../api";

export function drawDocument(
  ctx: CanvasRenderingContext2D,
  image: HTMLImageElement | null,
  doc: Document,
  zoom: number,
  panX: number,
  panY: number,
  selected: string[],
  editing: string | null = null,
) {
  const canvas = ctx.canvas;
  ctx.clearRect(0, 0, canvas.width, canvas.height);
  ctx.save();
  ctx.translate(panX, panY);
  ctx.scale(zoom, zoom);
  const scene = sceneCanvas(doc.width, doc.height);
  const sceneCtx = scene.getContext("2d");
  if (sceneCtx) {
    sceneCtx.clearRect(0, 0, scene.width, scene.height);
    if (image && image.complete && image.naturalWidth > 0) sceneCtx.drawImage(image, 0, 0, doc.width, doc.height);
    else {
      sceneCtx.fillStyle = "#d0d0d4";
      sceneCtx.fillRect(0, 0, doc.width, doc.height);
    }
    for (const object of doc.objects) {
      if (object.type === "obfuscate" && object.mode !== "solid") obfuscateRegion(sceneCtx, object);
      else drawObject(sceneCtx, object, editing);
    }
    ctx.drawImage(scene, 0, 0);
  }
  ctx.restore();
  for (const object of doc.objects) {
    if (!selected.includes(idOf(object))) continue;
    const box = bounds(object);
    ctx.save();
    ctx.strokeStyle = "#0A84FF";
    ctx.lineWidth = 1;
    ctx.setLineDash([4, 3]);
    if (box) ctx.strokeRect(panX + box.x * zoom, panY + box.y * zoom, box.w * zoom, box.h * zoom);
    ctx.setLineDash([]);
    ctx.fillStyle = "#ffffff";
    for (const handle of handles(object)) {
      const sx = panX + handle.x * zoom;
      const sy = panY + handle.y * zoom;
      ctx.fillRect(sx - 4, sy - 4, 8, 8);
      ctx.strokeRect(sx - 4, sy - 4, 8, 8);
    }
    ctx.restore();
  }
}

let scene: HTMLCanvasElement | null = null;

function sceneCanvas(width: number, height: number) {
  const w = Math.max(1, Math.round(width));
  const h = Math.max(1, Math.round(height));
  if (!scene) scene = document.createElement("canvas");
  if (scene.width !== w || scene.height !== h) {
    scene.width = w;
    scene.height = h;
  }
  return scene;
}

function obfuscateRegion(ctx: CanvasRenderingContext2D, object: Extract<AnnotationObject, { type: "obfuscate" }>) {
  const source = ctx.canvas;
  const x = Math.max(0, Math.floor(Math.min(object.x, object.x + object.w)));
  const y = Math.max(0, Math.floor(Math.min(object.y, object.y + object.h)));
  const x2 = Math.min(source.width, Math.ceil(Math.max(object.x, object.x + object.w)));
  const y2 = Math.min(source.height, Math.ceil(Math.max(object.y, object.y + object.h)));
  const w = x2 - x;
  const h = y2 - y;
  if (w < 1 || h < 1) return;
  if (object.mode === "pixelate") {
    const block = Math.max(2, object.blockSize);
    const sw = Math.max(1, Math.round(w / block));
    const sh = Math.max(1, Math.round(h / block));
    const tmp = document.createElement("canvas");
    tmp.width = sw;
    tmp.height = sh;
    const sample = tmp.getContext("2d");
    if (!sample) return;
    sample.imageSmoothingEnabled = false;
    sample.drawImage(source, x, y, w, h, 0, 0, sw, sh);
    ctx.imageSmoothingEnabled = false;
    ctx.drawImage(tmp, 0, 0, sw, sh, x, y, w, h);
    ctx.imageSmoothingEnabled = true;
    return;
  }
  blurRegion(ctx, x, y, w, h, Math.max(2, object.blockSize));
}

function blurRegion(ctx: CanvasRenderingContext2D, x: number, y: number, w: number, h: number, radius: number) {
  const canvas = ctx.canvas;
  const pad = radius;
  const sx = Math.max(0, x - pad);
  const sy = Math.max(0, y - pad);
  const ex = Math.min(canvas.width, x + w + pad);
  const ey = Math.min(canvas.height, y + h + pad);
  const sw = ex - sx;
  const sh = ey - sy;
  if (sw < 1 || sh < 1) return;
  const scale = Math.max(1, Math.ceil(Math.max(sw, sh) / 180));
  const dw = Math.max(1, Math.round(sw / scale));
  const dh = Math.max(1, Math.round(sh / scale));
  const tmp = document.createElement("canvas");
  tmp.width = dw;
  tmp.height = dh;
  const sample = tmp.getContext("2d", { willReadFrequently: true });
  if (!sample) return;
  sample.imageSmoothingEnabled = true;
  sample.drawImage(canvas, sx, sy, sw, sh, 0, 0, dw, dh);
  const pixels = sample.getImageData(0, 0, dw, dh);
  boxBlur(pixels.data, dw, dh, Math.max(1, Math.round(radius / scale)));
  sample.putImageData(pixels, 0, 0);
  ctx.imageSmoothingEnabled = true;
  ctx.drawImage(tmp, (x - sx) / scale, (y - sy) / scale, Math.max(1, w / scale), Math.max(1, h / scale), x, y, w, h);
}

function boxBlur(data: Uint8ClampedArray, width: number, height: number, radius: number) {
  const src = new Uint8ClampedArray(data);
  const dst = new Uint8ClampedArray(data.length);
  for (let pass = 0; pass < 3; pass++) {
    blurHorizontal(src, dst, width, height, radius);
    blurVertical(dst, src, width, height, radius);
  }
  data.set(src);
}

function blurHorizontal(src: Uint8ClampedArray, dst: Uint8ClampedArray, width: number, height: number, radius: number) {
  const span = radius * 2 + 1;
  for (let y = 0; y < height; y++) {
    const row = y * width;
    let r = 0;
    let g = 0;
    let b = 0;
    let a = 0;
    for (let i = -radius; i <= radius; i++) {
      const p = (row + Math.min(width - 1, Math.max(0, i))) * 4;
      r += src[p];
      g += src[p + 1];
      b += src[p + 2];
      a += src[p + 3];
    }
    for (let x = 0; x < width; x++) {
      const p = (row + x) * 4;
      dst[p] = r / span;
      dst[p + 1] = g / span;
      dst[p + 2] = b / span;
      dst[p + 3] = a / span;
      const remove = (row + Math.min(width - 1, Math.max(0, x - radius))) * 4;
      const add = (row + Math.min(width - 1, Math.max(0, x + radius + 1))) * 4;
      r += src[add] - src[remove];
      g += src[add + 1] - src[remove + 1];
      b += src[add + 2] - src[remove + 2];
      a += src[add + 3] - src[remove + 3];
    }
  }
}

function blurVertical(src: Uint8ClampedArray, dst: Uint8ClampedArray, width: number, height: number, radius: number) {
  const span = radius * 2 + 1;
  for (let x = 0; x < width; x++) {
    let r = 0;
    let g = 0;
    let b = 0;
    let a = 0;
    for (let i = -radius; i <= radius; i++) {
      const p = (Math.min(height - 1, Math.max(0, i)) * width + x) * 4;
      r += src[p];
      g += src[p + 1];
      b += src[p + 2];
      a += src[p + 3];
    }
    for (let y = 0; y < height; y++) {
      const p = (y * width + x) * 4;
      dst[p] = r / span;
      dst[p + 1] = g / span;
      dst[p + 2] = b / span;
      dst[p + 3] = a / span;
      const remove = (Math.min(height - 1, Math.max(0, y - radius)) * width + x) * 4;
      const add = (Math.min(height - 1, Math.max(0, y + radius + 1)) * width + x) * 4;
      r += src[add] - src[remove];
      g += src[add + 1] - src[remove + 1];
      b += src[add + 2] - src[remove + 2];
      a += src[add + 3] - src[remove + 3];
    }
  }
}

export function drawCrop(
  ctx: CanvasRenderingContext2D,
  doc: Document,
  box: { x: number; y: number; w: number; h: number },
  zoom: number,
  panX: number,
  panY: number,
) {
  ctx.save();
  ctx.translate(panX, panY);
  ctx.scale(zoom, zoom);
  ctx.fillStyle = "rgba(0, 0, 0, 0.45)";
  ctx.fillRect(0, 0, doc.width, box.y);
  ctx.fillRect(0, box.y, box.x, box.h);
  ctx.fillRect(box.x + box.w, box.y, Math.max(0, doc.width - box.x - box.w), box.h);
  ctx.fillRect(0, box.y + box.h, doc.width, Math.max(0, doc.height - box.y - box.h));
  ctx.strokeStyle = "#ffffff";
  ctx.lineWidth = 1 / zoom;
  ctx.setLineDash([4 / zoom, 3 / zoom]);
  ctx.strokeRect(box.x + 0.5 / zoom, box.y + 0.5 / zoom, box.w, box.h);
  ctx.restore();
}

export type Handle = "nw" | "n" | "ne" | "e" | "se" | "s" | "sw" | "w" | "start" | "end";

export function handles(object: AnnotationObject): { id: Handle; x: number; y: number }[] {
  if (object.type === "line" || object.type === "arrow") {
    return [
      { id: "start", x: object.x1, y: object.y1 },
      { id: "end", x: object.x2, y: object.y2 },
    ];
  }
  const box = bounds(object);
  if (!box) return [];
  const { x, y, w, h } = box;
  return [
    { id: "nw", x, y },
    { id: "n", x: x + w / 2, y },
    { id: "ne", x: x + w, y },
    { id: "e", x: x + w, y: y + h / 2 },
    { id: "se", x: x + w, y: y + h },
    { id: "s", x: x + w / 2, y: y + h },
    { id: "sw", x, y: y + h },
    { id: "w", x, y: y + h / 2 },
  ];
}

export function hitHandle(object: AnnotationObject, x: number, y: number, zoom: number): Handle | null {
  const radius = 8 / Math.max(zoom, 0.2);
  for (const handle of [...handles(object)].reverse()) {
    if (Math.abs(x - handle.x) <= radius && Math.abs(y - handle.y) <= radius) return handle.id;
  }
  return null;
}

export function idOf(object: AnnotationObject): string {
  return object.id;
}

export function bounds(object: AnnotationObject): { x: number; y: number; w: number; h: number } | null {
  switch (object.type) {
    case "rect":
    case "ellipse":
    case "highlight":
    case "obfuscate":
    case "image":
    case "cursor":
      return { x: object.x, y: object.y, w: object.w, h: object.h };
    case "line":
    case "arrow":
      return {
        x: Math.min(object.x1, object.x2),
        y: Math.min(object.y1, object.y2),
        w: Math.abs(object.x2 - object.x1) || 8,
        h: Math.abs(object.y2 - object.y1) || 8,
      };
    case "text": {
      const lines = object.text.split("\n");
      const longest = lines.reduce((max, line) => Math.max(max, line.length), 1);
      return { x: object.x - 4, y: object.y - object.size, w: Math.max(48, longest * object.size * 0.55 + 8), h: lines.length * object.size * 1.2 + 8 };
    }
    case "counter":
      return { x: object.x, y: object.y, w: object.size, h: object.size };
    case "freehand": {
      if (!object.points.length) return null;
      const xs = object.points.map((p) => p.x);
      const ys = object.points.map((p) => p.y);
      const x = Math.min(...xs);
      const y = Math.min(...ys);
      return { x, y, w: Math.max(...xs) - x || 4, h: Math.max(...ys) - y || 4 };
    }
    default:
      return null;
  }
}

function drawObject(ctx: CanvasRenderingContext2D, object: AnnotationObject, editing: string | null) {
  ctx.save();
  switch (object.type) {
    case "rect":
      ctx.globalAlpha = object.opacity;
      if (object.shadow) shadow(ctx, object);
      roundRect(ctx, object.x, object.y, object.w, object.h, object.radius);
      paint(ctx, object.fill, object.stroke, object.strokeWidth);
      break;
    case "ellipse":
      ctx.globalAlpha = object.opacity;
      if (object.shadow) shadow(ctx, object);
      ctx.beginPath();
      ctx.ellipse(object.x + object.w / 2, object.y + object.h / 2, Math.abs(object.w) / 2, Math.abs(object.h) / 2, 0, 0, Math.PI * 2);
      paint(ctx, object.fill, object.stroke, object.strokeWidth);
      break;
    case "line":
      if (object.shadow) shadow(ctx, object);
      line(ctx, object.x1, object.y1, object.x2, object.y2, object.stroke, object.strokeWidth, true);
      break;
    case "arrow": {
      if (object.shadow) shadow(ctx, object);
      const dx = object.x2 - object.x1;
      const dy = object.y2 - object.y1;
      const len = Math.hypot(dx, dy) || 1;
      const inset = Math.min(len * 0.45, Math.max(10, object.strokeWidth * 3.2) * 0.82);
      const end = object.heads === "end" || object.heads === "both";
      const both = object.heads === "both";
      const x2 = end ? object.x2 - (dx / len) * inset : object.x2;
      const y2 = end ? object.y2 - (dy / len) * inset : object.y2;
      const x1 = both ? object.x1 + (dx / len) * inset : object.x1;
      const y1 = both ? object.y1 + (dy / len) * inset : object.y1;
      line(ctx, x1, y1, x2, y2, object.stroke, object.strokeWidth, false);
      head(ctx, object.x2, object.y2, dx, dy, object.stroke, object.strokeWidth);
      if (both) head(ctx, object.x1, object.y1, -dx, -dy, object.stroke, object.strokeWidth);
      break;
    }
    case "freehand":
      if (object.points.length > 1) {
        ctx.beginPath();
        ctx.moveTo(object.points[0].x, object.points[0].y);
        object.points.slice(1).forEach((p) => ctx.lineTo(p.x, p.y));
        ctx.strokeStyle = object.stroke;
        ctx.lineWidth = object.strokeWidth;
        ctx.lineCap = "round";
        ctx.stroke();
      }
      break;
    case "text": {
      if (object.id === editing) break;
      const lines = object.text.split("\n");
      const longest = lines.reduce((max, line) => Math.max(max, line.length), 1);
      const w = Math.max(object.size, longest * object.size * 0.55);
      const h = lines.length * object.size * 1.2;
      if (object.fill !== "none") {
        ctx.fillStyle = object.fill === "white" ? "#fff" : object.fill;
        ctx.fillRect(object.x - 4, object.y - object.size, w + 8, h + 8);
      }
      ctx.fillStyle = object.color;
      ctx.font = `${object.bold ? "700" : "400"} ${object.size}px ${getComputedStyle(document.body).fontFamily}`;
      lines.forEach((line, index) => ctx.fillText(line, object.x, object.y + index * object.size * 1.2));
      break;
    }
    case "counter":
      ctx.fillStyle = object.fill;
      ctx.beginPath();
      if (object.style === "circle") ctx.arc(object.x + object.size / 2, object.y + object.size / 2, object.size / 2, 0, Math.PI * 2);
      else ctx.rect(object.x, object.y, object.size, object.size);
      ctx.fill();
      ctx.fillStyle = "#fff";
      ctx.font = `700 ${object.size * 0.5}px ${getComputedStyle(document.body).fontFamily}`;
      ctx.textAlign = "center";
      ctx.textBaseline = "middle";
      ctx.fillText(String(object.number), object.x + object.size / 2, object.y + object.size / 2);
      break;
    case "highlight":
      if (object.mode === "marker") {
        ctx.globalAlpha = object.opacity;
        ctx.fillStyle = object.color;
        ctx.fillRect(object.x, object.y, object.w, object.h);
      } else {
        ctx.strokeStyle = object.color;
        ctx.strokeRect(object.x, object.y, object.w, object.h);
      }
      break;
    case "obfuscate":
      ctx.fillStyle = object.mode === "solid" ? object.color : "rgba(80,80,80,.45)";
      ctx.fillRect(object.x, object.y, object.w, object.h);
      break;
    case "image":
    case "cursor":
      drawBitmap(ctx, object.pngBase64, object.x, object.y, object.w, object.h);
      break;
  }
  ctx.restore();
}

const bitmaps = new Map<string, HTMLImageElement>();
let whenReady: (() => void) | null = null;

export function whenBitmapsLoad(fn: () => void) {
  whenReady = fn;
}

function drawBitmap(ctx: CanvasRenderingContext2D, pngBase64: string, x: number, y: number, w: number, h: number) {
  let image = bitmaps.get(pngBase64);
  if (!image) {
    image = new Image();
    image.onload = () => whenReady?.();
    image.src = `data:image/png;base64,${pngBase64}`;
    bitmaps.set(pngBase64, image);
  }
  if (image.complete && image.naturalWidth > 0) ctx.drawImage(image, x, y, w, h);
}

function paint(ctx: CanvasRenderingContext2D, fill: string, stroke: string, width: number) {
  if (fill !== "none") {
    ctx.fillStyle = fill;
    ctx.fill();
  }
  ctx.strokeStyle = stroke;
  ctx.lineWidth = width;
  ctx.stroke();
}

function line(ctx: CanvasRenderingContext2D, x1: number, y1: number, x2: number, y2: number, stroke: string, width: number, round: boolean) {
  ctx.beginPath();
  ctx.moveTo(x1, y1);
  ctx.lineTo(x2, y2);
  ctx.strokeStyle = stroke;
  ctx.lineWidth = width;
  ctx.lineCap = round ? "round" : "butt";
  ctx.stroke();
}

function head(ctx: CanvasRenderingContext2D, x: number, y: number, dx: number, dy: number, stroke: string, width: number) {
  const len = Math.hypot(dx, dy) || 1;
  const ux = dx / len;
  const uy = dy / len;
  const size = Math.max(10, width * 3.2);
  ctx.beginPath();
  ctx.moveTo(x, y);
  ctx.lineTo(x - ux * size + -uy * size * 0.45, y - uy * size + ux * size * 0.45);
  ctx.lineTo(x - ux * size - -uy * size * 0.45, y - uy * size - ux * size * 0.45);
  ctx.closePath();
  ctx.fillStyle = stroke;
  ctx.fill();
}

function roundRect(ctx: CanvasRenderingContext2D, x: number, y: number, w: number, h: number, r: number) {
  const radius = Math.max(0, Math.min(r, Math.abs(w) / 2, Math.abs(h) / 2));
  ctx.beginPath();
  ctx.roundRect(x, y, w, h, radius);
}

function shadow(ctx: CanvasRenderingContext2D, object: { shadowBlur: number; shadowX: number; shadowY: number; shadowOpacity: number }) {
  ctx.shadowColor = `rgba(0,0,0,${object.shadowOpacity})`;
  ctx.shadowBlur = object.shadowBlur;
  ctx.shadowOffsetX = object.shadowX;
  ctx.shadowOffsetY = object.shadowY;
}

export function hitCenter(doc: Document, x: number, y: number, zoom: number): string | null {
  for (let i = doc.objects.length - 1; i >= 0; i -= 1) {
    const box = bounds(doc.objects[i]);
    if (!box || box.w < 1 || box.h < 1) continue;
    const cx = box.x + box.w / 2;
    const cy = box.y + box.h / 2;
    const reach = Math.min(Math.max(10 / Math.max(zoom, 0.2), Math.min(box.w, box.h) * 0.18), Math.min(box.w, box.h) * 0.32);
    if (Math.hypot(x - cx, y - cy) <= reach) return idOf(doc.objects[i]);
  }
  return null;
}

export function hitsBox(doc: Document, box: { x: number; y: number; w: number; h: number }): string[] {
  return doc.objects.filter((object) => {
    const boundsOf = bounds(object);
    if (!boundsOf) return false;
    return boundsOf.x < box.x + box.w && boundsOf.x + boundsOf.w > box.x && boundsOf.y < box.y + box.h && boundsOf.y + boundsOf.h > box.y;
  }).map(idOf);
}

export function hitTest(doc: Document, x: number, y: number): string | null {
  for (let i = doc.objects.length - 1; i >= 0; i -= 1) {
    const object = doc.objects[i];
    const box = bounds(object);
    if (!box) continue;
    if (x >= box.x && y >= box.y && x <= box.x + box.w && y <= box.y + box.h) return idOf(object);
  }
  return null;
}
