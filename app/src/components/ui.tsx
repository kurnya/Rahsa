import { useEffect, useRef, useState, type ReactNode } from "react";
import { zxcvbn, zxcvbnOptions } from "@zxcvbn-ts/core";
import * as common from "@zxcvbn-ts/language-common";
import { useT } from "../i18n";
import { Close } from "./icons";

zxcvbnOptions.setOptions({
  dictionary: { ...common.dictionary },
  graphs: common.adjacencyGraphs,
});

const dialogStack: object[] = [];

export function Dialog(props: {
  title: string;
  onClose: () => void;
  children: ReactNode;
  footer?: ReactNode;
  wide?: boolean;
  className?: string;
  /** Focus the first control on open (default). False focuses the dialog itself. */
  focusFirst?: boolean;
}) {
  const t = useT();
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const prev = document.activeElement as HTMLElement | null;
    if (props.focusFirst === false) ref.current?.focus();
    else ref.current?.querySelector<HTMLElement>("input, select, textarea, button:not(.dialog-close)")?.focus();
    const token = {};
    dialogStack.push(token);
    const onKey = (e: KeyboardEvent) => {
      // Only the top-most dialog reacts, so Escape in a nested dialog keeps the parent open.
      if (e.key === "Escape" && dialogStack[dialogStack.length - 1] === token) props.onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("keydown", onKey);
      dialogStack.splice(dialogStack.indexOf(token), 1);
      prev?.focus();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
  return (
    <div className="scrim" onMouseDown={(e) => e.target === e.currentTarget && props.onClose()}>
      <div className={`dialog${props.wide ? " wide" : ""}${props.className ? ` ${props.className}` : ""}`} role="dialog" aria-modal="true" aria-label={props.title} ref={ref} tabIndex={-1}>
        <div className="dialog-head">
          <h2>{props.title}</h2>
          <button className="btn ghost dialog-close" onClick={props.onClose} aria-label={t.close}>
            <Close />
          </button>
        </div>
        <div className="dialog-body">{props.children}</div>
        {props.footer && <div className="dialog-foot">{props.footer}</div>}
      </div>
    </div>
  );
}

export function StrengthMeter({ password }: { password: string }) {
  const t = useT();
  if (!password) return null;
  const score = zxcvbn(password).score;
  return (
    <div aria-live="polite">
      <div className="meter" role="meter" aria-valuemin={0} aria-valuemax={4} aria-valuenow={score} aria-label="Kekuatan">
        <i style={{ width: `${((score + 1) / 5) * 100}%` }} />
      </div>
      <span className="meter-label">{t.strength[score]}</span>
    </div>
  );
}

export function PasswordInput(props: {
  value: string;
  onChange: (v: string) => void;
  placeholder?: string;
  autoFocus?: boolean;
  id?: string;
  autoComplete?: string;
}) {
  const t = useT();
  const [show, setShow] = useState(false);
  return (
    <div className="row">
      <input
        id={props.id}
        className={`input grow${show ? " mono" : ""}`}
        type={show ? "text" : "password"}
        value={props.value}
        placeholder={props.placeholder}
        autoFocus={props.autoFocus}
        autoComplete={props.autoComplete ?? "off"}
        spellCheck={false}
        onChange={(e) => props.onChange(e.target.value)}
      />
      <button type="button" className="btn small" onClick={() => setShow((s) => !s)}>
        {show ? t.hide : t.show}
      </button>
    </div>
  );
}

/** Digits and symbols tinted so a generated password reads character by character. */
export function SecretText({ text }: { text: string }) {
  return (
    <>
      {[...text].map((c, i) =>
        /[0-9]/.test(c) ? (
          <span key={i} className="d">{c}</span>
        ) : /[A-Za-z]/.test(c) ? (
          c
        ) : (
          <span key={i} className="s">{c}</span>
        ),
      )}
    </>
  );
}

export function formatDate(ms: number | null, lang: string): string {
  if (!ms) return "—";
  return new Date(ms).toLocaleDateString(lang === "en" ? "en-GB" : "id-ID", {
    day: "numeric",
    month: "short",
    year: "numeric",
  });
}

export function relativeDays(ms: number): number {
  return Math.max(0, Math.floor((Date.now() - ms) / 86_400_000));
}
