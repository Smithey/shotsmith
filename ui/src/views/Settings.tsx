import { useEffect, useState } from "react";
import { api, type Preset, type Settings as SettingsModel } from "../api";
import type { ImageFormat } from "../bindings/ImageFormat";

const PAGES = ["General", "Capture", "Shortcuts", "Presets", "Donut", "Destinations", "Advanced", "About"];

export function Settings() {
  const [page, setPage] = useState("Presets");
  const [settings, setSettings] = useState<SettingsModel | null>(null);
  const [presets, setPresets] = useState<Preset[]>([]);
  const [selected, setSelected] = useState("builtin-docs-png");
  const [preview, setPreview] = useState("");
  const [error, setError] = useState("");

  useEffect(() => {
    Promise.all([api.settings(), api.presets()]).then(([next, list]) => {
      setSettings(next);
      setPresets(list);
      setSelected(next.defaultPresetId);
    }).catch((err: Error) => setError(err.message));
  }, []);

  const preset = presets.find((item) => item.id === selected) ?? presets[0];

  useEffect(() => {
    if (!preset) return;
    api.filenamePreview(preset.filenamePattern, 1320, 800).then(setPreview).catch(() => undefined);
  }, [preset?.filenamePattern, preset?.id]);

  function saveSettings(next: SettingsModel) {
    setSettings(next);
    api.saveSettings(next).catch((err: Error) => setError(err.message));
  }

  function savePreset(next: Preset) {
    api.savePreset(next).then(setPresets).catch((err: Error) => setError(err.message));
  }

  if (!settings || !preset) return <p className="pane">{error || "Loading settings…"}</p>;
  const lossless = preset.format === "png" || preset.format === "tiff";

  return (
    <div className="settings">
      <nav className="nav">
        {PAGES.map((item) => <button key={item} className={page === item ? "active" : ""} onClick={() => setPage(item)}>{item}</button>)}
      </nav>
      <section className="pane">
        {error && <p className="error">{error}</p>}
        {page === "General" && (
          <>
            <h1>General</h1>
            <Toggle label="Launch at login" on={settings.launchAtLogin} set={(launchAtLogin) => saveSettings({ ...settings, launchAtLogin })} />
            <Toggle label="Show Dock icon where the system has one" on={settings.showDockIcon} set={(showDockIcon) => saveSettings({ ...settings, showDockIcon })} />
            <Toggle label="Shutter sound" on={settings.sound} set={(sound) => saveSettings({ ...settings, sound })} />
            <Toggle label="Show the result HUD" on={settings.hudEnabled} set={(hudEnabled) => saveSettings({ ...settings, hudEnabled })} />
            <label>Appearance</label>
            <select className="field" value={settings.appearance} onChange={(event) => saveSettings({ ...settings, appearance: event.target.value })}>
              <option value="system">System</option>
              <option value="light">Light</option>
              <option value="dark">Dark</option>
            </select>
          </>
        )}
        {page === "Capture" && (
          <>
            <h1>Capture</h1>
            <Toggle label="Loupe" on={settings.loupe} set={(loupe) => saveSettings({ ...settings, loupe })} />
            <Toggle label="Overlay toolbar" on={settings.overlayToolbar} set={(overlayToolbar) => saveSettings({ ...settings, overlayToolbar })} />
            <Toggle label="Include cursor as an object" on={settings.includeCursor} set={(includeCursor) => saveSettings({ ...settings, includeCursor })} />
            <label>Timed capture</label>
            <select className="field" value={settings.timedSeconds} onChange={(event) => saveSettings({ ...settings, timedSeconds: Number(event.target.value) })}>
              <option value={3}>3 seconds</option>
              <option value={5}>5 seconds</option>
              <option value={10}>10 seconds</option>
            </select>
            <select className="field" value={settings.timedMode} onChange={(event) => saveSettings({ ...settings, timedMode: event.target.value })}>
              <option value="region">Then region</option>
              <option value="screen">Then full screen</option>
            </select>
          </>
        )}
        {page === "Shortcuts" && (
          <>
            <h1>Shortcuts</h1>
            <p>Use words separated by plus signs: control, alt, shift, super, and a key such as digit2 or keyh.</p>
            {(Object.keys(settings.shortcuts) as Array<keyof SettingsModel["shortcuts"]>).map((key) => (
              <label key={key}>{key}
                <input className="field" value={settings.shortcuts[key]} onChange={(event) => {
                  const shortcuts = { ...settings.shortcuts, [key]: event.target.value };
                  const values = Object.values(shortcuts);
                  if (values.filter((item) => item === event.target.value).length > 1) setError("That shortcut is already used.");
                  else setError("");
                  saveSettings({ ...settings, shortcuts });
                }} />
              </label>
            ))}
          </>
        )}
        {page === "Presets" && (
          <>
            <h1>Presets</h1>
            <div className="cards">
              {presets.map((item) => (
                <button key={item.id} className={`card ${item.id === preset.id ? "active" : ""}`} onClick={() => setSelected(item.id)}>
                  <strong>{item.name}</strong>
                  <div>{item.format} · {item.folder ? "file" : "clipboard only"}</div>
                </button>
              ))}
            </div>
            <label>Name</label>
            <input className="field" value={preset.name} onChange={(event) => savePreset({ ...preset, name: event.target.value })} />
            <label>Format</label>
            <select className="field" value={preset.format} onChange={(event) => savePreset({ ...preset, format: event.target.value as ImageFormat })}>
              {["png", "jpeg", "webp", "tiff", "pdf", "heic"].map((format) => <option key={format} value={format}>{format}</option>)}
            </select>
            <label>Quality {lossless ? "(not used for this format)" : Math.round(preset.quality * 100)}</label>
            <input className="field" type="range" min={0.1} max={1} step={0.05} disabled={lossless} value={preset.quality} onChange={(event) => savePreset({ ...preset, quality: Number(event.target.value) })} />
            <label>Scale</label>
            <select className="field" value={preset.scale} onChange={(event) => savePreset({ ...preset, scale: event.target.value as Preset["scale"] })}>
              <option value="points">@1x</option>
              <option value="native">Native</option>
            </select>
            <label>Folder</label>
            <div className="row">
              <input className="field" value={preset.folder ?? ""} placeholder="Clipboard only when empty" onChange={(event) => savePreset({ ...preset, folder: event.target.value || null })} />
              <button onClick={() => api.pickFolder().then((folder) => folder && savePreset({ ...preset, folder }))}>Choose</button>
            </div>
            <label>Filename pattern</label>
            <input className="field mono" value={preset.filenamePattern} onChange={(event) => savePreset({ ...preset, filenamePattern: event.target.value })} />
            <p>Preview: {preview}.{preset.format === "jpeg" ? "jpg" : preset.format}</p>
            <p>Existing names get “ (2)”.</p>
            <Toggle label="Strip metadata" on={preset.stripMetadata} set={(stripMetadata) => savePreset({ ...preset, stripMetadata })} />
            <button onClick={() => saveSettings({ ...settings, defaultPresetId: preset.id })}>Use as default</button>
            <button onClick={() => api.exportPresets().then((json) => {
              const blob = new Blob([json], { type: "application/json" });
              const link = document.createElement("a");
              link.href = URL.createObjectURL(blob);
              link.download = "shotsmith-presets.json";
              link.click();
            })}>Export JSON</button>
          </>
        )}
        {page === "Donut" && (
          <>
            <h1>Donut</h1>
            <label>Slots</label>
            <select className="field" value={settings.donutCount} onChange={(event) => saveSettings({ ...settings, donutCount: Number(event.target.value) })}>
              <option value={4}>4</option>
              <option value={6}>6</option>
              <option value={8}>8</option>
            </select>
            {settings.donutSlots.slice(0, settings.donutCount).map((slot, index) => (
              <div className="row" key={index}>
                <select className="field" value={slot} onChange={(event) => {
                  const donutSlots = settings.donutSlots.slice();
                  donutSlots[index] = event.target.value;
                  saveSettings({ ...settings, donutSlots });
                }}>
                  <option value="copy">Copy</option>
                  <option value="save">Save</option>
                  <option value="annotate">Annotate</option>
                  <option value="pin">Pin</option>
                  {presets.map((item) => <option key={item.id} value={`preset:${item.id}`}>{item.name}</option>)}
                </select>
                <button onClick={() => moveSlot(settings, index, -1, saveSettings)}>Up</button>
                <button onClick={() => moveSlot(settings, index, 1, saveSettings)}>Down</button>
              </div>
            ))}
          </>
        )}
        {page === "Destinations" && (
          <>
            <h1>Destinations</h1>
            <label>After capture</label>
            <select className="field" value={settings.afterCapture} onChange={(event) => saveSettings({ ...settings, afterCapture: event.target.value })}>
              <option value="donut">Show donut</option>
              <option value="copy">Copy</option>
              <option value="save">Save with the default preset</option>
              <option value="editor">Open editor</option>
            </select>
          </>
        )}
        {page === "Advanced" && (
          <>
            <h1>Advanced</h1>
            <Toggle label="Keep history" on={settings.keepHistory} set={(keepHistory) => saveSettings({ ...settings, keepHistory })} />
            <label>Keep this many captures</label>
            <input className="field" type="number" value={settings.historyLimit} onChange={(event) => saveSettings({ ...settings, historyLimit: Number(event.target.value) })} />
            <label>And this many days</label>
            <input className="field" type="number" value={settings.historyDays} onChange={(event) => saveSettings({ ...settings, historyDays: Number(event.target.value) })} />
            <p>History lives in the Shotsmith application support folder.</p>
          </>
        )}
        {page === "About" && (
          <>
            <h1>Shotsmith 0.1.0</h1>
            <p>Free screenshot and annotation tool. MIT licence. This build does not phone home.</p>
            <p>OCR, share, and scrolling capture come in later versions.</p>
          </>
        )}
      </section>
    </div>
  );
}

function Toggle({ label, on, set }: { label: string; on: boolean; set: (value: boolean) => void }) {
  return <label className="row"><input type="checkbox" checked={on} onChange={(event) => set(event.target.checked)} /> {label}</label>;
}

function moveSlot(settings: SettingsModel, index: number, delta: number, save: (next: SettingsModel) => void) {
  const next = index + delta;
  if (next < 0 || next >= settings.donutSlots.length) return;
  const donutSlots = settings.donutSlots.slice();
  const [item] = donutSlots.splice(index, 1);
  donutSlots.splice(next, 0, item);
  save({ ...settings, donutSlots });
}
