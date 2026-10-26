import { useEffect, useState } from "react";
import { api, errorText, type ExtensionInfo, type Pairing } from "../../api";
import type { Notify } from "../../App";
import { useT } from "../../i18n";
import { formatDate } from "../ui";
import { SettingBlock, SettingGroup, SettingRow, StatusDot } from "./parts";

export function ExtensionSection(props: { lang: string; notify: Notify }) {
  const t = useT();
  const [ext, setExt] = useState<ExtensionInfo | null>(null);
  const [pairings, setPairings] = useState<Pairing[]>([]);

  const load = () => {
    api.extensionInfo().then(setExt).catch(() => {});
    api.listPairings().then(setPairings).catch(() => {});
  };
  useEffect(load, []);

  const revoke = async (id: string) => {
    try {
      await api.revokePairing(id);
      load();
    } catch (e) {
      props.notify({ kind: "error", message: errorText(e), action: null });
    }
  };

  return (
    <>
      <SettingGroup title={t.extensionStatus}>
        {ext && (
          <>
            <SettingRow label={t.extensionId}>
              <span className="mono small">{ext.extension_id}</span>
            </SettingRow>
            <SettingRow label={t.nativeHost} hint={ext.host_registered ? undefined : t.hostMissing}>
              <StatusDot ok={ext.host_registered}>{ext.host_registered ? t.registered : t.notRegistered}</StatusDot>
            </SettingRow>
            <SettingRow label={t.connection}>
              <StatusDot ok={ext.connected}>{ext.connected ? t.connected : t.notConnected}</StatusDot>
            </SettingRow>
          </>
        )}
      </SettingGroup>

      <SettingGroup title={t.pairedExtensions}>
        {pairings.length === 0 ? (
          <SettingBlock>
            <p className="set-hint" style={{ margin: 0 }}>{t.noPairings}</p>
          </SettingBlock>
        ) : (
          pairings.map((p) => (
            <SettingRow
              key={p.extension_id}
              label={`Chrome · ${p.extension_id.slice(0, 8)}…`}
              hint={`${t.paired} ${formatDate(p.paired_at, props.lang)} · ${t.lastSeen} ${formatDate(p.last_seen_at, props.lang)}`}
            >
              <StatusDot ok={p.connected}>{p.connected ? t.connected : t.notConnected}</StatusDot>
              <button className="btn small danger" onClick={() => revoke(p.extension_id)}>
                {t.revoke}
              </button>
            </SettingRow>
          ))
        )}
      </SettingGroup>
    </>
  );
}
