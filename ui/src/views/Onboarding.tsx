import { useEffect, useState } from "react";
import { api } from "../api";

export function Onboarding() {
  const [step, setStep] = useState(0);
  const [permission, setPermission] = useState("checking");
  const [reason, setReason] = useState("");
  const [after, setAfter] = useState("donut");

  useEffect(() => {
    api.permission().then((status) => {
      setPermission(status.state);
      setReason(status.reason ?? "");
    });
  }, []);

  return (
    <div className="pane sheet">
      <div className="steps">{[0, 1, 2].map((item) => <span key={item} className={item <= step ? "on" : ""} />)}</div>
      {step === 0 && (
        <>
          <h1>Allow screen capture</h1>
          <p>Shotsmith freezes the display when you press the shortcut, then waits for a region, window, or full screen.</p>
          <p>Status: {permission === "granted" ? "Capture is available." : `Blocked. ${reason}`}</p>
          <p>On a Mac, grant Screen Recording in System Settings, Privacy & Security, Screen & System Audio Recording.</p>
        </>
      )}
      {step === 1 && (
        <>
          <h1>Shortcuts</h1>
          <p>Region is Ctrl+Shift+2 on Windows and Linux, and Command+Shift+2 on Mac. You can change every shortcut in Settings.</p>
          <p>If the system screenshot shortcuts are in the way, turn them off in the operating system keyboard settings.</p>
        </>
      )}
      {step === 2 && (
        <>
          <h1>After capture</h1>
          <label>What should happen when you confirm?</label>
          <select className="field" value={after} onChange={(event) => setAfter(event.target.value)}>
            <option value="donut">Show the donut</option>
            <option value="copy">Copy to the clipboard</option>
            <option value="save">Save with the default preset</option>
            <option value="editor">Open the editor</option>
          </select>
        </>
      )}
      <div className="row" style={{ marginTop: 18 }}>
        {step < 2 && <button className="split" onClick={() => setStep(step + 1)}><span style={{ padding: "8px 14px" }}>Continue</span></button>}
        {step === 2 && <button className="split" onClick={() => api.finishOnboarding(after)}><span style={{ padding: "8px 14px" }}>Start capturing</span></button>}
      </div>
    </div>
  );
}
