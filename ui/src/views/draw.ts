import type { AnnotationObject, Document } from "../api";

export function drawDocument(
  ctx: CanvasRenderingContext2D,
  image: HTMLImageElement | null,
  doc: Document,
  zoom: number,
  panX: number,
  panY: number,
  selected: string[],
) {
  const canvas = ctx.canvas;
  ctx.clearRect(0, 0, canvas.width, canvas.height);
  ctx.save();
  ctx.translate(panX, panY);
  ctx.scale(zoom, zoom);
  if (image && image.complete) ctx.drawImage(image, 0, 0, doc.width, doc.height);
  else {
    ctx.fillStyle = "#d0d0d4";
    ctx.fillRect(0, 0, doc.width, doc.height);
  }
  for (const object of doc.objects) drawObject(ctx, object);
  ctx.restore();
  for (const object of doc.objects) {
    if (!selected.includes(idOf(object))) continue;
    const box = bounds(object);
    if (!box) continue;
    ctx.save();
    ctx.strokeStyle = "#0A84FF";
    ctx.lineWidth = 1;
    ctx.setLineDash([4, 3]);
    ctx.strokeRect(panX + box.x * zoom, panY + box.y * zoom, box.w * zoom, box.h * zoom);
    ctx.restore();
  }
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
    case "text":
      return { x: object.x, y: object.y - object.size, w: Math.max(40, object.text.length * object.size * 0.55), h: object.size + 8 };
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

function drawObject(ctx: CanvasRenderingContext2D, object: AnnotationObject) {
  ctx.save();
  switch (object.type) {
    case "rect":
      ctx.globalAlpha = object.opacity;
      if (object.shadow) shadow(ctx);
      roundRect(ctx, object.x, object.y, object.w, object.h, object.radius);
      paint(ctx, object.fill, object.stroke, object.strokeWidth);
      break;
    case "ellipse":
      ctx.globalAlpha = object.opacity;
      ctx.beginPath();
      ctx.ellipse(object.x + object.w / 2, object.y + object.h / 2, Math.abs(object.w) / 2, Math.abs(object.h) / 2, 0, 0, Math.PI * 2);
      paint(ctx, object.fill, object.stroke, object.strokeWidth);
      break;
    case "line":
      line(ctx, object.x1, object.y1, object.x2, object.y2, object.stroke, object.strokeWidth);
      break;
    case "arrow":
      line(ctx, object.x1, object.y1, object.x2, object.y2, object.stroke, object.strokeWidth);
      head(ctx, object.x2, object.y2, object.x2 - object.x1, object.y2 - object.y1, object.stroke, object.strokeWidth);
      if (object.heads === "both") head(ctx, object.x1, object.y1, object.x1 - object.x2, object.y1 - object.y2, object.stroke, object.strokeWidth);
      break;
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
    case "text":
      if (object.fill !== "none") {
        ctx.fillStyle = object.fill === "white" ? "#fff" : object.fill;
        const w = Math.max(object.size, object.text.length * object.size * 0.55);
        ctx.fillRect(object.x - 4, object.y - object.size, w + 8, object.size + 8);
      }
      ctx.fillStyle = object.color;
      ctx.font = `${object.bold ? "700" : "400"} ${object.size}px ${getComputedStyle(document.body).fontFamily}`;
      ctx.fillText(object.text, object.x, object.y);
      break;
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

function line(ctx: CanvasRenderingContext2D, x1: number, y1: number, x2: number, y2: number, stroke: string, width: number) {
  ctx.beginPath();
  ctx.moveTo(x1, y1);
  ctx.lineTo(x2, y2);
  ctx.strokeStyle = stroke;
  ctx.lineWidth = width;
  ctx.lineCap = "round";
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

function shadow(ctx: CanvasRenderingContext2D) {
  ctx.shadowColor = "rgba(0,0,0,.35)";
  ctx.shadowBlur = 8;
  ctx.shadowOffsetY = 3;
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
