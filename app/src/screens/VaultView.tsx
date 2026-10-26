import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { api, errorText, originLabel, type AccountMeta, type Institution, type Overview } from "../api";
import type { Notify } from "../App";
import { useT } from "../i18n";
import { Building, Dice, Gear, LockIcon, Mark, Pencil, Plus, Search, Trash } from "../components/icons";
import { formatDate, shortDate } from "../components/ui";
import { CardMenu } from "../components/CardMenu";
import { AccountForm } from "../components/AccountForm";
import { InstitutionForm } from "../components/InstitutionForm";
import { GeneratorDialog } from "../components/GeneratorDialog";
import { SettingsDialog } from "../components/settings/SettingsDialog";
import { TrashDialog } from "../components/TrashDialog";

type Filter = { kind: "all" } | { kind: "institution"; id: string } | { kind: "none" } | { kind: "tag"; tag: string };
type Modal =
  | null
  | { kind: "institution"; institution: Institution | null }
  | { kind: "account"; account: AccountMeta | null; institutionId: string | null }
  | { kind: "generator" }
  | { kind: "settings"; section?: "backup" }
  | { kind: "trash" };

export function VaultView(props: { notify: Notify; onLock: () => void; onLanguage: (l: "id" | "en") => void }) {
  const t = useT();
  const { notify } = props;
  const [data, setData] = useState<Overview | null>(null);
  const [filter, setFilter] = useState<Filter>({ kind: "all" });
  const [query, setQuery] = useState("");
  const [modal, setModal] = useState<Modal>(null);
  const [opening, setOpening] = useState<string | null>(null);
  const [lang, setLang] = useState<"id" | "en">("id");
  const searchRef = useRef<HTMLInputElement>(null);

  const reload = useCallback(async () => {
    try {
      setData(await api.overview());
    } catch {
      props.onLock();
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    reload();
    api.getSettings().then((s) => setLang(s.language)).catch(() => {});
    const un = listen("vault-unlocked", reload);
    const onKey = (e: KeyboardEvent) => {
      if (e.ctrlKey && e.key.toLowerCase() === "f") {
        e.preventDefault();
        searchRef.current?.focus();
      } else if (e.ctrlKey && e.key.toLowerCase() === "l") {
        e.preventDefault();
        lock();
      } else if (e.ctrlKey && e.key.toLowerCase() === "n") {
        e.preventDefault();
        setModal({ kind: "account", account: null, institutionId: filter.kind === "institution" ? filter.id : null });
      }
    };
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("keydown", onKey);
      un.then((f) => f());
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [filter]);

  const lock = async () => {
    await api.lock();
    props.onLock();
  };

  const institutions = data?.institutions ?? [];
  const accounts = data?.accounts ?? [];
  const tags = useMemo(() => [...new Set(accounts.flatMap((a) => a.tags))].sort(), [accounts]);
  const instById = useMemo(() => new Map(institutions.map((i) => [i.id, i])), [institutions]);

  // F-07: label, username, domain, tag. Plain in-memory filter; 1000 accounts is trivial.
  const matches = useMemo(() => {
    const q = query.trim().toLowerCase();
    return accounts.filter((a) => {
      if (filter.kind === "institution" && a.institution_id !== filter.id) return false;
      if (filter.kind === "none" && a.institution_id) return false;
      if (filter.kind === "tag" && !a.tags.includes(filter.tag)) return false;
      if (!q) return true;
      const inst = a.institution_id ? instById.get(a.institution_id) : undefined;
      const hay = [
        a.label,
        a.username,
        ...a.tags,
        ...a.origins.map(originLabel),
        ...(inst ? [inst.name, ...inst.origins.map(originLabel)] : []),
      ]
        .join("\n")
        .toLowerCase();
      return hay.includes(q);
    });
  }, [accounts, filter, query, instById]);

  // Group onto boards: one per institution, in rail order, then unassigned.
  const boards = useMemo(() => {
    const byInst = new Map<string | null, AccountMeta[]>();
    for (const a of matches) {
      const k = a.institution_id;
      byInst.set(k, [...(byInst.get(k) ?? []), a]);
    }
    for (const list of byInst.values()) {
      list.sort((x, y) => (y.last_used_at ?? 0) - (x.last_used_at ?? 0) || x.label.localeCompare(y.label));
    }
    const out: { inst: Institution | null; list: AccountMeta[] }[] = [];
    const showEmpty = !query && (filter.kind === "all" || filter.kind === "institution");
    for (const inst of institutions) {
      if (filter.kind === "institution" && filter.id !== inst.id) continue;
      const list = byInst.get(inst.id) ?? [];
      if (list.length || showEmpty) out.push({ inst, list });
    }
    const loose = byInst.get(null);
    if (loose?.length) out.push({ inst: null, list: loose });
    return out;
  }, [matches, institutions, filter, query]);


  const masuk = async (a: AccountMeta) => {
    setOpening(a.id);
    try {
      await api.masuk(a.id);
    } catch (e) {
      notify({ kind: "error", message: errorText(e), action: null });
    } finally {
      setOpening(null);
      reload();
    }
  };

  const countFor = (id: string) => accounts.filter((a) => a.institution_id === id).length;
  const looseCount = accounts.filter((a) => !a.institution_id).length;

  return (
    <div className="shell">
      <aside className="rail">
        <div className="rail-head">
          <Mark />
          <strong>{t.appName}</strong>
        </div>
        <nav aria-label={t.institutions}>
          <button className="rail-item" aria-current={filter.kind === "all"} onClick={() => setFilter({ kind: "all" })}>
            {t.allAccounts}
            <span className="count">{accounts.length}</span>
          </button>
          <h2>{t.institutions}</h2>
          {institutions.map((i) => (
            <button
              key={i.id}
              className="rail-item"
              aria-current={filter.kind === "institution" && filter.id === i.id}
              onClick={() => setFilter({ kind: "institution", id: i.id })}
            >
              <Building size={15} />
              <span className="grow" style={{ overflow: "hidden", textOverflow: "ellipsis" }}>{i.name}</span>
              <span className="count">{countFor(i.id)}</span>
            </button>
          ))}
          {looseCount > 0 && (
            <button className="rail-item" aria-current={filter.kind === "none"} onClick={() => setFilter({ kind: "none" })}>
              {t.noInstitution}
              <span className="count">{looseCount}</span>
            </button>
          )}
          <button className="rail-item muted" onClick={() => setModal({ kind: "institution", institution: null })}>
            <Plus size={15} /> {t.addInstitution}
          </button>
          {tags.length > 0 && <h2>{t.tags}</h2>}
          {tags.map((tag) => (
            <button
              key={tag}
              className="rail-item"
              aria-current={filter.kind === "tag" && filter.tag === tag}
              onClick={() => setFilter({ kind: "tag", tag })}
            >
              # {tag}
            </button>
          ))}
        </nav>
        <div className="rail-foot">
          <button className="rail-item" onClick={() => setModal({ kind: "generator" })}>
            <Dice size={15} /> {t.generator}
          </button>
          <button className="rail-item" onClick={() => setModal({ kind: "trash" })}>
            <Trash size={15} /> {t.trash}
            {data && data.trash_count > 0 && <span className="count">{data.trash_count}</span>}
          </button>
          <button className="rail-item" onClick={() => setModal({ kind: "settings" })}>
            <Gear size={15} /> {t.settings}
          </button>
        </div>
      </aside>

      <section className="main">
        <div className="topbar">
          <label className="search">
            <Search />
            <span className="sr-only">{t.search}</span>
            <input
              ref={searchRef}
              className="input"
              placeholder={t.search}
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              onKeyDown={(e) => e.key === "Escape" && setQuery("")}
            />
          </label>
          <span className="kbd" aria-hidden="true">Ctrl F</span>
          <span className="grow" />
          <button
            className="btn"
            onClick={() =>
              setModal({ kind: "account", account: null, institutionId: filter.kind === "institution" ? filter.id : null })
            }
            disabled={institutions.length === 0 && accounts.length === 0}
          >
            <Plus /> {t.addAccount}
          </button>
          <button className="btn" onClick={lock} title="Ctrl L">
            <LockIcon /> {t.lockNow}
          </button>
        </div>

        <div className="content">
          {data?.backup_due && (
            <div className="banner">
              <span className="grow">{t.backupDue}</span>
              <button className="btn small" onClick={() => setModal({ kind: "settings", section: "backup" })}>
                {t.backupNow}
              </button>
            </div>
          )}

          {data && institutions.length === 0 && accounts.length === 0 && (
            <div className="empty">
              <h3>{t.emptyBoardTitle}</h3>
              <p>{t.emptyBoardLead}</p>
              <button className="btn primary" onClick={() => setModal({ kind: "institution", institution: null })}>
                <Plus /> {t.addInstitution}
              </button>
            </div>
          )}

          {data && query && matches.length === 0 && <p className="muted">{t.noResults}</p>}

          {boards.map(({ inst, list }) => (
            <section className="board" key={inst?.id ?? "none"} aria-label={inst?.name ?? t.noInstitution}>
              <div className="board-head">
                <h3>{inst?.name ?? t.noInstitution}</h3>
                {inst && <span className="url">{inst.login_url}</span>}
                {inst && (
                  <div className="row">
                    <button className="btn ghost small" onClick={() => setModal({ kind: "institution", institution: inst })}>
                      {t.edit}
                    </button>
                    <button
                      className="btn small board-add"
                      title={t.addAccount}
                      aria-label={`${t.addAccount}: ${inst.name}`}
                      onClick={() => setModal({ kind: "account", account: null, institutionId: inst.id })}
                    >
                      <Plus size={14} />
                    </button>
                  </div>
                )}
              </div>
              {list.length === 0 ? (
                <p className="muted small">{t.emptyAccounts}</p>
              ) : (
                <div className="hooks">
                  {list.map((a) => (
                    <div className="tag-wrap" key={a.id}>
                      <div className="tag">
                        <button
                          className="btn ghost small tag-edit"
                          title={t.edit}
                          aria-label={`${t.edit}: ${a.label}`}
                          onClick={() => setModal({ kind: "account", account: a, institutionId: a.institution_id })}
                        >
                          <Pencil size={14} />
                        </button>
                        <span className="tag-label">{a.label}</span>
                        <span className="tag-user">{a.username || "—"}</span>
                        {a.tags.length > 0 && (
                          <div className="tag-chips">
                            {a.tags.slice(0, 3).map((tg) => <span className="chip" key={tg}>{tg}</span>)}
                          </div>
                        )}
                        <div className="tag-foot">
                          <span className="tag-used" title={a.last_used_at ? `${t.lastUsed}: ${formatDate(a.last_used_at, lang)}` : t.never}>
                            {a.last_used_at ? shortDate(a.last_used_at, lang) : ""}
                          </span>
                          <CardMenu
                            account={a}
                            notify={notify}
                            onDeleted={reload}
                          />
                          <button className="btn brass small" disabled={opening === a.id} onClick={() => masuk(a)}>
                            {opening === a.id ? t.opening : t.masuk}
                          </button>
                        </div>
                      </div>
                    </div>
                  ))}
                </div>
              )}
            </section>
          ))}
        </div>
      </section>

      {modal?.kind === "institution" && (
        <InstitutionForm
          institution={modal.institution}
          notify={notify}
          onClose={() => setModal(null)}
          onSaved={(id) => {
            setModal(null);
            reload();
            if (id) setFilter({ kind: "institution", id });
            else setFilter({ kind: "all" });
          }}
        />
      )}
      {modal?.kind === "account" && (
        <AccountForm
          account={modal.account}
          institutionId={modal.institutionId}
          institutions={institutions}
          notify={notify}
          onClose={() => setModal(null)}
          onSaved={() => {
            setModal(null);
            reload();
          }}
        />
      )}
      {modal?.kind === "generator" && <GeneratorDialog onClose={() => setModal(null)} notify={notify} />}
      {modal?.kind === "trash" && <TrashDialog onClose={() => { setModal(null); reload(); }} lang={lang} notify={notify} />}
      {modal?.kind === "settings" && (
        <SettingsDialog
          initial={modal.section}
          notify={notify}
          onClose={() => {
            setModal(null);
            reload();
          }}
          onLanguage={(l) => {
            setLang(l);
            props.onLanguage(l);
          }}
          onLocked={props.onLock}
        />
      )}
    </div>
  );
}
