import { api } from "../api";
import { useT } from "../i18n";
import { Dialog } from "./ui";

export function PairingDialog({ code, onClose }: { code: string; onClose: () => void }) {
  const t = useT();
  const respond = async (approve: boolean) => {
    await api.respondPairing(approve).catch(() => {});
    onClose();
  };
  return (
    <Dialog
      title={t.pairTitle}
      onClose={() => respond(false)}
      footer={
        <>
          <button className="btn" onClick={() => respond(false)}>{t.pairReject}</button>
          <button className="btn primary" onClick={() => respond(true)}>{t.pairApprove}</button>
        </>
      }
    >
      <p className="muted">{t.pairLead}</p>
      <div className="pair-code" aria-label={code.split("").join(" ")}>
        {code.slice(0, 3)} {code.slice(3)}
      </div>
    </Dialog>
  );
}
