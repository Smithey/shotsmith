import { convertFileSrc, invoke } from "@tauri-apps/api/core";
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
};

export type DonutState = {
  captureId: string;
  width: number;
  height: number;
  imagePath: string;
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
};

export const SWATCHES = ["#FF3B30", "#FF9500", "#FFCC00", "#34C759", "#007AFF", "#AF52DE", "#1D1D1F"];

export function assetUrl(path: string): string {
  if (!path) return "";
  return convertFileSrc(path);
}

export function viewName(): string {
  return new URLSearchParams(window.location.search).get("view") ?? "settings";
}

export function queryId(): string {
  return new URLSearchParams(window.location.search).get("id") ?? "";
}

export async function onRefresh(run: () => void): Promise<() => void> {
  const unlisten = await listen("refresh", () => run());
  return unlisten;
}

export const api = {
  settings: () => invoke<Settings>("settings_get"),
  saveSettings: (settings: Settings) => invoke<void>("settings_set", { settings }),
  presets: () => invoke<Preset[]>("presets_list"),
  savePreset: (preset: Preset) => invoke<Preset[]>("presets_save", { preset }),
  deletePreset: (id: string) => invoke<Preset[]>("presets_delete", { id }),
  importPresets: (json: string) => invoke<Preset[]>("presets_import", { json }),
  exportPresets: () => invoke<string>("presets_export"),
  filenamePreview: (pattern: string, width: number, height: number) =>
    invoke<string>("filename_preview", { pattern, width, height }),
  permission: () => invoke<{ state: string; reason?: string }>("permission_status"),
  platform: () => invoke<string>("platform_name"),
  windows: () => invoke<WindowInfo[]>("capture_windows"),
  overlays: () => invoke<DisplayInfo[]>("overlay_state"),
  begin: (mode: string) => invoke<void>("capture_begin", { mode }),
  cancel: () => invoke<void>("capture_cancel"),
  confirm: (args: Record<string, unknown>) => invoke<string>("capture_confirm", args),
  repeat: () => invoke<string>("capture_repeat"),
  countdown: () => invoke<void>("capture_countdown"),
  countdownDone: () => invoke<void>("capture_countdown_done"),
  history: (query: string) => invoke<HistoryItem[]>("history_list", { query }),
  deleteHistory: (id: string) => invoke<void>("history_delete", { id }),
  openEditor: (id: string) => invoke<void>("editor_open", { id }),
  editor: () => invoke<EditorState>("editor_state"),
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
  finishOnboarding: (afterCapture: string) => invoke<void>("finish_onboarding", { afterCapture }),
  openImage: () => invoke<void>("open_image"),
  editClipboard: () => invoke<void>("edit_clipboard"),
  pickFolder: () => invoke<string | null>("pick_folder"),
  pickSave: (suggested: string) => invoke<string | null>("pick_save", { suggested }),
  copyHex: (text: string) => invoke<void>("copy_hex", { text }),
  quit: () => invoke<void>("quit"),
};

export function currentWindow() {
  return getCurrentWindow();
}

export function uid(): string {
  return crypto.randomUUID();
}
