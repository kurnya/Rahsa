import { useEffect, useState } from "react";
import { api, errorText, type AppStatus } from "../api";
import type { Notify } from "../App";
import { useT } from "../i18n";
import { Mark } from "../components/icons";
import { PasswordInput } from "../components/ui";
import { RestoreDialog } from "../components/RestoreDialog";

export function Unlock({ status, onDone, notify }: { status: AppStatus; onDone: () => void; notify: Notify }) {
  const t = useT();
  const [pw, setPw] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [wait, setWait] = useState(status.retry_after_secs);
  const [restoring, setRestoring] = useState(false);

  useEffect(() => {
    if (wait <= 0) return;
    const id = setTimeout(() => setWait((w) => w - 1), 1000);
    return () => clearTimeout(id);
  }, [wait]);

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!pw || busy || wait > 0) return;
    setBusy(true);
    setError(null);
    try {
      await api.unlock(pw);
      setPw("");
      onDone();
    } catch (err) {
      setError(errorText(err));
      const s = await api.status();
      setWait(s.retry_after_secs);
    } finally {
      setBusy(false);
    }
  };

  return (
    <main className="gate">
      <form className="gate-card" onSubmit={submit}>
        <Mark />
        <h1>{t.unlockTitle}</h1>
        <p className="lead">{t.unlockLead}</p>
        <label className="field">
          <span>{t.masterPassword}</span>
          <PasswordInput value={pw} onChange={setPw} autoFocus autoComplete="current-password" />
        </label>
        {error && <p className="error" role="alert">{error}</p>}
        {wait > 0 && <p className="muted small">{t.waitSeconds(wait)}</p>}
        <div className="row">
          <button className="btn primary" type="submit" disabled={busy || !pw || wait > 0}>
            {busy ? t.unlocking : t.unlock}
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
