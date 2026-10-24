import { useEffect, useId, useRef, useState, type ReactNode } from "react";
import { api, uid, type PluginInfo, type Preset, type Settings as SettingsModel } from "../api";
import { t } from "../i18n";
import { canonicalShortcut, fromKeyEvent, isMac, keyLabel, splitShortcut } from "../keys";
import type { ImageFormat } from "../bindings/ImageFormat";
import type { OutputScale } from "../bindings/OutputScale";

const NAV: Array<[string, string]> = [
  ["General", "#8e8e93"],
  ["Capture", "#0a84ff"],
  ["Shortcuts", "#636366"],
  ["Presets", "#30b158"],
  ["Donut", "#ff9f0a"],
  ["Destinations", "#5e5ce6"],
  ["Automation", "#bf5af2"],
  ["Plugins", "#ff375f"],
  ["Advanced", "#636366"],
  ["About", "#8e8e93"],
];

function NavIcon({ name }: { name: string }) {
  const pen = { fill: "none", stroke: "#fff", strokeWidth: 1.6, strokeLinecap: "round" as const, strokeLinejoin: "round" as const };
  return (
    <svg width="14" height="14" viewBox="0 0 14 14" aria-hidden="true">
      {name === "General" && (
        <>
          <circle {...pen} cx="7" cy="7" r="2" />
          <path {...pen} d="M7 1.4v1.5M7 11.1v1.5M1.4 7h1.5M11.1 7h1.5M3 3l1.1 1.1M9.9 9.9 11 11M11 3 9.9 4.1M4.1 9.9 3 11" />
        </>
      )}
      {name === "Capture" && (
        <>
          <path {...pen} d="M2 4.2h1.3l.8-1.3h5.8l.8 1.3H12v6.4H2z" />
          <circle {...pen} cx="7" cy="7.2" r="1.7" />
        </>
      )}
      {name === "Shortcuts" && <path {...pen} d="M4.2 2.2h2.1v2.1H4.2zM7.7 2.2h2.1v2.1H7.7zM4.2 5.7h2.1v2.6H4.2zM7.7 5.7h2.1v2.6H7.7zM4.2 9.7h5.6v2.1H4.2z" />}
      {name === "Presets" && (
        <>
          <rect {...pen} x="2.6" y="3.4" width="8.8" height="7.4" rx="1.2" />
          <path {...pen} d="M4.6 3.4V2.6A1 1 0 0 1 5.6 1.6h2.8a1 1 0 0 1 1 1v.8" />
        </>
      )}
      {name === "Donut" && (
        <>
          <circle {...pen} cx="7" cy="7" r="4.6" />
          <circle {...pen} cx="7" cy="7" r="1.5" />
        </>
      )}
      {name === "Destinations" && <path {...pen} d="M7 1.8v6.2M4.2 5.6 7 8.4l2.8-2.8M2.4 11.2h9.2" />}
      {name === "Automation" && <path fill="#fff" d="M8.1 1.2 3.4 7.4h2.6L5.2 12.8 10.6 6.2H8z" />}
      {name === "Plugins" && (
        <>
          <path {...pen} d="M5.2 2.2v2M8.8 2.2v2" />
          <rect {...pen} x="2.4" y="4" width="9.2" height="7.2" rx="1.6" />
          <path {...pen} d="M5.2 7.6h3.6" />
        </>
      )}
      {name === "Advanced" && (
        <>
          <path {...pen} d="M1.6 4h10.8M1.6 10h10.8" />
          <circle cx="4.6" cy="4" r="1.5" fill="#fff" />
          <circle cx="9.4" cy="10" r="1.5" fill="#fff" />
        </>
      )}
      {name === "About" && (
        <>
          <circle {...pen} cx="7" cy="7" r="5" />
          <path {...pen} d="M7 6.3v3.4" />
          <circle cx="7" cy="4.2" r="0.8" fill="#fff" />
        </>
      )}
    </svg>
  );
}

const FORMATS: ImageFormat[] = ["png", "jpeg", "heic", "webp", "tiff", "pdf"];
const FORMAT_LABEL: Record<ImageFormat, string> = { png: "PNG", jpeg: "JPEG", heic: "HEIC", webp: "WebP", tiff: "TIFF", pdf: "PDF" };
const TOKENS = ["{app}", "{window}", "{yyyy}", "{MM}", "{dd}", "{HH}", "{mm}", "{ss}", "{w}", "{h}", "{n}"];

export function Settings() {
  const [page, setPage] = useState("General");
  const [settings, setSettings] = useState<SettingsModel | null>(null);
  const [presets, setPresets] = useState<Preset[]>([]);
  const [selected, setSelected] = useState("builtin-docs-png");
  const [preview, setPreview] = useState("");
  const [error, setError] = useState("");
  const [plugins, setPlugins] = useState<PluginInfo[]>([]);
  const [updateNote, setUpdateNote] = useState("");

  useEffect(() => {
    Promise.all([api.settings(), api.presets()]).then(([next, list]) => {
      setSettings(next);
      setPresets(list);
      setSelected(next.defaultPresetId);
    }).catch((err: Error) => setError(err.message));
  }, []);

  useEffect(() => {
    if (settings?.locale) document.documentElement.lang = settings.locale;
  }, [settings?.locale]);

  useEffect(() => {
    if (page === "Plugins") api.plugins().then(setPlugins).catch(() => setPlugins([]));
  }, [page]);

  const preset = presets.find((item) => item.id === selected) ?? presets[0];

  useEffect(() => {
    if (!preset) return;
    api.filenamePreview(preset.filenamePattern, 1320, 800).then(setPreview).catch(() => undefined);
  }, [preset?.filenamePattern, preset?.id]);

  function saveSettings(next: SettingsModel) {
    setSettings(next);
    return api.saveSettings(next).catch((err: Error) => setError(err.message));
  }

  function savePreset(next: Preset) {
    return api.savePreset(next).then(setPresets).catch((err: Error) => setError(err.message));
  }

  if (!settings || !preset) return <p className="set-loading">{error || "Loading settings…"}</p>;

  return (
    <div className="settings">
      <nav className="set-nav">
        {NAV.map(([label, color]) => (
          <button key={label} type="button" className={page === label ? "on" : ""} onClick={() => setPage(label)}>
            <span className="set-dot" style={{ background: color, boxShadow: page === label ? "inset 0 0 0 1.5px #ffffff" : undefined }}>
              <NavIcon name={label} />
            </span>
            {label}
          </button>
        ))}
      </nav>
      <section className="set-main">
        <header><strong>{page}</strong></header>
        <div className="set-body">
          {error && <p className="error">{error}</p>}
          {page === "General" && <General settings={settings} save={saveSettings} />}
          {page === "Capture" && <Capture settings={settings} save={saveSettings} />}
          {page === "Shortcuts" && <Shortcuts settings={settings} presets={presets} save={saveSettings} setError={setError} />}
          {page === "Presets" && (
            <Presets
              settings={settings}
              presets={presets}
              preset={preset}
              preview={preview}
              saveSettings={saveSettings}
              savePreset={savePreset}
              setError={setError}
              setPresets={setPresets}
              setSelected={setSelected}
            />
          )}
          {page === "Donut" && <Donut settings={settings} presets={presets} save={saveSettings} />}
          {page === "Destinations" && <Destinations settings={settings} save={saveSettings} />}
          {page === "Automation" && <Automation settings={settings} save={saveSettings} />}
          {page === "Plugins" && <Plugins settings={settings} plugins={plugins} save={saveSettings} />}
          {page === "Advanced" && <Advanced settings={settings} save={saveSettings} updateNote={updateNote} setUpdateNote={setUpdateNote} />}
          {page === "About" && (
            <div className="set-fill">
              <Group>
                <div className="set-note">
                  <strong>Shotsmith 0.1.0</strong>
                  <p>Free screenshot and annotation tool. MIT licence. This build does not phone home.</p>
                </div>
              </Group>
            </div>
          )}
        </div>
      </section>
    </div>
  );
}

function General({ settings, save }: { settings: SettingsModel; save: (next: SettingsModel) => void }) {
  return (
    <div className="set-fill">
      <Group>
        <Check label="Launch at login" on={settings.launchAtLogin} set={(launchAtLogin) => save({ ...settings, launchAtLogin })} />
        {isMac() && <Check label="Show Dock icon" info="Hides the icon in the Dock. Shotsmith stays in the menu bar." on={settings.showDockIcon} set={(showDockIcon) => save({ ...settings, showDockIcon })} />}
        <Check label="Shutter sound" on={settings.sound} set={(sound) => save({ ...settings, sound })} />
        <Check label="Show the result HUD" info="A small banner after a capture. It says what happened, such as copied or saved." on={settings.hudEnabled} set={(hudEnabled) => save({ ...settings, hudEnabled })} />
        <Row label="Appearance">
          <select className="set-input" value={settings.appearance} onChange={(event) => save({ ...settings, appearance: event.target.value })}>
            <option value="system">System</option>
            <option value="light">Light</option>
            <option value="dark">Dark</option>
          </select>
        </Row>
      </Group>
    </div>
  );
}

function Capture({ settings, save }: { settings: SettingsModel; save: (next: SettingsModel) => void }) {
  return (
    <div className="set-fill">
      <Group>
        <Check label="Loupe" info="Magnifies the pixels under the pointer while you drag a region." on={settings.loupe} set={(loupe) => save({ ...settings, loupe })} />
        <Check label="Overlay toolbar" info="Shows the capture tools on the selection screen." on={settings.overlayToolbar} set={(overlayToolbar) => save({ ...settings, overlayToolbar })} />
        <Check label="Include cursor as an object" info="Adds the pointer as a shape you can move or delete. It is not drawn into the picture." on={settings.includeCursor} set={(includeCursor) => save({ ...settings, includeCursor })} />
        <Row label="Timed capture" info="Waits, then takes the shot. Use the pause to open a menu or a tooltip.">
          <select className="set-input" value={settings.timedSeconds} onChange={(event) => save({ ...settings, timedSeconds: Number(event.target.value) })}>
            <option value={3}>3 seconds</option>
            <option value={5}>5 seconds</option>
            <option value={10}>10 seconds</option>
          </select>
        </Row>
        <Row label="Then" info="What the timer captures when the wait ends.">
          <select className="set-input" value={settings.timedMode} onChange={(event) => save({ ...settings, timedMode: event.target.value })}>
            <option value="region">Region</option>
            <option value="screen">Full screen</option>
          </select>
        </Row>
      </Group>
    </div>
  );
}

function Shortcuts({ settings, presets, save, setError }: {
  settings: SettingsModel;
  presets: Preset[];
  save: (next: SettingsModel) => void;
  setError: (value: string) => void;
}) {
  return (
    <div className="set-fill">
      <Group>
        {(Object.keys(settings.shortcuts) as Array<keyof SettingsModel["shortcuts"]>).map((key) => (
          <Row key={key} label={shortcutName(key)} info={shortcutInfo(key)}>
            <ShortcutField
              value={settings.shortcuts[key]}
              onError={setError}
              taken={(next) => shortcutConflict(next, key, settings, presets)}
              onChange={(value) => save({ ...settings, shortcuts: { ...settings.shortcuts, [key]: value } })}
            />
          </Row>
        ))}
      </Group>
      <p className="set-hint">Click a shortcut, then press the new keys. Backspace clears it. Escape cancels.</p>
    </div>
  );
}

function Presets({ settings, presets, preset, preview, saveSettings, savePreset, setError, setPresets, setSelected }: {
  settings: SettingsModel;
  presets: Preset[];
  preset: Preset;
  preview: string;
  saveSettings: (next: SettingsModel) => void;
  savePreset: (next: Preset) => void;
  setError: (value: string) => void;
  setPresets: (next: Preset[]) => void;
  setSelected: (id: string) => void;
}) {
  const ext = preset.format === "jpeg" ? "jpg" : preset.format;
  return (
    <div className="preset-split">
      <div className="preset-list">
        {presets.map((item) => {
          const on = item.id === preset.id;
          return (
            <button key={item.id} type="button" className={on ? "preset-card on" : "preset-card"} onClick={() => {
              setSelected(item.id);
              saveSettings({ ...settings, defaultPresetId: item.id });
            }}>
              <span><strong>{item.name}</strong><span>{subtitle(item)}</span></span>
              {on && <em>Default</em>}
            </button>
          );
        })}
        <div className="preset-tools">
          <button type="button" data-tip="Add preset" aria-label="Add preset" onClick={() => {
            const next: Preset = { ...preset, id: uid(), name: "New preset", builtin: false, donutSlot: null, shortcut: null };
            api.savePreset(next).then((list) => { setPresets(list); setSelected(next.id); });
          }}>
            <svg width="14" height="14" viewBox="0 0 14 14" aria-hidden="true"><path d="M7 2.5v9M2.5 7h9" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" /></svg>
          </button>
          <button type="button" data-tip="Remove preset" aria-label="Remove preset" disabled={preset.builtin} onClick={() => {
            api.deletePreset(preset.id).then((list) => {
              setPresets(list);
              const fallback = list[0];
              if (fallback) {
                setSelected(fallback.id);
                saveSettings({ ...settings, defaultPresetId: fallback.id });
              }
            });
          }}>
            <svg width="14" height="14" viewBox="0 0 14 14" aria-hidden="true"><path d="M3 7h8" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" /></svg>
          </button>
          <button type="button" className="import" onClick={() => document.getElementById("preset-import")?.click()}>Import…</button>
          <input id="preset-import" hidden type="file" accept="application/json,.json" onChange={(event) => {
            const file = event.target.files?.[0];
            if (!file) return;
            file.text().then((json) => api.importPresets(json).then(setPresets));
          }} />
        </div>
      </div>
      <div className="preset-editor">
        <Group>
          <Row label="Name">
            <input id="p-name" className="set-input" value={preset.name} onChange={(event) => savePreset({ ...preset, name: event.target.value })} />
          </Row>
          <Row label="Format" info="PNG is lossless. JPEG and WebP are smaller. HEIC is listed, but this build cannot save it yet.">
            <Segment value={preset.format} options={FORMATS.map((format) => [format, FORMAT_LABEL[format]])} set={(format) => savePreset({ ...preset, format })} />
          </Row>
          {(preset.format === "jpeg" || preset.format === "webp" || preset.format === "heic") && (
            <Row label={`Quality ${Math.round(preset.quality * 100)}`}>
              <input type="range" min={0.1} max={1} step={0.05} value={preset.quality} onChange={(event) => savePreset({ ...preset, quality: Number(event.target.value) })} />
            </Row>
          )}
          <Row label="Scale" info="@1x matches the size on screen. Native keeps every pixel from a Retina display.">
            <Segment value={preset.scale} options={[["points", "@1x"], ["native", "Native (@2x)"]] as Array<[OutputScale, string]>} set={(scale) => savePreset({ ...preset, scale })} />
          </Row>
          <Row label="Folder">
            <span className="folder-line">
              <span>{preset.folder ? preset.folder.replace(/^\/Users\/[^/]+/, "~") : "Clipboard only"}</span>
              <button type="button" onClick={() => {
                api.pickFolder().then((folder) => {
                  if (folder) savePreset({ ...preset, folder });
                }).catch((err: Error) => setError(err.message));
              }}>Choose…</button>
              {preset.folder && <button type="button" onClick={() => savePreset({ ...preset, folder: null })}>Clear</button>}
            </span>
          </Row>
        </Group>
        <div className="set-card pattern-card">
          <span className="set-label"><label htmlFor="p-pattern">Filename pattern</label><Info text={'Words in braces are filled in when the file is saved. A name that already exists gets " (2)".'} /></span>
          <input id="p-pattern" className="mono" value={preset.filenamePattern} onChange={(event) => savePreset({ ...preset, filenamePattern: event.target.value })} />
          <div className="token-row">
            {TOKENS.map((token) => (
              <button key={token} type="button" className="mono" onClick={() => savePreset({ ...preset, filenamePattern: preset.filenamePattern + token })}>{token}</button>
            ))}
          </div>
          <span className="set-hint">Preview: <span>{preview}.{ext}</span> · existing names get “ (2)”</span>
        </div>
        <div className="preset-grid">
          <div className="set-card">
            <strong>After saving</strong>
            <Check label="Copy image to clipboard" on={preset.afterSave.includes("copyImage")} set={() => savePreset({ ...preset, afterSave: toggle(preset.afterSave, "copyImage") })} />
            <Check label="Copy file path" on={preset.afterSave.includes("copyPath")} set={() => savePreset({ ...preset, afterSave: toggle(preset.afterSave, "copyPath") })} />
            <Check label={isMac() ? "Reveal in Finder" : "Reveal in File Explorer"} on={preset.afterSave.includes("reveal")} set={() => savePreset({ ...preset, afterSave: toggle(preset.afterSave, "reveal") })} />
            <Check label="Strip metadata" info="Removes location and camera details from the saved file." on={preset.stripMetadata} set={(stripMetadata) => savePreset({ ...preset, stripMetadata })} />
          </div>
          <div className="set-card">
            <strong>Effects</strong>
            <Check label="Drop shadow" info="Puts a soft shadow outside the picture when you save." on={preset.effects.includes("shadow")} set={() => savePreset({ ...preset, effects: toggle(preset.effects, "shadow") })} />
            <Check label="Border 1 px" info="Draws a thin line around the saved picture." on={preset.effects.includes("border")} set={() => savePreset({ ...preset, effects: toggle(preset.effects, "border") })} />
            <Check label="Torn edges" info="Roughens the edges of the saved picture." on={preset.effects.includes("tornEdges")} set={() => savePreset({ ...preset, effects: toggle(preset.effects, "tornEdges") })} />
            <Check label="Show on donut (slot 7)" info="Adds this preset to the wheel that appears after a capture." on={preset.donutSlot === 7} set={(on) => savePreset({ ...preset, donutSlot: on ? 7 : null })} />
          </div>
        </div>
        <Group>
          <Row label="Global shortcut" info="Runs this preset from any app. It has to differ from the capture shortcuts.">
            <ShortcutField
              value={preset.shortcut ?? ""}
              onError={setError}
              taken={(next) => shortcutConflict(next, `preset:${preset.id}`, settings, presets)}
              onChange={(value) => savePreset({ ...preset, shortcut: value || null })}
            />
          </Row>
        </Group>
      </div>
    </div>
  );
}

function Donut({ settings, presets, save }: { settings: SettingsModel; presets: Preset[]; save: (next: SettingsModel) => void }) {
  return (
    <div className="set-fill">
      <Group>
        <Row label="Slots" info="How many actions appear on the wheel after a capture.">
          <select className="set-input" value={settings.donutCount} onChange={(event) => save({ ...settings, donutCount: Number(event.target.value) })}>
            <option value={4}>4</option>
            <option value={6}>6</option>
            <option value={8}>8</option>
          </select>
        </Row>
        {settings.donutSlots.slice(0, settings.donutCount).map((slot, index) => (
          <Row key={index} label={`Slot ${index + 1}`}>
            <select className="set-input" value={slot} onChange={(event) => {
              const donutSlots = settings.donutSlots.slice();
              donutSlots[index] = event.target.value;
              save({ ...settings, donutSlots });
            }}>
              <option value="copy">Copy</option>
              <option value="save">Save</option>
              <option value="annotate">Annotate</option>
              <option value="pin">Pin</option>
              <option value="share">Share</option>
              <option value="copy-text">Copy text</option>
              <option value="open-in">Open in</option>
              <option value="print">Print</option>
              {presets.map((item) => <option key={item.id} value={`preset:${item.id}`}>{item.name}</option>)}
            </select>
            <button type="button" className="set-mini" onClick={() => moveSlot(settings, index, -1, save)}>Up</button>
            <button type="button" className="set-mini" onClick={() => moveSlot(settings, index, 1, save)}>Down</button>
          </Row>
        ))}
      </Group>
    </div>
  );
}

function Destinations({ settings, save }: { settings: SettingsModel; save: (next: SettingsModel) => void }) {
  return (
    <div className="set-fill">
      <Group>
        <Row label="After capture" info="What Shotsmith does as soon as the shot is taken. The donut is the wheel of actions.">
          <select className="set-input" value={settings.afterCapture} onChange={(event) => save({ ...settings, afterCapture: event.target.value })}>
            <option value="donut">Show donut</option>
            <option value="copy">Copy</option>
            <option value="save">Save with the default preset</option>
            <option value="editor">Open editor</option>
          </select>
        </Row>
        <Check label="Warn before saving a card number or key" info="Stops copy, save, share, and print while a card number or a key is still visible." on={settings.warnOnSecrets} set={(warnOnSecrets) => save({ ...settings, warnOnSecrets })} />
      </Group>
    </div>
  );
}

function Automation({ settings, save }: { settings: SettingsModel; save: (next: SettingsModel) => void }) {
  return (
    <div className="set-fill">
      <Group>
        <div className="set-note"><strong>{t("automation.title")}</strong><p>{t("automation.command")}</p><p>{t("automation.link")}</p></div>
        {isMac() && <div className="set-note"><strong>Apple Shortcuts</strong><p>{t("automation.shortcuts")}</p></div>}
      </Group>
      <Group>
        <div className="set-note"><strong>{t("automation.watch")}</strong></div>
        {settings.watchFolders.map((folder) => (
          <Row key={folder} label={folder}>
            <button type="button" className="set-mini" onClick={() => save({ ...settings, watchFolders: settings.watchFolders.filter((item) => item !== folder) })}>Remove</button>
          </Row>
        ))}
        <Row label={t("automation.add")} info="Images added here are imported once the file size stays the same for a second.">
          <button type="button" className="set-mini" onClick={() => api.pickFolder().then((folder) => {
            if (!folder || settings.watchFolders.includes(folder)) return;
            save({ ...settings, watchFolders: [...settings.watchFolders, folder] });
          })}>Choose</button>
        </Row>
      </Group>
    </div>
  );
}

function Plugins({ settings, plugins, save }: { settings: SettingsModel; plugins: PluginInfo[]; save: (next: SettingsModel) => void }) {
  return (
    <div className="set-fill">
      <Group>
        <div className="set-note"><strong>{t("plugins.title")}</strong><p>{t("plugins.off")}</p></div>
        <Check label={t("plugins.enable")} info="Each plugin runs outside Shotsmith. The line under its name says what it uploads." on={settings.pluginsEnabled} set={(pluginsEnabled) => save({ ...settings, pluginsEnabled })} />
        {plugins.length === 0 && <div className="set-note"><p>{t("plugins.empty")}</p></div>}
        {plugins.map((plugin) => (
          <div key={plugin.id} className="set-note"><strong>{plugin.name}</strong><p>{plugin.uploads}</p></div>
        ))}
      </Group>
    </div>
  );
}

function Advanced({ settings, save, updateNote, setUpdateNote }: { settings: SettingsModel; save: (next: SettingsModel) => void; updateNote: string; setUpdateNote: (value: string) => void }) {
  return (
    <div className="set-fill">
      <Group>
        <Check label="Keep history" info="Saves captures so you can open them later. New captures are not stored while this is off." on={settings.keepHistory} set={(keepHistory) => save({ ...settings, keepHistory })} />
        <Row label="Keep this many captures" info="When you pass this number, the oldest captures are removed.">
          <input className="set-input" type="number" value={settings.historyLimit} onChange={(event) => save({ ...settings, historyLimit: Number(event.target.value) })} />
        </Row>
        <Row label="And this many days" info="Captures older than this are removed.">
          <input className="set-input" type="number" value={settings.historyDays} onChange={(event) => save({ ...settings, historyDays: Number(event.target.value) })} />
        </Row>
        <Check label={t("updates.check")} info="Off until you turn it on. Check asks GitHub which version is newest. Nothing else is sent." on={settings.checkUpdates} set={(checkUpdates) => save({ ...settings, checkUpdates })} />
        <Row label={t("updates.button")}>
          <button type="button" className="set-mini" disabled={!settings.checkUpdates} onClick={() => api.checkUpdates().then(setUpdateNote).catch((err: Error) => setUpdateNote(err.message))}>Check</button>
        </Row>
        {updateNote && <div className="set-note"><p>{updateNote}</p></div>}
        <Row label={t("language.label")} info="English is the only language in this build.">
          <select className="set-input" value={settings.locale || "en"} onChange={(event) => save({ ...settings, locale: event.target.value })}>
            <option value="en">English</option>
          </select>
        </Row>
      </Group>
      <p className="set-hint">History lives in the Shotsmith application support folder.</p>
    </div>
  );
}

function Group({ children }: { children: ReactNode }) {
  return <div className="set-group">{children}</div>;
}

function Row({ label, info, children }: { label: string; info?: string; children: ReactNode }) {
  return (
    <div className="set-row">
      <span className="set-label">{label}{info && <Info text={info} />}</span>
      <div className="set-end">{children}</div>
    </div>
  );
}

function Check({ label, info, on, set }: { label: string; info?: string; on: boolean; set: (value: boolean) => void }) {
  const id = useId();
  return (
    <div className="set-row">
      <span className="set-label"><label htmlFor={id}>{label}</label>{info && <Info text={info} />}</span>
      <input id={id} type="checkbox" checked={on} onChange={(event) => set(event.target.checked)} />
    </div>
  );
}

function Info({ text }: { text: string }) {
  const ref = useRef<HTMLButtonElement>(null);
  const [tip, setTip] = useState<{ x: number; y: number; below: boolean } | null>(null);

  function show() {
    const rect = ref.current?.getBoundingClientRect();
    if (!rect) return;
    const below = rect.top < 88;
    setTip({
      x: Math.max(8, Math.min(rect.left, window.innerWidth - 276)),
      y: below ? rect.bottom + 6 : rect.top - 6,
      below,
    });
  }

  return (
    <>
      <button
        ref={ref}
        type="button"
        className="set-info"
        aria-label={text}
        onMouseEnter={show}
        onMouseLeave={() => setTip(null)}
        onFocus={show}
        onBlur={() => setTip(null)}
      >
        i
      </button>
      {tip && (
        <span className={tip.below ? "set-info-tip below" : "set-info-tip"} style={{ left: tip.x, top: tip.y }} role="tooltip">
          {text}
        </span>
      )}
    </>
  );
}

function Segment<T extends string>({ value, options, set }: { value: T; options: Array<[T, string]>; set: (value: T) => void }) {
  return (
    <div className="segment">
      {options.map(([id, label]) => (
        <button key={id} type="button" className={value === id ? "on" : ""} onClick={() => set(id)}>{label}</button>
      ))}
    </div>
  );
}

function subtitle(item: Preset): string {
  const lossy = item.format === "jpeg" || item.format === "webp" || item.format === "heic";
  const parts = [FORMAT_LABEL[item.format] + (lossy ? ` ${Math.round(item.quality * 100)}` : "")];
  if (!item.folder) parts.push("no file");
  else parts.push(item.scale === "points" ? "@1x" : "@2x");
  if (item.folder) {
    const name = item.folder.split("/").filter(Boolean).pop();
    if (name) parts.push(name === "Documents" || name === "Screenshots" ? `${name} folder` : name);
  }
  if (item.includeCursor) parts.push("cursor");
  if (item.effects.includes("border")) parts.push("border");
  return parts.slice(0, 4).join(" · ");
}

function ShortcutField({ value, taken, onChange, onError }: {
  value: string;
  taken: (next: string) => string | null;
  onChange: (next: string) => void | Promise<unknown>;
  onError: (message: string) => void;
}) {
  const [recording, setRecording] = useState(false);
  const recordingRef = useRef(false);
  const readyRef = useRef(false);
  const buttonRef = useRef<HTMLButtonElement>(null);
  const disarmRef = useRef<() => void>(() => {});
  const commitRef = useRef<(next: string) => void>(() => {});
  const mac = isMac();

  useEffect(() => {
    if (!recording) return;
    const onKey = (event: KeyboardEvent) => {
      if (!recordingRef.current) return;
      event.preventDefault();
      event.stopPropagation();
      if (event.key === "Escape") {
        disarmRef.current();
        return;
      }
      if (!readyRef.current) return;
      if ((event.key === "Backspace" || event.key === "Delete") && !event.metaKey && !event.ctrlKey && !event.altKey && !event.shiftKey) {
        commitRef.current("");
        return;
      }
      const next = fromKeyEvent(event);
      if (next) commitRef.current(next);
    };
    const onPointer = (event: PointerEvent) => {
      if (!buttonRef.current?.contains(event.target as Node)) disarm();
    };
    window.addEventListener("keydown", onKey, true);
    window.addEventListener("pointerdown", onPointer, true);
    return () => {
      window.removeEventListener("keydown", onKey, true);
      window.removeEventListener("pointerdown", onPointer, true);
    };
  }, [recording]);

  useEffect(() => {
    return () => {
      if (recordingRef.current) {
        recordingRef.current = false;
        void api.releaseShortcuts().catch(() => undefined);
      }
    };
  }, []);

  function arm() {
    recordingRef.current = true;
    readyRef.current = false;
    setRecording(true);
    void api.holdShortcuts().then(() => {
      if (!recordingRef.current) return;
      readyRef.current = true;
      buttonRef.current?.focus();
    }).catch((err: Error) => {
      recordingRef.current = false;
      readyRef.current = false;
      setRecording(false);
      onError(err.message);
    });
  }

  function disarm() {
    if (!recordingRef.current) return;
    recordingRef.current = false;
    readyRef.current = false;
    setRecording(false);
    void api.releaseShortcuts().catch((err: Error) => onError(err.message));
  }

  async function commit(next: string) {
    const message = taken(next);
    if (message) {
      onError(message);
      return;
    }
    onError("");
    recordingRef.current = false;
    readyRef.current = false;
    setRecording(false);
    await onChange(next);
    await api.releaseShortcuts().catch((err: Error) => onError(err.message));
  }

  disarmRef.current = disarm;
  commitRef.current = commit;

  return (
    <button
      ref={buttonRef}
      type="button"
      className={recording ? "key-edit on" : "key-edit"}
      onClick={() => { if (!recording) arm(); }}
    >
      {recording ? "Press shortcut" : <Keycaps raw={value} mac={mac} />}
    </button>
  );
}

function Keycaps({ raw, mac }: { raw: string; mac: boolean }) {
  const { mods, key } = splitShortcut(raw);
  if (!key) return <span className="keys-empty">Not set</span>;
  return (
    <span className="keys-row">
      {mods.map((mod) => <Modifier key={mod} name={mod} mac={mac} />)}
      <span className="keycap">{keyLabel(key)}</span>
    </span>
  );
}

function Modifier({ name, mac }: { name: string; mac: boolean }) {
  if (mac) {
    const symbol = { control: "⌃", alt: "⌥", shift: "⇧", super: "⌘" }[name] ?? name;
    const label = { control: "Control", alt: "Option", shift: "Shift", super: "Command" }[name] ?? name;
    return <span className="keycap symbol" aria-label={label}>{symbol}</span>;
  }
  if (name === "super") {
    return (
      <span className="keycap symbol" aria-label="Windows">
        <svg width="11" height="11" viewBox="0 0 11 11" aria-hidden="true">
          <path fill="currentColor" d="M0 1.4 4.6.8v4.1H0zM5.2.7 11 0v4.9H5.2zM0 6.1h4.6v4.1L0 9.6zM5.2 6.1H11V11l-5.8-.8z" />
        </svg>
      </span>
    );
  }
  const word = { control: "Ctrl", alt: "Alt", shift: "Shift" }[name] ?? name;
  return <span className="keycap">{word}</span>;
}

function shortcutConflict(next: string, self: string, settings: SettingsModel, presets: Preset[]): string | null {
  const canon = canonicalShortcut(next);
  if (!canon) return null;
  for (const [key, value] of Object.entries(settings.shortcuts)) {
    if (key !== self && canonicalShortcut(value) === canon) return `${shortcutName(key)} already uses that shortcut.`;
  }
  for (const preset of presets) {
    if (preset.id !== self.replace(/^preset:/, "") && preset.shortcut && canonicalShortcut(preset.shortcut) === canon) {
      return `${preset.name} already uses that shortcut.`;
    }
  }
  return null;
}

function shortcutName(key: string): string {
  if (key === "repeatLast") return "Repeat last region";
  if (key === "fullScreen") return "Full screen";
  return key.charAt(0).toUpperCase() + key.slice(1);
}

function shortcutInfo(key: string): string | undefined {
  if (key === "region") return "Drag to choose the area of the screen.";
  if (key === "repeatLast") return "Captures the same rectangle as your last region shot.";
  if (key === "window") return "Captures one window. You pick it after the shortcut.";
  if (key === "fullScreen") return "Captures the whole display.";
  if (key === "history") return "Opens the captures you have kept.";
  return undefined;
}

function toggle<T extends string>(list: T[], value: T): T[] {
  return list.includes(value) ? list.filter((item) => item !== value) : [...list, value];
}

function moveSlot(settings: SettingsModel, index: number, delta: number, save: (next: SettingsModel) => void) {
  const next = index + delta;
  if (next < 0 || next >= settings.donutSlots.length) return;
  const donutSlots = settings.donutSlots.slice();
  const [item] = donutSlots.splice(index, 1);
  donutSlots.splice(next, 0, item);
  save({ ...settings, donutSlots });
}
