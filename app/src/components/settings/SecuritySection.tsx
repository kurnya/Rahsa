import { useState } from "react";
import { api, errorText, type Settings } from "../../api";
import type { Notify } from "../../App";
import { useT } from "../../i18n";
import { PasswordInput, StrengthMeter } from "../ui";
import { NumberSelect, SettingBlock, SettingGroup, SettingRow, Switch } from "./parts";

export function SecuritySection(props: { settings: Settings; update: (p: Partial<Settings>) => void; notify: Notify }) {
  const t = useT();
  const s = props.settings;
  return (
    <>
      <SettingGroup title={t.security}>
        <SettingRow label={t.autoLock} hint={t.autoLockHint} htmlFor="set-autolock">
          <NumberSelect
            id="set-autolock"
            value={s.auto_lock_minutes}
            options={[1, 5, 10, 15, 30, 60]}
            unit={t.minutes}
            onChange={(v) => props.update({ auto_lock_minutes: v })}
          />
        </SettingRow>
        <SettingRow label={t.lockOnScreenLock} hint={t.screenLockHint} htmlFor="set-screenlock">
          <Switch
            id="set-screenlock"
            label={t.lockOnScreenLock}
            checked={s.lock_on_screen_lock}
            onChange={(v) => props.update({ lock_on_screen_lock: v })}
          />
        </SettingRow>
        <SettingRow label={t.clipboardClear} hint={t.clipboardHint} htmlFor="set-clipboard">
          <NumberSelect
            id="set-clipboard"
            value={s.clipboard_clear_seconds}
            options={[10, 20, 30, 60, 120]}
            unit={t.seconds}
            onChange={(v) => props.update({ clipboard_clear_seconds: v })}
          />
        </SettingRow>
      </SettingGroup>

      <ChangeMasterPassword notify={props.notify} />
    </>
  );
}

function ChangeMasterPassword({ notify }: { notify: Notify }) {
  const t = useT();
  const [oldPw, setOldPw] = useState("");
  const [newPw, setNewPw] = useState("");
  const [busy, setBusy] = useState(false);

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    if ([...newPw].length < 12) return notify({ kind: "error", message: t.tooShort, action: null });
    setBusy(true);
    try {
      await api.changeMasterPassword(oldPw, newPw);
      setOldPw("");
      setNewPw("");
      notify({ kind: "success", message: t.masterChanged, action: null });
    } catch (err) {
      notify({ kind: "error", message: errorText(err), action: null });
    } finally {
      setBusy(false);
    }
  };

  return (
    <SettingGroup title={t.changeMaster}>
      <SettingBlock>
        <p className="set-hint" style={{ margin: "0 0 8px" }}>{t.masterSectionHint}</p>
        <form onSubmit={submit}>
          <label className="field">
            <span>{t.oldMaster}</span>
            <PasswordInput value={oldPw} onChange={setOldPw} autoComplete="current-password" />
          </label>
          <label className="field">
            <span>{t.newMaster}</span>
            <PasswordInput value={newPw} onChange={setNewPw} autoComplete="new-password" />
            <StrengthMeter password={newPw} />
          </label>
          <button className="btn primary" type="submit" disabled={!oldPw || !newPw || busy}>
            {busy ? t.saving : t.changeMaster}
          </button>
        </form>
      </SettingBlock>
    </SettingGroup>
  );
}
