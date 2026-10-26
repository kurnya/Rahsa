import { useEffect, useState } from "react";
import { api, errorText, type TrashEntry } from "../api";
import type { Notify } from "../App";
import { useT } from "../i18n";
import { Dialog, formatDate } from "./ui";

export function TrashDialog(props: { onClose: () => void; lang: string; notify: Notify }) {
  const t = useT();
  const [items, setItems] = useState<TrashEntry[] | null>(null);
  const load = () => api.listTrash().then(setItems);
  useEffect(() => {
    load();
  }, []);

  const act = async (f: () => Promise<void>) => {
    try {
      await f();
      await load();
    } catch (e) {
      props.notify({ kind: "error", message: errorText(e), action: null });
    }
  };

  return (
    <Dialog title={t.trash} onClose={props.onClose}>
      <p className="muted">{t.trashLead}</p>
      {items?.length === 0 && <p>{t.trashEmpty}</p>}
      <div style={{ marginBottom: 16 }}>
        {items?.map((it) => (
          <div className="list-row" key={it.account.id}>
            <div className="grow">
              <strong>{it.account.label}</strong>
              <div className="muted small">
                {it.account.username} · {t.deletedOn} {formatDate(it.deleted_at, props.lang)}
              </div>
            </div>
            <button className="btn small" onClick={() => act(() => api.restoreTrash(it.account.id))}>{t.restore}</button>
            <button className="btn small danger" onClick={() => act(() => api.purgeTrashItem(it.account.id))}>
              {t.deleteForever}
            </button>
          </div>
        ))}
      </div>
    </Dialog>
  );
}
