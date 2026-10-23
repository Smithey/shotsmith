import { useEffect, useRef, useState } from "react";
import { api } from "../api";

const SHORTCUTS = [
  ["Region", "⌘⇧2"],
  ["Repeat last region", "⌘⇧1"],
  ["Window", "⌥⌘⇧2"],
  ["Full screen", "⌃⌘⇧2"],
];

const AFTER = [
  ["donut", "Show the donut", "The action wheel opens at the cursor."],
  ["copy", "Copy", "The capture goes straight to the clipboard."],
  ["save", "Save", "Save with the default preset."],
  ["editor", "Open the editor", "Annotate before you copy or save."],
];

function Permit({ title, detail, on, busy, onAllow }: { title: string; detail: string; on: boolean; busy: boolean; onAllow: () => void }) {
  return (
    <li>
      <div>
        <strong>{title}</strong>
        <span className={on ? "permit-on" : ""}>{detail}</span>
      </div>
      <button type="button" className={on ? "secondary" : "primary"} disabled={on || busy} onClick={onAllow}>
        {on ? "On" : busy ? "Opening…" : "Allow"}
      </button>
    </li>
  );
}

export function Onboarding() {
  const [step, setStep] = useState(0);
  const [screen, setScreen] = useState<boolean | null>(null);
  const [access, setAccess] = useState<boolean | null>(null);
  const [after, setAfter] = useState("donut");
  const [busy, setBusy] = useState("");
  const [error, setError] = useState("");
  const hold = useRef(false);

  useEffect(() => {
    if (step !== 0) return;
    let stop = false;
    const tick = () => {
      api.permission().then((status) => {
        if (stop) return;
        setScreen(status.screenRecording === true);
        setAccess(status.accessibility === true);
      }).catch(() => {
        if (stop) return;
        setScreen(false);
        setAccess(false);
      });
    };
    tick();
    const timer = window.setInterval(tick, 1000);
    return () => {
      stop = true;
      window.clearInterval(timer);
    };
  }, [step]);

  useEffect(() => {
    if (screen === false || access === false) hold.current = false;
  }, [screen, access]);

  useEffect(() => {
    if (hold.current || step !== 0 || screen !== true || access !== true) return;
    const timer = window.setTimeout(() => setStep(1), 700);
    return () => window.clearTimeout(timer);
  }, [step, screen, access]);

  const ready = screen === true && access === true;

  async function allow(kind: "screen" | "accessibility") {
    setBusy(kind);
    setError("");
    try {
      if (kind === "screen") await api.openScreenSettings();
      else await api.openAccessibilitySettings();
    } catch (err) {
      setError(err instanceof Error ? err.message : "System Settings did not open.");
    } finally {
      setBusy("");
    }
  }

  return (
    <div className="pane sheet">
      <div className="steps">{[0, 1, 2].map((item) => <span key={item} className={item <= step ? "on" : ""} />)}</div>
      {step === 0 && (
        <>
          <p className="eyebrow">Step 1 of 3</p>
          <h1>Allow Shotsmith</h1>
          <p>Capture needs Screen Recording. Global shortcuts need Accessibility. This step continues once both are on.</p>
          <ul className="permits">
            <Permit
              title="Screen Recording"
              detail={screen === true ? "On" : screen === false ? "Off" : "Checking…"}
              on={screen === true}
              busy={busy === "screen"}
              onAllow={() => allow("screen")}
            />
            <Permit
              title="Accessibility"
              detail={access === true ? "On" : access === false ? "Off" : "Checking…"}
              on={access === true}
              busy={busy === "accessibility"}
              onAllow={() => allow("accessibility")}
            />
          </ul>
          <p className={ready ? "status ok" : "status"}>
            {ready ? "Both are on. Continuing…" : "Turn both on to continue."}
          </p>
          {error && <p className="error">{error}</p>}
          {screen === false && <button className="secondary" onClick={() => api.relaunch()}>Relaunch Shotsmith</button>}
        </>
      )}
      {step === 1 && (
        <>
          <p className="eyebrow">Step 2 of 3</p>
          <h1>Shortcuts</h1>
          <p>These are on by default. Change any of them later in Settings.</p>
          <dl className="keys">
            {SHORTCUTS.map(([name, keys]) => (
              <div key={name}>
                <dt>{name}</dt>
                <dd>{keys}</dd>
              </div>
            ))}
          </dl>
          <p>If nothing happens, macOS is still using those keys for its own screenshots. Turn those off under Keyboard Shortcuts, Screenshots.</p>
          <button className="secondary" onClick={() => api.openKeyboardSettings()}>Open Keyboard settings</button>
        </>
      )}
      {step === 2 && (
        <>
          <p className="eyebrow">Step 3 of 3</p>
          <h1>After capture</h1>
          <p>Release the mouse to capture a region. Click a window to capture that window.</p>
          <div className="choices">
            {AFTER.map(([id, title, detail]) => (
              <button key={id} className={after === id ? "choice on" : "choice"} onClick={() => setAfter(id)}>
                <strong>{title}</strong>
                <span>{detail}</span>
              </button>
            ))}
          </div>
        </>
      )}
      <div className="row" style={{ marginTop: 18 }}>
        {step > 0 && <button className="secondary" onClick={() => { hold.current = true; setStep(step - 1); }}>Back</button>}
        {step < 2 && (
          <button className="primary" disabled={step === 0 && !ready} onClick={() => setStep(step + 1)}>
            Continue
          </button>
        )}
        {step === 2 && <button className="primary" onClick={() => api.finishOnboarding(after)}>Start capturing</button>}
      </div>
    </div>
  );
}
