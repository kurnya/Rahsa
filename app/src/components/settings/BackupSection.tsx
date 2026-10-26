import { useEffect, useState } from "react";
import { save } from "@tauri-apps/plugin-dialog";
import { api, errorText, type Settings } from "../../api";
import type { Notify } from "../../App";
import { useT } from "../../i18n";
import { formatDate, PasswordInput } from "../ui";
import { RestoreDialog } from "../RestoreDialog";
import { SettingBlock, SettingGroup, SettingRow } from "./parts";

function today(): string {
  const d = new Date();
  return `${d.getFullYear()}${String(d.getMonth() + 1).padStart(2, "0")}${String(d.getDate()).padStart(2, "0")}`;
}

export function BackupSection(props: { settings: Settings; reload: () => void; notify: Notify; onRestored: () => void }) {
  const t = useT();
  const { notify } = props;
  const [pw, setPw] = useState("");
  const [busy, setBusy] = useState(false);
  const [restoring, setRestoring] = useState(false);
  const [vaultPath, setVaultPath] = useState("");

  useEffect(() => {
    api.status().then((s) => setVaultPath(s.vault_path));
  }, []);

  const exportBackup = async (e: React.FormEvent) => {
    e.preventDefault();
    const path = await save({ defaultPath: `rahsa-backup-${today()}.rahsa`, filters: [{ name: "Rahsa", extensions: ["rahsa"] }] });
    if (!path) return;
    setBusy(true);
    try {
      await api.exportBackup(path, pw);
      setPw("");
      props.reload();
      notify({ kind: "success", message: t.exported, action: null });
    } catch (err) {
      notify({ kind: "error", message: errorText(err), action: null });
    } finally {
      setBusy(false);
    }
  };

  const saveKit = async () => {
    const path = await save({ defaultPath: "Rahsa-Emergency-Kit.html", filters: [{ name: "HTML", extensions: ["html"] }] });
    if (!path) return;
    await api.saveEmergencyKit(path).then(
      () => notify({ kind: "success", message: t.kitSaved, action: null }),
      (err) => notify({ kind: "error", message: errorText(err), action: null }),
    );
  };

  return (
    <>
      <SettingGroup title={t.exportBackup}>
        <SettingBlock>
          <p className="set-hint" style={{ margin: "0 0 8px" }}>
            {t.exportHint} {t.exportLead} {t.lastBackup}: <strong>{formatDate(props.settings.last_backup_at, props.settings.language)}</strong>
          </p>
          <form onSubmit={exportBackup}>
            <label className="field">
              <span>{t.confirmWithMaster}</span>
              <PasswordInput value={pw} onChange={setPw} autoComplete="current-password" />
            </label>
            <button className="btn primary" type="submit" disabled={!pw || busy}>
              {busy ? t.saving : t.exportBackup}
            </button>
          </form>
        </SettingBlock>
      </SettingGroup>

      <SettingGroup title={t.dataSection}>
        <SettingRow label={t.restoreBackup} hint={t.restoreHint}>
          <button className="btn small" onClick={() => setRestoring(true)}>{t.chooseFile}</button>
        </SettingRow>
        <SettingRow label={t.emergencyKit} hint={t.kitHint}>
          <button className="btn small" onClick={saveKit}>{t.save}</button>
        </SettingRow>
        <SettingRow label={t.vaultLocation} hint={<span className="mono">{vaultPath}</span>} />
      </SettingGroup>

      {restoring && <RestoreDialog onClose={() => setRestoring(false)} onDone={props.onRestored} notify={notify} />}
    </>
  );
}
