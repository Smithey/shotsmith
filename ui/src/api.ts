import { convertFileSrc, invoke as tauriInvoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { Document } from "./bindings/Document";
import type { Preset } from "./bindings/Preset";

export type { Document, Preset };
export type AnnotationObject = Document["objects"][number];

export type Settings = {
  launchAtLogin: boolean;
  showDockIcon: boolean;
  sound: boolean;
  hudEnabled: boolean;
  appearance: string;
  loupe: boolean;
  overlayToolbar: boolean;
  includeCursor: boolean;
  afterCapture: string;
  defaultPresetId: string;
  shortcuts: {
    region: string;
    repeatLast: string;
    window: string;
    fullScreen: string;
    history: string;
  };
  donutSlots: string[];
  donutCount: number;
  historyLimit: number;
  historyDays: number;
  keepHistory: boolean;
  pinsPersist: boolean;
  onboardingDone: boolean;
  timedSeconds: number;
  timedMode: string;
  filenameCounter: number;
  warnOnSecrets: boolean;
  watchFolders: string[];
  recentApps: string[];
  checkUpdates: boolean;
  pluginsEnabled: boolean;
  locale: string;
};

export type DisplayInfo = {
  id: number;
  name: string;
  x: number;
  y: number;
  width: number;
  height: number;
  scale: number;
  imagePath: string;
};

export type WindowInfo = {
  id: number;
  appName: string;
  title: string;
  x: number;
  y: number;
  width: number;
  height: number;
};

export type HistoryItem = {
  id: string;
  date: string;
  mode: string;
  sourceApp: string | null;
  windowTitle: string | null;
  pixelWidth: number;
  pixelHeight: number;
  scale: number;
  originalPath: string;
  documentPath: string | null;
  pinned: boolean;
  ocrText?: string | null;
  previewPath?: string | null;
  previewRev?: number;
};

export type DonutState = {
  captureId: string;
  width: number;
  height: number;
  imagePath: string;
  preview: string;
  slots: { id: string; label: string; key: string }[];
  cursorX: number;
  cursorY: number;
};

export type HudState = {
  title: string;
  detail: string;
  imagePath: string;
  captureId: string;
  countdown: number | null;
};

export type EditorState = {
  captureId: string;
  document: Document;
  imagePath: string;
  canUndo: boolean;
  canRedo: boolean;
  dirty: boolean;
  pixelsChanged?: boolean;
};

export const SWATCHES = ["#FF3B30", "#FF9500", "#FFCC00", "#34C759", "#007AFF", "#AF52DE", "#1D1D1F"];

function invoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (typeof window === "undefined" || !("__TAURI_INTERNALS__" in window)) {
    return Promise.reject(new Error("Shotsmith commands are only available in the desktop app."));
  }
  return tauriInvoke<T>(command, args);
}

export function assetUrl(path: string): string {
  if (!path) return "";
  if (typeof window === "undefined" || !("__TAURI_INTERNALS__" in window)) return "";
  return convertFileSrc(path);
}

export function shotUrl(item: { originalPath: string; previewPath?: string | null; previewRev?: number }): string {
  const path = item.previewPath || item.originalPath;
  const url = assetUrl(path);
  return item.previewPath && item.previewRev ? `${url}?v=${item.previewRev}` : url;
}

export function viewName(): string {
  return new URLSearchParams(window.location.search).get("view") ?? "settings";
}

export function queryId(): string {
  return new URLSearchParams(window.location.search).get("id") ?? "";
}

export async function onRefresh(run: (payload?: unknown) => void): Promise<() => void> {
  const unlisten = await listen("refresh", (event) => run(event.payload));
  return unlisten;
}

export async function onHistoryChanged(run: () => void): Promise<() => void> {
  const unlisten = await listen("history-changed", () => run());
  return unlisten;
}

let shortcutTail: Promise<void> = Promise.resolve();
let shortcutIntent: "live" | "held" = "live";
let shortcutApplied: "live" | "held" = "live";

function queueShortcuts(task: () => Promise<void>): Promise<void> {
  const run = shortcutTail.then(task, task);
  shortcutTail = run.then(() => undefined, () => undefined);
  return run;
}

async function applyShortcutIntent(): Promise<void> {
  const want = shortcutIntent;
  if (want === shortcutApplied) return;
  if (want === "held") await invoke<void>("shortcuts_suspend");
  else await invoke<void>("shortcuts_reload");
  shortcutApplied = want;
  if (shortcutIntent !== shortcutApplied) await applyShortcutIntent();
}

export function holdShortcuts(): Promise<void> {
  shortcutIntent = "held";
  return queueShortcuts(applyShortcutIntent);
}

export function releaseShortcuts(): Promise<void> {
  shortcutIntent = "live";
  return queueShortcuts(applyShortcutIntent);
}

export const api = {
  settings: () => invoke<Settings>("settings_get"),
  saveSettings: (settings: Settings) => {
    shortcutIntent = "live";
    return queueShortcuts(async () => {
      await invoke<void>("settings_set", { settings });
      shortcutApplied = "live";
      if (shortcutIntent !== shortcutApplied) await applyShortcutIntent();
    });
  },
  holdShortcuts,
  releaseShortcuts,
  presets: () => invoke<Preset[]>("presets_list"),
  savePreset: (preset: Preset) => invoke<Preset[]>("presets_save", { preset }),
  deletePreset: (id: string) => invoke<Preset[]>("presets_delete", { id }),
  importPresets: (json: string) => invoke<Preset[]>("presets_import", { json }),
  exportPresets: () => invoke<string>("presets_export"),
  filenamePreview: (pattern: string, width: number, height: number) =>
    invoke<string>("filename_preview", { pattern, width, height }),
  permission: () => invoke<{ state: string; reason?: string; screenRecording?: boolean; accessibility?: boolean }>("permission_status"),
  platform: () => invoke<string>("platform_name"),
  windows: () => invoke<WindowInfo[]>("capture_windows"),
  overlays: () => invoke<DisplayInfo[]>("overlay_state"),
  begin: (mode: string) => invoke<void>("capture_begin", { mode }),
  captureMode: () => invoke<string>("capture_mode"),
  openScreenSettings: () => invoke<void>("open_screen_settings"),
  openAccessibilitySettings: () => invoke<void>("open_accessibility_settings"),
  openKeyboardSettings: () => invoke<void>("open_keyboard_settings"),
  relaunch: () => invoke<void>("relaunch"),
  cancel: () => invoke<void>("capture_cancel"),
  confirm: (args: Record<string, unknown>) => invoke<string>("capture_confirm", args),
  repeat: () => invoke<string>("capture_repeat"),
  countdown: () => invoke<void>("capture_countdown"),
  countdownDone: () => invoke<void>("capture_countdown_done"),
  history: (query: string) => invoke<HistoryItem[]>("history_list", { query }),
  deleteHistory: (id: string) => invoke<void>("history_delete", { id }),
  copyHistory: (id: string) => invoke<void>("history_copy", { id }),
  openEditor: (id: string) => invoke<void>("editor_open", { id }),
  editor: () => invoke<EditorState>("editor_state"),
  editorImage: () => invoke<string>("editor_image"),
  commit: (document: Document) => invoke<EditorState>("editor_commit", { document }),
  undo: () => invoke<EditorState>("editor_undo"),
  redo: () => invoke<EditorState>("editor_redo"),
  crop: (x: number, y: number, width: number, height: number) =>
    invoke<EditorState>("editor_crop", { x, y, width, height }),
  copyEditor: () => invoke<void>("editor_copy"),
  saveEditor: (presetId: string | null, saveAs: string | null) =>
    invoke<string>("editor_save", { presetId, saveAs }),
  dirty: () => invoke<boolean>("editor_dirty"),
  donut: () => invoke<DonutState>("donut_state"),
  hud: () => invoke<HudState>("hud_state"),
  donutAct: (action: string, keepOpen: boolean) => invoke<void>("donut_act", { action, keepOpen }),
  pin: (id: string) => invoke<void>("pin_open", { id }),
  closePin: (id: string) => invoke<void>("pin_close", { id }),
  reveal: (id: string) => invoke<void>("reveal_capture", { id }),
  openHistory: () => invoke<void>("open_history"),
  openSettings: () => invoke<void>("open_settings"),
  finishOnboarding: (afterCapture: string, launchAtLogin: boolean) =>
    invoke<void>("finish_onboarding", { afterCapture, launchAtLogin }),
  openImage: () => invoke<void>("open_image"),
  editClipboard: () => invoke<void>("edit_clipboard"),
  pickFolder: () => invoke<string | null>("pick_folder"),
  pickSave: (suggested: string) => invoke<string | null>("pick_save", { suggested }),
  copyHex: (text: string) => invoke<void>("copy_hex", { text }),
  quit: () => invoke<void>("quit"),
  featureNotice: (message: string) => invoke<void>("feature_notice", { message }),
  editorText: () => invoke<TextLine[]>("editor_text"),
  editorSecrets: () => invoke<SecretFinding[]>("editor_secrets"),
  dismissSecret: (findingId: string) => invoke<SecretFinding[]>("editor_dismiss_secret", { findingId }),
  redactSecrets: () => invoke<EditorState>("editor_redact"),
  shareEditor: () => invoke<void>("editor_share"),
  printEditor: (fit: boolean, rotate: boolean) => invoke<void>("editor_print", { fit, rotate }),
  beautify: (options: BeautifyOptions) => invoke<EditorState>("editor_beautify", { options }),
  apps: () => invoke<AppChoice[]>("apps_list"),
  openInState: () => invoke<{ captureId: string; recent: AppChoice[]; apps: AppChoice[] }>("open_in_state"),
  openInChoose: (appId: string, captureId: string | null) => invoke<void>("open_in_choose", { appId, captureId }),
  openInSystem: (captureId: string) => invoke<void>("open_in_system", { captureId }),
  measureAt: (x: number, y: number) => invoke<{ x: number; y: number; w: number; h: number }>("measure_at", { x, y }),
  lookup: (text: string) => invoke<void>("lookup_text", { text }),
  plugins: () => invoke<PluginInfo[]>("plugins_list"),
  runPlugin: (pluginId: string) => invoke<void>("editor_plugin", { pluginId }),
  checkUpdates: () => invoke<string>("updates_check"),
};

export type TextLine = { text: string; x: number; y: number; w: number; h: number };
export type SecretFinding = { id: string; kind: string; text: string; preview: string; x: number; y: number; w: number; h: number };
export type AppChoice = { id: string; name: string };
export type PluginInfo = { id: string; name: string; uploads: string };
export type BeautifyOptions = { scale: number; backdrop: string; padding: number; radius: number; shadow: boolean; frame: boolean; watermark: string };

export function currentWindow() {
  return getCurrentWindow();
}

export function uid(): string {
  return crypto.randomUUID();
}
