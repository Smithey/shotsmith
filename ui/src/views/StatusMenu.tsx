import { useEffect, useRef, useState } from "react";
import { LogicalPosition, LogicalSize } from "@tauri-apps/api/dpi";
import { currentMonitor } from "@tauri-apps/api/window";
import { api, currentWindow, onHistoryChanged, onRefresh, shotUrl, type HistoryItem, type Settings } from "../api";
import { menuHint } from "../keys";

export function StatusMenu() {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [recent, setRecent] = useState<HistoryItem[]>([]);
  const [pinned, setPinned] = useState<HistoryItem[]>([]);
  const [hover, setHover] = useState("");
  const [pinsOpen, setPinsOpen] = useState(false);
  const [side, setSide] = useState<"left" | "right">("right");
  const base = useRef<{ x: number; y: number } | null>(null);

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
    Promise.all([win.scaleFactor(), win.outerPosition(), currentMonitor()]).then(([factor, pos, monitor]) => {
      if (cancel) return;
      const origin = pos.toLogical(factor);
      if (!base.current) base.current = { x: origin.x, y: origin.y };
      if (!monitor) return;
      const edge = monitor.position.toLogical(monitor.scaleFactor);
      const bounds = monitor.size.toLogical(monitor.scaleFactor);
      setSide(edge.x + bounds.width - (origin.x + 396) < 180 ? "left" : "right");
    }).catch(() => undefined);
    return () => { cancel = true; };
  }, []);

  useEffect(() => {
    const shell = document.querySelector(".status-shell") as HTMLElement | null;
    if (!shell) return;
    const open = hover === "timed";
    const extra = open ? 172 : 0;
    shell.style.paddingLeft = side === "left" ? `${28 + extra}px` : "28px";
    shell.style.paddingRight = side === "right" ? `${28 + extra}px` : "28px";
    const win = currentWindow();
    const origin = base.current;
    if (origin) {
      void win.setPosition(new LogicalPosition(origin.x - (side === "left" ? extra : 0), origin.y));
    }
    void win.setSize(new LogicalSize(396 + extra, shell.offsetHeight));
  }, [recent, pinned, pinsOpen, settings, hover, side]);

  async function run(action: () => Promise<unknown> | void) {
    await currentWindow().hide();
    // Hide returns before the panel is gone from the screen. Wait, or the
    // capture includes the menu.
    await new Promise((resolve) => window.setTimeout(resolve, 140));
    await action();
  }

  const seconds = settings?.timedSeconds ?? 5;

  return (
    <div className="status-shell">
      <div className="status-menu" onMouseLeave={() => setHover("")}>
        <div className="menu-label">Capture</div>
        <Row id="region" hover={hover} setHover={setHover} label="Region" hint={menuHint(settings?.shortcuts.region)} onClick={() => run(() => api.begin("region"))} />
        <Row id="repeat" hover={hover} setHover={setHover} label="Repeat last region" hint={menuHint(settings?.shortcuts.repeatLast)} onClick={() => run(() => api.repeat())} />
        <Row id="window" hover={hover} setHover={setHover} label="Window" hint={menuHint(settings?.shortcuts.window)} onClick={() => run(() => api.begin("window"))} />
        <Row id="screen" hover={hover} setHover={setHover} label="Full screen" hint={menuHint(settings?.shortcuts.fullScreen)} onClick={() => run(() => api.begin("screen"))} />
        <Row id="scroll" hover={hover} setHover={setHover} label="Scrolling…" hint="" onClick={() => run(() => api.begin("scroll"))} />
        <div className={side === "left" ? "menu-sub flip" : "menu-sub"} onMouseEnter={() => { setHover("timed"); setPinsOpen(false); }}>
          <button type="button" className={hover === "timed" ? "menu-row on" : "menu-row"} onClick={() => run(() => api.countdown())}>
            <span>Timed · {seconds} s</span>
            <span className="menu-key">›</span>
          </button>
          {hover === "timed" && (
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
        <Row id="text" hover={hover} setHover={setHover} label="Copy text from screen" hint="" onClick={() => run(() => api.begin("text"))} />
        <Row id="measure" hover={hover} setHover={setHover} label="Measure" hint="" onClick={() => run(() => api.begin("measure"))} />
        <div className="menu-rule" />
        <div className="recent-block" onMouseEnter={() => setHover("history")}>
          <div className="recent-head">
            <span>Recent</span>
            <button type="button" className={hover === "history-all" ? "on" : ""} onMouseEnter={() => setHover("history-all")} onClick={() => run(() => api.openHistory())}>Show History {menuHint(settings?.shortcuts.history) || "⌘Y"}</button>
          </div>
          {recent.length > 0 ? (
            <div className="recent-grid">
              {recent.map((item) => (
                <button key={item.id} type="button" className={hover === item.id ? "recent-item on" : "recent-item"} onMouseEnter={() => setHover(item.id)} onClick={() => run(() => api.openEditor(item.id))}>
                  <span className="recent-card"><img key={`${item.id}-${item.previewRev ?? 0}`} src={shotUrl(item)} alt="" /></span>
                  <span className="recent-cap">{caption(item)}</span>
                </button>
              ))}
            </div>
          ) : (
            <p className={hover === "history-empty" ? "recent-empty on" : "recent-empty"} onMouseEnter={() => setHover("history-empty")}>No recent captures</p>
          )}
        </div>
        <div className="menu-rule tight" />
        <Row id="open" hover={hover} setHover={setHover} label="Open image…" hint="⌘O" onClick={() => run(() => api.openImage())} />
        <Row id="clipboard" hover={hover} setHover={setHover} label="Edit image from clipboard" hint="⌘⇧V" onClick={() => run(() => api.editClipboard())} />
        <Row id="pins" hover={hover} setHover={setHover} label={`Pinned images (${pinned.length})`} hint="›" onClick={() => setPinsOpen((open) => !open)} />
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
        <Row id="settings" hover={hover} setHover={setHover} label="Settings…" hint="⌘," onClick={() => run(() => api.openSettings())} />
        <Row id="quit" hover={hover} setHover={setHover} label="Quit Shotsmith" hint="⌘Q" onClick={() => run(() => api.quit())} />
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
