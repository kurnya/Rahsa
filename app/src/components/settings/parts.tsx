// Building blocks shared by every settings section.
import type { ReactNode } from "react";

/** A titled, bordered group of related rows. */
export function SettingGroup(props: { title?: string; children: ReactNode }) {
  return (
    <section className="set-group">
      {props.title && <h4>{props.title}</h4>}
      {props.children}
    </section>
  );
}

/** One setting: name and explanation on the left, its control on the right. */
export function SettingRow(props: { label: string; hint?: ReactNode; children?: ReactNode; htmlFor?: string }) {
  return (
    <div className="set-row">
      <div className="set-text">
        <label className="set-label" htmlFor={props.htmlFor}>
          {props.label}
        </label>
        {props.hint && <div className="set-hint">{props.hint}</div>}
      </div>
      {props.children && <div className="set-control">{props.children}</div>}
    </div>
  );
}

/** Free-form content inside a group (forms, lists). */
export function SettingBlock(props: { children: ReactNode }) {
  return <div className="set-block">{props.children}</div>;
}

export function Switch(props: { id?: string; checked: boolean; onChange: (v: boolean) => void; label: string }) {
  return (
    <input
      id={props.id}
      type="checkbox"
      role="switch"
      className="switch"
      aria-label={props.label}
      checked={props.checked}
      onChange={(e) => props.onChange(e.target.checked)}
    />
  );
}

export function StatusDot(props: { ok: boolean; children: ReactNode }) {
  return (
    <span className="status">
      <i className={props.ok ? "ok" : "bad"} aria-hidden="true" />
      {props.children}
    </span>
  );
}

/** A <select> of preset numbers that also keeps an unusual saved value selectable. */
export function NumberSelect(props: {
  id?: string;
  value: number;
  options: number[];
  unit: string;
  onChange: (v: number) => void;
}) {
  const options = props.options.includes(props.value) ? props.options : [...props.options, props.value].sort((a, b) => a - b);
  return (
    <select id={props.id} className="input" value={props.value} onChange={(e) => props.onChange(+e.target.value)}>
      {options.map((n) => (
        <option key={n} value={n}>
          {n} {props.unit}
        </option>
      ))}
    </select>
  );
}
