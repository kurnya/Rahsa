import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { api, errorText, type AccountMeta } from "../api";
import type { Notify } from "../App";
import { useT } from "../i18n";

/** The ⋯ menu on an account card: copy and delete. A small popover, not a dialog. */
export function CardMenu(props: { account: AccountMeta; notify: Notify; onDeleted: () => void }) {
  const t = useT();
  const { account: a, notify } = props;
  const [pos, setPos] = useState<{ left: number; top?: number; bottom?: number } | null>(null);
  const open = pos !== null;
  const trigger = useRef<HTMLButtonElement>(null);
  const list = useRef<HTMLDivElement>(null);
  const setOpen = (v: boolean | ((o: boolean) => boolean)) => {
    const next = typeof v === "function" ? v(open) : v;
    if (!next) return setPos(null);
    // Anchor to the ⋯ button; open upward when there is no room below.
    const r = trigger.current!.getBoundingClientRect();
    const left = Math.min(r.right - 160, window.innerWidth - 168);
    setPos(r.bottom + 170 > window.innerHeight ? { left, bottom: window.innerHeight - r.top + 4 } : { left, top: r.bottom + 4 });
  };

  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      const n = e.target as Node;
      if (!list.current?.contains(n) && !trigger.current?.contains(n)) setOpen(false);
    };
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && setOpen(false);
    document.addEventListener("mousedown", onDown);
    window.addEventListener("keydown", onKey);
    list.current?.querySelector<HTMLButtonElement>("button")?.focus();
    const close = () => setPos(null);
    window.addEventListener("resize", close);
    document.addEventListener("scroll", close, true);
    return () => {
      document.removeEventListener("mousedown", onDown);
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("resize", close);
      document.removeEventListener("scroll", close, true);
    };
  }, [open]);

  const run = (f: () => Promise<void> | void) => async () => {
    setOpen(false);
    try {
      await f();
    } catch (e) {
      notify({ kind: "error", message: errorText(e), action: null });
    }
  };

  const copy = (field: "username" | "password") =>
    run(async () => {
      const secs = await api.copySecret(a.id, field);
      notify({ kind: "success", message: t.copied(secs), action: null });
    });

  return (
    <div className="card-menu">
      <button
        ref={trigger}
        className="btn ghost small card-menu-trigger"
        aria-label={`${t.moreActions}: ${a.label}`}
        aria-haspopup="menu"
        aria-expanded={open}
        onClick={() => setOpen((o) => !o)}
      >
        ⋯
      </button>
      {pos &&
        createPortal(
        <div className="card-menu-list" role="menu" ref={list} style={pos}>
          <button role="menuitem" onClick={copy("username")}>{t.copyUsername}</button>
          <button role="menuitem" onClick={copy("password")}>{t.copyPassword}</button>
          <button
            role="menuitem"
            className="danger"
            onClick={run(async () => {
              await api.deleteAccount(a.id);
              notify({ kind: "info", message: t.movedToTrash, action: null });
              props.onDeleted();
            })}
          >
            {t.delete}
          </button>
        </div>,
        document.body,
      )}
    </div>
  );
}
