import { useCallback, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { api, errorText, type AppStatus, type Toast } from "./api";
import { dictionaries, I18n } from "./i18n";
import { Setup } from "./screens/Setup";
import { Unlock } from "./screens/Unlock";
import { VaultView } from "./screens/VaultView";
import { PairingDialog } from "./components/PairingDialog";

export interface ToastItem extends Toast {
  key: number;
}

let toastKey = 0;

export default function App() {
  const [status, setStatus] = useState<AppStatus | null>(null);
  const [lang, setLang] = useState<"id" | "en">("id");
  const [toasts, setToasts] = useState<ToastItem[]>([]);
  const [pairCode, setPairCode] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    const s = await api.status();
    setStatus(s);
    if (s.unlocked) setLang(s.language);
  }, []);

  const notify = useCallback((t: Toast) => {
    const key = ++toastKey;
    setToasts((ts) => [...ts.slice(-3), { ...t, key }]);
    setTimeout(() => setToasts((ts) => ts.filter((x) => x.key !== key)), t.action ? 15000 : 5000);
  }, []);

  useEffect(() => {
    refresh();
    const t = dictionaries[lang];
    const subs = [
      listen<string>("vault-locked", (e) => {
        refresh();
        const msg = { idle: t.lockedByIdle, screen: t.lockedByScreen, sleep: t.lockedBySleep }[e.payload];
        if (msg) notify({ kind: "info", message: msg, action: null });
      }),
      listen("vault-unlocked", () => refresh()),
      listen<Toast>("toast", (e) => notify(e.payload)),
      listen<{ code: string }>("pairing-request", (e) => setPairCode(e.payload.code)),
      listen<boolean>("pairing-finished", (e) => {
        setPairCode(null);
        if (e.payload) notify({ kind: "success", message: t.pairDone, action: null });
      }),
    ];
    // A pairing request may have arrived before the window loaded.
    api.extensionInfo().then((i) => i.pending_code && setPairCode(i.pending_code)).catch(() => {});
    return () => subs.forEach((p) => p.then((un) => un()));
  }, [refresh, notify, lang]);

  const t = dictionaries[lang];

  const runIncognito = async (accountId: string) => {
    try {
      await api.masuk(accountId, true);
    } catch (e) {
      notify({ kind: "error", message: errorText(e), action: null });
    }
  };

  return (
    <I18n.Provider value={t}>
      {status === null ? null : !status.vault_exists ? (
        <Setup onDone={refresh} notify={notify} />
      ) : !status.unlocked ? (
        <Unlock status={status} onDone={refresh} notify={notify} />
      ) : (
        <VaultView notify={notify} onLock={refresh} onLanguage={setLang} />
      )}

      {pairCode && <PairingDialog code={pairCode} onClose={() => setPairCode(null)} />}

      <div className="toasts" role="status" aria-live="polite">
        {toasts.map((x) => (
          <div key={x.key} className={`toast ${x.kind}`}>
            <span>{x.message}</span>
            {x.action?.kind === "incognito" && (
              <button className="btn small" onClick={() => runIncognito(x.action!.account_id)}>
                {t.openIncognito}
              </button>
            )}
          </div>
        ))}
      </div>
    </I18n.Provider>
  );
}

export type Notify = (t: Toast) => void;
