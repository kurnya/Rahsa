import type { Settings } from "../../api";
import { useT } from "../../i18n";
import { SettingGroup, SettingRow } from "./parts";

export function GeneralSection(props: { settings: Settings; update: (p: Partial<Settings>) => void }) {
  const t = useT();
  return (
    <SettingGroup>
      <SettingRow label={t.language} hint={t.languageHint} htmlFor="set-language">
        <select
          id="set-language"
          className="input"
          value={props.settings.language}
          onChange={(e) => props.update({ language: e.target.value as "id" | "en" })}
        >
          <option value="id">Bahasa Indonesia</option>
          <option value="en">English</option>
        </select>
      </SettingRow>
    </SettingGroup>
  );
}
