import { useEffect, useState } from "react";
import { api, errorText, originLabel, type AccountMeta, type CustomFieldInput, type Institution, type OriginInput } from "../api";
import type { Notify } from "../App";
import { useT } from "../i18n";
import { Dice, Plus, Trash } from "./icons";
import { Dialog, formatDate, PasswordInput } from "./ui";
import { GeneratorDialog } from "./GeneratorDialog";

export function AccountForm(props: {
  account: AccountMeta | null;
  institutionId: string | null;
  institutions: Institution[];
  notify: Notify;
  onClose: () => void;
  onSaved: (id: string) => void;
}) {
  const t = useT();
  const a = props.account;
  const [institutionId, setInstitutionId] = useState<string | null>(props.institutionId ?? props.institutions[0]?.id ?? null);
  const [label, setLabel] = useState(a?.label ?? "");
  const [username, setUsername] = useState(a?.username ?? "");
  const [password, setPassword] = useState("");
  const [original, setOriginal] = useState<string | null>(null);
  const [history, setHistory] = useState<{ password: string; changed_at: number }[] | null>(null);

  // Editing: load the current password (shown masked) so it can be revealed or copied here.
  useEffect(() => {
    if (!a) return;
    api.revealPassword(a.id).then((p) => {
      setPassword(p);
      setOriginal(p);
    });
  }, [a]);
  const [origins, setOrigins] = useState<OriginInput[]>(
    a?.origins.map((o) => ({ url: `https://${originLabel(o)}`, match_mode: o.match })) ?? [],
  );
  const [notes, setNotes] = useState(a?.notes ?? "");
  const [fields, setFields] = useState<CustomFieldInput[]>(
    a?.custom_fields.map((f) => ({ name: f.name, value: f.value, selector: f.selector, secret: f.secret })) ?? [],
  );
  const [tags, setTags] = useState(a?.tags.join(", ") ?? "");
  const [autoSubmit, setAutoSubmit] = useState(a?.auto_submit ?? false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [gen, setGen] = useState(false);

  const submit = async (e?: React.FormEvent) => {
    e?.preventDefault();
    setBusy(true);
    setError(null);
    try {
      const id = await api.saveAccount({
        id: a?.id ?? null,
        institution_id: institutionId,
        label,
        username,
        // Unchanged (or not loaded yet) means "keep the current password".
        password: a && (password === original || (original === null && password === "")) ? null : password,
        origins: origins.filter((o) => o.url.trim()),
        notes,
        custom_fields: fields,
        tags: tags.split(",").map((s) => s.trim()).filter(Boolean),
        auto_submit: autoSubmit,
      });
      setPassword("");
      props.notify({ kind: "success", message: t.saved, action: null });
      props.onSaved(id);
    } catch (err) {
      setError(errorText(err));
    } finally {
      setBusy(false);
    }
  };

  const setOrigin = (i: number, patch: Partial<OriginInput>) =>
    setOrigins((os) => os.map((o, j) => (j === i ? { ...o, ...patch } : o)));
  const setField = (i: number, patch: Partial<CustomFieldInput>) =>
    setFields((fs) => fs.map((f, j) => (j === i ? { ...f, ...patch } : f)));

  return (
    <Dialog
      wide
      title={a ? `${t.edit}: ${a.label}` : t.addAccount}
      onClose={props.onClose}
      footer={
        <>
          <button className="btn" onClick={props.onClose}>{t.cancel}</button>
          <button className="btn primary" onClick={() => submit()} disabled={busy}>
            {busy ? t.saving : t.save}
          </button>
        </>
      }
    >
      <form onSubmit={submit}>
        <div className="row wrap" style={{ alignItems: "flex-start", gap: 14 }}>
          <label className="field grow">
            <span>{t.label}</span>
            <input className="input" value={label} onChange={(e) => setLabel(e.target.value)} placeholder={t.labelPlaceholder} />
          </label>
          <label className="field grow">
            <span>{t.institution}</span>
            <select
              className="input"
              value={institutionId ?? ""}
              onChange={(e) => setInstitutionId(e.target.value || null)}
            >
              {props.institutions.map((i) => (
                <option key={i.id} value={i.id}>{i.name}</option>
              ))}
              <option value="">{t.noInstitution}</option>
            </select>
          </label>
        </div>

        <label className="field">
          <span>{t.username}</span>
          <input className="input" value={username} onChange={(e) => setUsername(e.target.value)} autoComplete="off" spellCheck={false} />
        </label>

        <div className="field">
          <span>{t.password}</span>
          <div className="row">
            <div className="grow">
              <PasswordInput value={password} onChange={setPassword} />
            </div>
            <button type="button" className="btn small" onClick={() => setGen(true)}>
              <Dice size={14} /> {t.generate}
            </button>
          </div>
        </div>

        {a && a.history_count > 0 && (
          <div className="field">
            <span>{t.history}</span>
            {history === null ? (
              <div>
                <button type="button" className="btn small" onClick={async () => setHistory(await api.passwordHistory(a.id))}>
                  {t.show} ({a.history_count})
                </button>
              </div>
            ) : (
              history.map((h, i) => (
                <div key={i} className="row small">
                  <span className="mono grow">{h.password}</span>
                  <span className="muted">{formatDate(h.changed_at, "id")}</span>
                </div>
              ))
            )}
          </div>
        )}

        <div className="field">
          <span>{institutionId ? t.extraOrigins : t.origins}</span>
          {institutionId && <span className="hint">{t.extraOriginsHint}</span>}
          {origins.map((o, i) => (
            <div className="row" key={i}>
              <input
                className="input grow mono"
                value={o.url}
                placeholder="https://sso.kampus.ac.id"
                onChange={(e) => setOrigin(i, { url: e.target.value })}
                aria-invalid={!!o.url && !o.url.startsWith("https://")}
              />
              <label className="check" style={{ margin: 0 }}>
                <input
                  type="checkbox"
                  checked={o.match_mode === "subdomain"}
                  onChange={(e) => setOrigin(i, { match_mode: e.target.checked ? "subdomain" : "exact" })}
                />
                <span className="small">{t.subdomain}</span>
              </label>
              <button type="button" className="btn ghost" aria-label={t.delete} onClick={() => setOrigins((os) => os.filter((_, j) => j !== i))}>
                <Trash size={14} />
              </button>
            </div>
          ))}
          {origins.some((o) => o.url && !o.url.startsWith("https://")) && <span className="error">{t.notHttps}</span>}
          <div>
            <button type="button" className="btn small" onClick={() => setOrigins((os) => [...os, { url: "https://", match_mode: "exact" }])}>
              <Plus size={14} /> {t.addOrigin}
            </button>
          </div>
        </div>

        <div className="field">
          <span>{t.customFields}</span>
          {fields.map((f, i) => (
            <div className="row wrap" key={i}>
              <input className="input" style={{ flex: "1 1 120px" }} placeholder={t.fieldName} value={f.name} onChange={(e) => setField(i, { name: e.target.value })} />
              <input
                className="input"
                style={{ flex: "1 1 140px" }}
                placeholder={f.secret && a ? t.unchangedPassword : t.fieldValue}
                type={f.secret ? "password" : "text"}
                value={f.value}
                onChange={(e) => setField(i, { value: e.target.value })}
              />
              <input
                className="input mono"
                style={{ flex: "1 1 140px" }}
                placeholder={t.fieldSelector}
                value={f.selector ?? ""}
                onChange={(e) => setField(i, { selector: e.target.value || null })}
              />
              <label className="check" style={{ margin: 0 }}>
                <input type="checkbox" checked={f.secret} onChange={(e) => setField(i, { secret: e.target.checked })} />
                <span className="small">{t.secretField}</span>
              </label>
              <button type="button" className="btn ghost" aria-label={t.delete} onClick={() => setFields((fs) => fs.filter((_, j) => j !== i))}>
                <Trash size={14} />
              </button>
            </div>
          ))}
          <div>
            <button type="button" className="btn small" onClick={() => setFields((fs) => [...fs, { name: "", value: "", selector: null, secret: false }])}>
              <Plus size={14} /> {t.addField}
            </button>
          </div>
        </div>

        <label className="field">
          <span>{t.tags}</span>
          <input className="input" value={tags} onChange={(e) => setTags(e.target.value)} placeholder="kampus, dinas" />
          <span className="hint">{t.tagsHint}</span>
        </label>

        <label className="field">
          <span>{t.notes}</span>
          <textarea className="input" value={notes} onChange={(e) => setNotes(e.target.value)} />
        </label>

        <label className="check">
          <input type="checkbox" checked={autoSubmit} onChange={(e) => setAutoSubmit(e.target.checked)} />
          <span>{t.autoSubmit}</span>
        </label>

        {error && <p className="error" role="alert">{error}</p>}
        <button type="submit" hidden />
      </form>

      {gen && (
        <GeneratorDialog
          notify={props.notify}
          onClose={() => setGen(false)}
          onUse={(p) => {
            setPassword(p);
            setGen(false);
          }}
        />
      )}
    </Dialog>
  );
}
