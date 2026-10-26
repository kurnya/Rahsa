import { useState } from "react";
import { save } from "@tauri-apps/plugin-dialog";
import { api, errorText } from "../api";
import type { Notify } from "../App";
import { useT } from "../i18n";
import { Mark } from "../components/icons";
import { PasswordInput, StrengthMeter } from "../components/ui";
import { RestoreDialog } from "../components/RestoreDialog";

export function Setup({ onDone, notify }: { onDone: () => void; notify: Notify }) {
  const t = useT();
  const [pw, setPw] = useState("");
  const [confirm, setConfirm] = useState("");
  const [ack, setAck] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [step, setStep] = useState<"create" | "kit">("create");
  const [restoring, setRestoring] = useState(false);

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    if ([...pw].length < 12) return setError(t.tooShort);
    if (pw !== confirm) return setError(t.mismatch);
    setBusy(true);
    setError(null);
    try {
      await api.createVault(pw);
      setPw("");
      setConfirm("");
      setStep("kit");
    } catch (err) {
      setError(errorText(err));
    } finally {
      setBusy(false);
    }
  };

  const saveKit = async () => {
    const path = await save({
      defaultPath: "Rahsa-Emergency-Kit.html",
      filters: [{ name: "HTML", extensions: ["html"] }],
    });
    if (!path) return;
    try {
      await api.saveEmergencyKit(path);
      notify({ kind: "success", message: t.kitSaved, action: null });
      onDone();
    } catch (err) {
      notify({ kind: "error", message: errorText(err), action: null });
    }
  };

  if (step === "kit") {
    return (
      <main className="gate">
        <div className="gate-card">
          <Mark />
          <h1>{t.kitTitle}</h1>
          <p className="lead">{t.kitLead}</p>
          <div className="row">
            <button className="btn primary" onClick={saveKit}>{t.kitSave}</button>
            <button className="btn ghost" onClick={onDone}>{t.kitSkip}</button>
          </div>
        </div>
      </main>
    );
  }

  return (
    <main className="gate">
      <form className="gate-card" onSubmit={submit}>
        <Mark />
        <h1>{t.setupTitle}</h1>
        <p className="lead">{t.setupLead}</p>

        <label className="field">
          <span>{t.masterPassword}</span>
          <PasswordInput value={pw} onChange={setPw} autoFocus autoComplete="new-password" />
          <StrengthMeter password={pw} />
          <span className="hint">{t.masterHint}</span>
        </label>
        <label className="field">
          <span>{t.masterConfirm}</span>
          <PasswordInput value={confirm} onChange={setConfirm} autoComplete="new-password" />
        </label>

        <div className="warn-box">{t.masterWarning}</div>
        <label className="check">
          <input type="checkbox" checked={ack} onChange={(e) => setAck(e.target.checked)} />
          <span>{t.understand}</span>
        </label>

        {error && <p className="error" role="alert">{error}</p>}

        <div className="row">
          <button className="btn primary" type="submit" disabled={!ack || busy || !pw}>
            {busy ? t.creating : t.createVault}
          </button>
          <button type="button" className="btn ghost" onClick={() => setRestoring(true)}>
            {t.restoreInstead}
          </button>
        </div>
      </form>
      {restoring && <RestoreDialog onClose={() => setRestoring(false)} onDone={onDone} notify={notify} />}
    </main>
  );
}
