import { LogicalSize } from "@tauri-apps/api/dpi";
import { useEffect, useState } from "react";
import { api, currentWindow, onHistoryChanged, queryId, shotUrl, type HistoryItem } from "../api";

export function Pin() {
  const id = queryId() || window.location.search.split("id=")[1] || "";
  const [item, setItem] = useState<HistoryItem | null>(null);
  useEffect(() => {
    const load = () => api.history("").then((rows) => setItem(rows.find((row) => row.id === id) ?? null));
    load();
    let stop = () => {};
    onHistoryChanged(load).then((un) => { stop = un; });
    return () => stop();
  }, [id]);
  useEffect(() => {
    const onWheel = (event: WheelEvent) => {
      event.preventDefault();
      const win = currentWindow();
      const delta = event.deltaY < 0 ? 28 : -28;
      win.innerSize().then((size) => {
        win.setSize(new LogicalSize(Math.max(140, size.width + delta), Math.max(100, size.height + delta * 0.7)));
      });
    };
    window.addEventListener("wheel", onWheel, { passive: false });
    return () => window.removeEventListener("wheel", onWheel);
  }, []);
  if (!item) return null;
  return (
    <div className="pin" data-tauri-drag-region onDoubleClick={() => api.openEditor(item.id)}>
      <img key={item.previewRev ?? 0} src={shotUrl(item)} alt="" />
      <div className="bar">
        <button onClick={() => api.openEditor(item.id)}>Annotate</button>
        <button onClick={() => api.closePin(item.id)}>Close</button>
      </div>
    </div>
  );
}
