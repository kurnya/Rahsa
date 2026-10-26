import { useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { api, errorText } from "../api";
import type { Notify } from "../App";
import { useT } from "../i18n";
import { Dialog, PasswordInput } from "./ui";

export function RestoreDialog(props: { onClose: () => void; onDone: () => void; notify: Notify }) {
  const t = useT();
  const [path, setPath] = useState<string | null>(null);
  const [pw, setPw] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const pick = async () => {
    const p = await open({ multiple: false, filters: [{ name: "Rahsa", extensions: ["rahsa"] }] });
    if (typeof p === "string") setPath(p);
  };

  const submit = async () => {
    if (!path) return;
    setBusy(true);
    setError(null);
    try {
      const n = await api.restoreBackup(path, pw);
      setPw("");
      props.notify({ kind: "success", message: t.restored(n), action: null });
      props.onClose();
      props.onDone();
    } catch (e) {
      setError(errorText(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog
      title={t.restoreBackup}
      onClose={props.onClose}
      footer={
        <>
          <button className="btn" onClick={props.onClose}>{t.cancel}</button>
          <button className="btn primary" disabled={!path || !pw || busy} onClick={submit}>
            {busy ? t.unlocking : t.restore}
          </button>
        </>
      }
    >
      <p className="warn-box">{t.restoreWarning}</p>
      <div className="field">
        <span>File</span>
        <div className="row">
          <span className="grow small mono" style={{ overflowWrap: "anywhere" }}>{path ?? "—"}</span>
          <button className="btn small" onClick={pick}>Pilih file…</button>
        </div>
      </div>
      <label className="field">
        <span>{t.masterPassword}</span>
        <PasswordInput value={pw} onChange={setPw} />
      </label>
      {error && <p className="error" role="alert">{error}</p>}
    </Dialog>
  );
}
