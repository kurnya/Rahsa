import { useCallback, useEffect, useState } from "react";
import { api, errorText, type GeneratorOptions } from "../api";
import type { Notify } from "../App";
import { useT } from "../i18n";
import { Dialog, SecretText } from "./ui";

const FALLBACK: GeneratorOptions = {
  length: 20,
  lower: true,
  upper: true,
  digits: true,
  symbols: true,
  min_digits: 1,
  min_symbols: 1,
  min_upper: 1,
  min_lower: 1,
  avoid_ambiguous: true,
  symbol_set: null,
};

export function GeneratorDialog(props: { onClose: () => void; notify: Notify; onUse?: (p: string) => void }) {
  const t = useT();
  const [opts, setOpts] = useState<GeneratorOptions | null>(null);
  const [value, setValue] = useState("");
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    api.getSettings().then((s) => setOpts(s.generator_defaults)).catch(() => setOpts(FALLBACK));
  }, []);

  const run = useCallback(async (o: GeneratorOptions) => {
    try {
      setValue(await api.generatePassword(o));
      setError(null);
    } catch (e) {
      setError(errorText(e));
    }
  }, []);

  useEffect(() => {
    if (opts) run(opts);
  }, [opts, run]);

  if (!opts) return null;
  const set = (patch: Partial<GeneratorOptions>) => setOpts({ ...opts, ...patch });

  const copy = async () => {
    try {
      const secs = await api.copyGenerated(value);
      props.notify({ kind: "success", message: t.copied(secs), action: null });
    } catch (e) {
      props.notify({ kind: "error", message: errorText(e), action: null });
    }
  };

  return (
    <Dialog
      title={t.generator}
      onClose={props.onClose}
      footer={
        <>
          <button className="btn" onClick={() => run(opts)}>{t.regenerate}</button>
          {props.onUse ? (
            <button className="btn primary" disabled={!value} onClick={() => props.onUse!(value)}>{t.usePassword}</button>
          ) : (
            <button className="btn primary" disabled={!value} onClick={copy}>{t.copy}</button>
          )}
        </>
      }
    >
      <div className="generated" aria-live="polite">
        <SecretText text={value} />
      </div>
      <label className="field">
        <span>
          {t.length}: {opts.length}
        </span>
        <input type="range" min={12} max={64} value={opts.length} onChange={(e) => set({ length: +e.target.value })} />
      </label>
      <div className="row wrap" style={{ gap: 16 }}>
        {(["lower", "upper", "digits", "symbols"] as const).map((k) => (
          <label className="check" key={k}>
            <input type="checkbox" checked={opts[k]} onChange={(e) => set({ [k]: e.target.checked })} />
            <span>{t[k]}</span>
          </label>
        ))}
      </div>
      <div className="row wrap" style={{ gap: 14 }}>
        <label className="field grow">
          <span>{t.minDigits}</span>
          <input className="input" type="number" min={0} max={10} value={opts.min_digits} onChange={(e) => set({ min_digits: +e.target.value })} />
        </label>
        <label className="field grow">
          <span>{t.minSymbols}</span>
          <input className="input" type="number" min={0} max={10} value={opts.min_symbols} onChange={(e) => set({ min_symbols: +e.target.value })} />
        </label>
      </div>
      <label className="field">
        <span>{t.symbolSet}</span>
        <input
          className="input mono"
          value={opts.symbol_set ?? ""}
          placeholder="!@#$%"
          onChange={(e) => set({ symbol_set: e.target.value || null })}
        />
      </label>
      <label className="check">
        <input type="checkbox" checked={opts.avoid_ambiguous} onChange={(e) => set({ avoid_ambiguous: e.target.checked })} />
        <span>{t.avoidAmbiguous}</span>
      </label>
      {error && <p className="error" role="alert">{error}</p>}
    </Dialog>
  );
}
