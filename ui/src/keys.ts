const MODS = ["control", "alt", "shift", "super"] as const;
const MOD_NAMES = new Set(["control", "ctrl", "alt", "option", "shift", "super", "cmd", "command", "meta", "win"]);

export type ShortcutParts = { mods: string[]; key: string };

export function isMac(): boolean {
  return /Mac|iPhone|iPad/.test(navigator.platform);
}

export function splitShortcut(raw: string | undefined): ShortcutParts {
  const parts = (raw ?? "").split("+").map((part) => part.trim().toLowerCase()).filter(Boolean);
  const present = new Set(parts);
  const mods = MODS.filter((name) => {
    if (present.has(name)) return true;
    if (name === "alt" && present.has("option")) return true;
    if (name === "control" && present.has("ctrl")) return true;
    if (name === "super" && parts.some((part) => ["cmd", "command", "meta", "win"].includes(part))) return true;
    return false;
  });
  const key = parts.find((part) => !MOD_NAMES.has(part)) ?? "";
  return { mods, key };
}

export function canonicalShortcut(raw: string | undefined): string {
  const { mods, key } = splitShortcut(raw);
  if (!key) return "";
  return [...mods, key].join("+");
}

export function fromKeyEvent(event: KeyboardEvent): string | null {
  if (["Meta", "Shift", "Alt", "Control"].includes(event.key)) return null;
  const key = codeToken(event.code);
  if (!key) return null;
  const mods: string[] = [];
  if (event.ctrlKey) mods.push("control");
  if (event.altKey) mods.push("alt");
  if (event.shiftKey) mods.push("shift");
  if (event.metaKey) mods.push("super");
  return [...mods, key].join("+");
}

export function keyLabel(token: string): string {
  if (token.startsWith("digit")) return token.slice(5);
  if (token.startsWith("key") && token.length === 4) return token.slice(3).toUpperCase();
  if (token.startsWith("f") && /^f\d+$/.test(token)) return token.toUpperCase();
  if (token.startsWith("arrow")) {
    return { up: "↑", down: "↓", left: "←", right: "→" }[token.slice(5)] ?? token;
  }
  if (token.startsWith("numpad")) return `Num ${token.slice(6).toUpperCase()}`;
  const names: Record<string, string> = {
    space: "Space",
    tab: "Tab",
    enter: "↩",
    minus: "−",
    equal: "=",
    comma: ",",
    period: ".",
    slash: "/",
    semicolon: ";",
    quote: "'",
    backquote: "`",
    backslash: "\\",
    bracketleft: "[",
    bracketright: "]",
    backspace: "⌫",
    delete: "⌦",
    escape: "Esc",
    pageup: "PgUp",
    pagedown: "PgDn",
    home: "Home",
    end: "End",
    insert: "Ins",
    printscreen: "PrtSc",
  };
  return names[token] ?? token.toUpperCase();
}

export function menuHint(raw: string | undefined, mac = isMac()): string {
  const { mods, key } = splitShortcut(raw);
  if (!key) return "";
  if (mac) {
    const symbols: Record<string, string> = { control: "⌃", alt: "⌥", shift: "⇧", super: "⌘" };
    return `${mods.map((name) => symbols[name] ?? "").join("")}${keyLabel(key)}`;
  }
  const words: Record<string, string> = { control: "Ctrl", alt: "Alt", shift: "Shift", super: "Win" };
  return [...mods.map((name) => words[name] ?? name), keyLabel(key)].join("+");
}

function codeToken(code: string): string | null {
  if (code.startsWith("Key") && code.length === 4) return `key${code.slice(3).toLowerCase()}`;
  if (code.startsWith("Digit")) return `digit${code.slice(5)}`;
  if (code.startsWith("Numpad")) return `numpad${code.slice(6).toLowerCase()}`;
  if (code.startsWith("Arrow")) return `arrow${code.slice(5).toLowerCase()}`;
  if (/^F\d+$/.test(code)) return code.toLowerCase();
  const named: Record<string, string> = {
    Backquote: "backquote",
    Backslash: "backslash",
    BracketLeft: "bracketleft",
    BracketRight: "bracketright",
    Comma: "comma",
    Equal: "equal",
    Minus: "minus",
    Period: "period",
    Quote: "quote",
    Semicolon: "semicolon",
    Slash: "slash",
    Space: "space",
    Tab: "tab",
    Enter: "enter",
    Backspace: "backspace",
    Delete: "delete",
    Escape: "escape",
    Home: "home",
    End: "end",
    PageUp: "pageup",
    PageDown: "pagedown",
    Insert: "insert",
    PrintScreen: "printscreen",
  };
  return named[code] ?? null;
}
