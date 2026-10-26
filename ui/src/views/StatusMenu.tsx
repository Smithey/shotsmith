import { useEffect, useRef, useState } from "react";
import { LogicalPosition, LogicalSize } from "@tauri-apps/api/dpi";
import { availableMonitors } from "@tauri-apps/api/window";
import { api, currentWindow, onHistoryChanged, onRefresh, shotUrl, type HistoryItem, type Settings } from "../api";
import { isMac, menuHint } from "../keys";

export function StatusMenu() {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [recent, setRecent] = useState<HistoryItem[]>([]);
  const [pinned, setPinned] = useState<HistoryItem[]>([]);
  const [hover, setHover] = useState("");
  const [pinsOpen, setPinsOpen] = useState(false);
  const [side, setSide] = useState<"left" | "right">("right");
  const [ready, setReady] = useState(false);
  const [wide, setWide] = useState(false);
  const [pop, setPop] = useState(false);
  const base = useRef<{ x: number; y: number } | null>(null);
  const work = useRef<{ x: number; y: number; w: number; h: number } | null>(null);
  const shown = useRef(false);
  const geom = useRef({ x: 0, y: 0, w: 0, h: 0 });
  const placeTicket = useRef(0);

  async function place(nextSide: "left" | "right", open: boolean) {
    const shell = document.querySelector(".status-shell") as HTMLElement | null;
    const origin = base.current;
    if (!shell || !origin) return;
    const mine = ++placeTicket.current;
    // Windows keeps the flyout gutter from the start. Measuring after the
    // gutter is added, while the window is still narrow, wraps the menu and
    // the whole panel jumps.
    const extra = !isMac() || open ? 172 : 0;
    const w = 396 + extra;
    let h: number;
    if (geom.current.w >= w) {
      shell.style.paddingLeft = nextSide === "left" ? `${28 + extra}px` : "28px";
      shell.style.paddingRight = nextSide === "right" ? `${28 + extra}px` : "28px";
      h = Math.round(shell.scrollHeight);
    } else {
      h = Math.round(shell.scrollHeight);
      shell.style.paddingLeft = nextSide === "left" ? `${28 + extra}px` : "28px";
      shell.style.paddingRight = nextSide === "right" ? `${28 + extra}px` : "28px";
    }
    const dropUp = new URLSearchParams(window.location.search).get("drop") === "up";
    // 560 is the window height chosen before the page can measure itself.
    const box = work.current;
    let x = Math.round(origin.x - (nextSide === "left" ? extra : 0));
    let y = Math.round(dropUp ? origin.y + 560 - h : origin.y);
    // The flyout gutter and a bottom taskbar can push the panel onto the next
    // screen. Keep the whole window inside the work area.
    if (box) {
      const limitH = Math.max(160, Math.floor(box.h));
      if (h > limitH) {
        h = limitH;
        shell.style.maxHeight = `${h}px`;
        shell.style.overflowY = "auto";
      } else {
        shell.style.maxHeight = "";
        shell.style.overflowY = "";
      }
      if (x + w > box.x + box.w) x = Math.round(box.x + box.w - w);
      if (x < box.x) x = Math.round(box.x);
      if (y + h > box.y + box.h) y = Math.round(box.y + box.h - h);
      if (y < box.y) y = Math.round(box.y);
    }
    const prev = geom.current;
    const moved = prev.x !== x || prev.y !== y;
    const resized = prev.w !== w || prev.h !== h;
    if (!moved && !resized && shown.current) return;
    geom.current = { x, y, w, h };
    const win = currentWindow();
    // Position first so a bottom-anchored panel does not grow through the taskbar
    // and then jump back.
    if (moved) await win.setPosition(new LogicalPosition(x, y));
    if (mine !== placeTicket.current) return;
    if (resized) await win.setSize(new LogicalSize(w, h));
    if (mine !== placeTicket.current || shown.current) return;
    shown.current = true;
    await win.show();
    await win.setFocus();
  }

  function load() {
    api.settings().then(setSettings).catch(() => undefined);
    api.history("").then((items) => {
      setRecent(items.slice(0, 3));
      setPinned(items.filter((item) => item.pinned).slice(0, 8));
    }).catch(() => undefined);
  }

  useEffect(() => {
    load();
    let stop = () => {};
    onRefresh(() => load()).then((un) => { stop = un; });
    let stopHistory = () => {};
    onHistoryChanged(() => load()).then((un) => { stopHistory = un; });
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") currentWindow().hide();
    };
    window.addEventListener("keydown", onKey);
    return () => {
      stop();
      stopHistory();
      window.removeEventListener("keydown", onKey);
    };
  }, []);

  useEffect(() => {
    const win = currentWindow();
    let cancel = false;
    Promise.all([win.scaleFactor(), win.outerPosition(), availableMonitors()]).then(([factor, pos, monitors]) => {
      if (cancel) return;
      const origin = pos.toLogical(factor);
      if (!base.current) base.current = { x: origin.x, y: origin.y };
      const monitor = monitors.find((item) => {
        const edge = item.position.toLogical(item.scaleFactor);
        const bounds = item.size.toLogical(item.scaleFactor);
        return origin.x >= edge.x && origin.y >= edge.y && origin.x < edge.x + bounds.width && origin.y < edge.y + bounds.height;
      }) ?? monitors[0];
      if (monitor) {
        const scale = monitor.scaleFactor;
        const area = monitor.workArea.position.toLogical(scale);
        const areaSize = monitor.workArea.size.toLogical(scale);
        work.current = { x: area.x, y: area.y, w: areaSize.width, h: areaSize.height };
        setSide(area.x + areaSize.width - (origin.x + 396) < 180 ? "left" : "right");
      }
      setReady(true);
    }).catch(() => undefined);
    return () => { cancel = true; };
  }, []);

  useEffect(() => {
    if (!ready) return;
    void place(side, wide);
  }, [ready, recent, pinned, pinsOpen, settings, side, wide]);

  async function run(action: () => Promise<unknown> | void) {
    await currentWindow().hide();
    // Hide returns before the panel is gone from the screen. Wait, or the
    // capture includes the menu.
    await new Promise((resolve) => window.setTimeout(resolve, 140));
    await action();
  }

  const seconds = settings?.timedSeconds ?? 5;

  function point(id: string) {
    setHover(id);
    if (id !== "timed") {
      setWide(false);
      setPop(false);
    }
  }

  return (
    <div className="status-shell">
      <div className="status-menu" onMouseLeave={() => point("")}>
        <div className="menu-label">Capture</div>
        <Row id="region" hover={hover} setHover={point} label="Region" hint={menuHint(settings?.shortcuts.region)} onClick={() => run(() => api.begin("region"))} />
        <Row id="repeat" hover={hover} setHover={point} label="Repeat last region" hint={menuHint(settings?.shortcuts.repeatLast)} onClick={() => run(() => api.repeat())} />
        <Row id="window" hover={hover} setHover={point} label="Window" hint={menuHint(settings?.shortcuts.window)} onClick={() => run(() => api.begin("window"))} />
        <Row id="screen" hover={hover} setHover={point} label="Full screen" hint={menuHint(settings?.shortcuts.fullScreen)} onClick={() => run(() => api.begin("screen"))} />
        <Row id="scroll" hover={hover} setHover={point} label="Scrolling…" hint="" onClick={() => run(() => api.begin("scroll"))} />
        <div className={side === "left" ? "menu-sub flip" : "menu-sub"} onMouseEnter={() => { setHover("timed"); setPinsOpen(false); setPop(true); if (isMac()) setWide(true); }}>
          <button type="button" className={hover === "timed" ? "menu-row on" : "menu-row"} onClick={() => run(() => api.countdown())}>
            <span>Timed · {seconds} s</span>
            <span className="menu-key">›</span>
          </button>
          {pop && (
            <div className="menu-pop">
              {[3, 5, 10].map((value) => (
                <button key={value} type="button" onClick={() => run(async () => {
                  const current = settings ?? await api.settings();
                  await api.saveSettings({ ...current, timedSeconds: value });
                  await api.countdown();
                })}>
                  <span className="menu-check">{value === seconds ? "✓" : ""}</span>
                  {value} seconds
                </button>
              ))}
            </div>
          )}
        </div>
        <Row id="text" hover={hover} setHover={point} label="Copy text from screen" hint="" onClick={() => run(() => api.begin("text"))} />
        <Row id="measure" hover={hover} setHover={point} label="Measure" hint="" onClick={() => run(() => api.begin("measure"))} />
        <div className="menu-rule" />
        <div className="recent-block" onMouseEnter={() => point("history")}>
          <div className="recent-head">
            <span>Recent</span>
            <button type="button" className={hover === "history-all" ? "on" : ""} onMouseEnter={() => point("history-all")} onClick={() => run(() => api.openHistory())}>Show History {menuHint(settings?.shortcuts.history) || menuHint(isMac() ? "super+keyy" : "control+keyh")}</button>
          </div>
          {recent.length > 0 ? (
            <div className="recent-grid">
              {recent.map((item) => (
                <button key={item.id} type="button" className={hover === item.id ? "recent-item on" : "recent-item"} onMouseEnter={() => point(item.id)} onClick={() => run(() => api.openEditor(item.id))}>
                  <span className="recent-card"><img key={`${item.id}-${item.previewRev ?? 0}`} src={shotUrl(item)} alt="" /></span>
                  <span className="recent-cap">{caption(item)}</span>
                </button>
              ))}
            </div>
          ) : (
            <p className={hover === "history-empty" ? "recent-empty on" : "recent-empty"} onMouseEnter={() => point("history-empty")}>No recent captures</p>
          )}
        </div>
        <div className="menu-rule tight" />
        <Row id="open" hover={hover} setHover={point} label="Open image…" hint={menuHint(isMac() ? "super+keyo" : "control+keyo")} onClick={() => run(() => api.openImage())} />
        <Row id="clipboard" hover={hover} setHover={point} label="Edit image from clipboard" hint={menuHint(isMac() ? "super+shift+keyv" : "control+shift+keyv")} onClick={() => run(() => api.editClipboard())} />
        <Row id="pins" hover={hover} setHover={point} label={`Pinned images (${pinned.length})`} hint="›" onClick={() => setPinsOpen((open) => !open)} />
        {pinsOpen && (
          <div className="menu-fly">
            {pinned.length === 0 && <p>No pinned images</p>}
            {pinned.map((item) => (
              <button key={item.id} type="button" onClick={() => run(() => api.pin(item.id))}>
                {item.windowTitle || item.sourceApp || "Pinned capture"}
              </button>
            ))}
          </div>
        )}
        <div className="menu-rule" />
        <Row id="settings" hover={hover} setHover={point} label="Settings…" hint={menuHint(isMac() ? "super+comma" : "control+comma")} onClick={() => run(() => api.openSettings())} />
        <Row id="quit" hover={hover} setHover={point} label="Quit Shotsmith" hint={menuHint(isMac() ? "super+keyq" : "control+keyq")} onClick={() => run(() => api.quit())} />
      </div>
    </div>
  );
}

function Row({ id, hover, setHover, label, hint, onClick, disabled = false }: {
  id: string;
  hover: string;
  setHover: (id: string) => void;
  label: string;
  hint: string;
  onClick?: () => void;
  disabled?: boolean;
}) {
  return (
    <button type="button" className={!disabled && hover === id ? "menu-row on" : "menu-row"} disabled={disabled} onMouseEnter={() => { if (!disabled) setHover(id); }} onClick={onClick}>
      <span>{label}</span>
      <span className="menu-key">{hint}</span>
    </button>
  );
}

function caption(item: HistoryItem): string {
  const date = new Date(item.date);
  if (Number.isNaN(date.getTime())) return `${item.pixelWidth}×${item.pixelHeight}`;
  const today = new Date();
  const start = new Date(today.getFullYear(), today.getMonth(), today.getDate()).getTime();
  const day = new Date(date.getFullYear(), date.getMonth(), date.getDate()).getTime();
  const diff = Math.round((start - day) / 86400000);
  if (diff === 1) return "Yesterday";
  if (diff > 1) return date.toLocaleDateString(undefined, { month: "short", day: "numeric" });
  const time = date.toLocaleTimeString(undefined, { hour: "numeric", minute: "2-digit" });
  if (item.mode === "window") return `${time} · Window`;
  return `${time} · ${item.pixelWidth}×${item.pixelHeight}`;
}
