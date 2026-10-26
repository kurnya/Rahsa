import { useState } from "react";
import { api, errorText, originLabel, type Institution, type OriginInput } from "../api";
import type { Notify } from "../App";
import { useT } from "../i18n";
import { Plus, Trash } from "./icons";
import { Dialog } from "./ui";

export function InstitutionForm(props: {
  institution: Institution | null;
  notify: Notify;
  onClose: () => void;
  onSaved: (id: string | null) => void;
}) {
  const t = useT();
  const inst = props.institution;
  const [name, setName] = useState(inst?.name ?? "");
  const [loginUrl, setLoginUrl] = useState(inst?.login_url ?? "https://");
  // The first origin is derived from the login URL; the rest are extras.
  const [extra, setExtra] = useState<OriginInput[]>(
    inst?.origins.slice(1).map((o) => ({ url: `https://${originLabel(o)}`, match_mode: o.match })) ?? [],
  );
  const [incognito, setIncognito] = useState(inst?.open_incognito ?? false);
  const [fresh, setFresh] = useState(inst?.fresh_session ?? true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [confirmDelete, setConfirmDelete] = useState(false);

  const derivedOrigin = (() => {
    try {
      const u = new URL(loginUrl);
      return u.protocol === "https:" ? u.origin : null;
    } catch {
      return null;
    }
  })();

  const submit = async (e?: React.FormEvent) => {
    e?.preventDefault();
    setBusy(true);
    setError(null);
    try {
      const id = await api.saveInstitution({
        id: inst?.id ?? null,
        name,
        login_url: loginUrl,
        extra_origins: extra.filter((o) => o.url.trim() && o.url !== "https://"),
        icon: inst?.icon ?? null,
        open_incognito: incognito,
        fresh_session: fresh,
      });
      props.notify({ kind: "success", message: t.saved, action: null });
      props.onSaved(id);
    } catch (err) {
      setError(errorText(err));
    } finally {
      setBusy(false);
    }
  };

  const remove = async () => {
    if (!inst) return;
    try {
      await api.deleteInstitution(inst.id);
      props.onSaved(null);
    } catch (err) {
      setError(errorText(err));
    }
  };

  return (
    <Dialog
      title={inst ? `${t.edit}: ${inst.name}` : t.addInstitution}
      onClose={props.onClose}
      footer={
        <>
          {inst && (
            <button className="btn ghost danger" style={{ marginRight: "auto" }} onClick={() => setConfirmDelete(true)}>
              {t.deleteInstitution}
            </button>
          )}
          <button className="btn" onClick={props.onClose}>{t.cancel}</button>
          <button className="btn primary" onClick={() => submit()} disabled={busy}>
            {busy ? t.saving : t.save}
          </button>
        </>
      }
    >
      <form onSubmit={submit}>
        <label className="field">
          <span>{t.instName}</span>
          <input className="input" value={name} onChange={(e) => setName(e.target.value)} placeholder={t.instNamePlaceholder} />
        </label>
        <label className="field">
          <span>{t.loginUrl}</span>
          <input
            className="input mono"
            value={loginUrl}
            onChange={(e) => setLoginUrl(e.target.value)}
            placeholder="https://siakad.kampus.ac.id/login"
            spellCheck={false}
            aria-invalid={!derivedOrigin}
          />
          {derivedOrigin ? (
            <span className="hint">{t.origins}: <span className="mono">{derivedOrigin}</span></span>
          ) : (
            loginUrl.length > 8 && <span className="error">{t.notHttps}</span>
          )}
        </label>

        <div className="field">
          <span>{t.extraOrigins}</span>
          <span className="hint">{t.extraOriginsHint}</span>
          {extra.map((o, i) => (
            <div className="row" key={i}>
              <input
                className="input grow mono"
                value={o.url}
                onChange={(e) => setExtra((xs) => xs.map((x, j) => (j === i ? { ...x, url: e.target.value } : x)))}
              />
              <label className="check" style={{ margin: 0 }}>
                <input
                  type="checkbox"
                  checked={o.match_mode === "subdomain"}
                  onChange={(e) =>
                    setExtra((xs) => xs.map((x, j) => (j === i ? { ...x, match_mode: e.target.checked ? "subdomain" : "exact" } : x)))
                  }
                />
                <span className="small">{t.subdomain}</span>
              </label>
              <button type="button" className="btn ghost" aria-label={t.delete} onClick={() => setExtra((xs) => xs.filter((_, j) => j !== i))}>
                <Trash size={14} />
              </button>
            </div>
          ))}
          <div>
            <button type="button" className="btn small" onClick={() => setExtra((xs) => [...xs, { url: "https://", match_mode: "exact" }])}>
              <Plus size={14} /> {t.addOrigin}
            </button>
          </div>
        </div>

        <label className="check">
          <input type="checkbox" checked={fresh} onChange={(e) => setFresh(e.target.checked)} />
          <span>
            {t.freshSession}
            <br />
            <span className="muted small">{t.freshSessionHint}</span>
          </span>
        </label>
        <label className="check">
          <input type="checkbox" checked={incognito} onChange={(e) => setIncognito(e.target.checked)} />
          <span>{t.incognitoAlways}</span>
        </label>

        {error && <p className="error" role="alert">{error}</p>}
        <button type="submit" hidden />
      </form>

      {confirmDelete && (
        <Dialog
          title={t.deleteInstitution}
          onClose={() => setConfirmDelete(false)}
          footer={
            <>
              <button className="btn" onClick={() => setConfirmDelete(false)}>{t.cancel}</button>
              <button className="btn primary" onClick={remove}>{t.delete}</button>
            </>
          }
        >
          <p>{t.deleteInstitutionConfirm}</p>
        </Dialog>
      )}
    </Dialog>
  );
}
