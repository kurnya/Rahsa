// Toolbar popup: vault status, pairing, accounts for the active tab (PRD §10).

import type { PopupState, ToBackground } from "./messages";

const view = document.getElementById("view")!;
const status = document.getElementById("status")!;
const send = <T>(msg: ToBackground) => chrome.runtime.sendMessage(msg) as Promise<T>;

let tab: chrome.tabs.Tab | undefined;
let poll: ReturnType<typeof setInterval> | undefined;

function el<K extends keyof HTMLElementTagNameMap>(tag: K, props: Partial<HTMLElementTagNameMap[K]> = {}, ...children: (Node | string)[]) {
  const e = Object.assign(document.createElement(tag), props);
  e.append(...children);
  return e;
}

function button(text: string, cls: string, onClick: () => void) {
  return el("button", { className: `btn ${cls}`, textContent: text, onclick: onClick });
}

async function render() {
  [tab] = await chrome.tabs.query({ active: true, currentWindow: true });
  const s = await send<PopupState>({ kind: "popup_state", tab_id: tab?.id ?? -1, url: tab?.url ?? "" });
  view.replaceChildren();
  status.textContent = { offline: "Tidak tersambung", unpaired: "Belum dipasangkan", pairing: "Menunggu konfirmasi", locked: "Terkunci", ready: "Terbuka" }[s.state];

  if (s.state === "offline") {
    view.append(
      el("p", { textContent: "Aplikasi Rahsa sedang tidak dibuka." }),
      el("p", { className: "muted", textContent: "Buka aplikasi Rahsa di Windows, lalu klik Coba lagi. Ekstensi hanya bekerja selama aplikasinya terbuka." }),
      button("Coba lagi", "primary", render),
    );
    return;
  }

  if (s.state === "unpaired") {
    view.append(
      el("p", { textContent: "Pasangkan ekstensi ini dengan aplikasi Rahsa di laptop Anda." }),
      el("p", { className: "muted", textContent: "Kode 6 digit akan tampil di sini dan di aplikasi. Setujui hanya jika keduanya sama." }),
      button("Pasangkan", "primary", async () => {
        await send({ kind: "pair_start" });
        render();
      }),
    );
    return;
  }

  if (s.state === "pairing") {
    view.append(
      el("p", { textContent: "Cocokkan kode ini dengan yang tampil di aplikasi Rahsa:" }),
      el("div", { className: "code", textContent: s.pair_code ? `${s.pair_code.slice(0, 3)} ${s.pair_code.slice(3)}` : "…" }),
      el("p", { className: "muted", textContent: "Jika sama, klik “Kodenya sama, pasangkan” di aplikasi. Jika berbeda, tolak." }),
    );
    poll ??= setInterval(render, 1000);
    return;
  }
  if (poll) {
    clearInterval(poll);
    poll = undefined;
  }

  if (s.state === "locked") {
    view.append(
      el("p", { textContent: "Vault Rahsa terkunci." }),
      button("Buka kunci", "primary", async () => {
        await send({ kind: "unlock" });
        window.close();
      }),
    );
    return;
  }

  if (s.missing_permissions.length > 0) {
    const n = s.missing_permissions.length;
    view.append(
      el(
        "div",
        { className: "notice" },
        `Izinkan Rahsa membaca halaman login di ${n} situs institusi agar ikon akun muncul otomatis.`,
        button("Izinkan", "brass", async () => {
          const ok = await chrome.permissions.request({ origins: s.missing_permissions });
          if (ok) await send({ kind: "grant_done" });
          render();
        }),
      ),
    );
  }

  if (!s.origin) {
    view.append(el("p", { className: "muted", textContent: "Buka halaman login web institusi (https) untuk melihat akun." }));
    return;
  }

  view.append(el("div", { className: "origin", textContent: s.origin }));
  if (s.accounts.length === 0) {
    view.append(el("p", { className: "muted", textContent: "Belum ada akun untuk situs ini." }));
  }
  for (const a of s.accounts) {
    view.append(
      el(
        "div",
        { className: "account" },
        el("div", { className: "who" }, el("div", { className: "label", textContent: a.label }), el("div", { className: "hint", textContent: a.username_hint })),
        button("Isi", "brass", async () => {
          if (!tab?.id || !tab.url) return;
          await send({ kind: "popup_fill", tab_id: tab.id, url: tab.url, account_id: a.id });
          window.close();
        }),
      ),
    );
  }
  view.append(button("Tambah akun untuk situs ini", "", () => void send({ kind: "open_app" })));
}

void render();
