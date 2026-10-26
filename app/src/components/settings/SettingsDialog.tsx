import { useEffect, useState } from "react";
import { api, errorText, type Settings } from "../../api";
import type { Notify } from "../../App";
import { useT } from "../../i18n";
import { Dialog } from "../ui";
import { BackupSection } from "./BackupSection";
import { ExtensionSection } from "./ExtensionSection";
import { GeneralSection } from "./GeneralSection";
import { SecuritySection } from "./SecuritySection";

type Section = "general" | "security" | "extension" | "backup";

export function SettingsDialog(props: {
  notify: Notify;
  onClose: () => void;
  onLanguage: (l: "id" | "en") => void;
  onLocked: () => void;
  initial?: Section;
}) {
  const t = useT();
  const [section, setSection] = useState<Section>(props.initial ?? "general");
  const [settings, setSettings] = useState<Settings | null>(null);

  const reload = () => api.getSettings().then(setSettings);
  useEffect(() => {
    reload();
  }, []);

  // Settings save immediately; the UI updates first, then reverts on error.
  const update = async (patch: Partial<Settings>) => {
    if (!settings) return;
    const prev = settings;
    const next = { ...settings, ...patch };
    setSettings(next);
    try {
      await api.updateSettings(next);
      if (patch.language) props.onLanguage(patch.language);
    } catch (e) {
      setSettings(prev);
      props.notify({ kind: "error", message: errorText(e), action: null });
    }
  };

  const sections: { id: Section; label: string }[] = [
    { id: "general", label: t.general },
    { id: "security", label: t.security },
    { id: "extension", label: t.extension },
    { id: "backup", label: t.backup },
  ];

  return (
    <Dialog title={t.settings} onClose={props.onClose} className="settings-dialog" focusFirst={false}>
      <nav className="settings-nav" aria-label={t.settings}>
        {sections.map((s) => (
          <button key={s.id} className="rail-item" aria-current={section === s.id} onClick={() => setSection(s.id)}>
            {s.label}
          </button>
        ))}
      </nav>
      <div className="settings-pane">
        {settings && section === "general" && <GeneralSection settings={settings} update={update} />}
        {settings && section === "security" && <SecuritySection settings={settings} update={update} notify={props.notify} />}
        {section === "extension" && <ExtensionSection lang={settings?.language ?? "id"} notify={props.notify} />}
        {settings && section === "backup" && (
          <BackupSection settings={settings} reload={reload} notify={props.notify} onRestored={props.onClose} />
        )}
      </div>
    </Dialog>
  );
}
