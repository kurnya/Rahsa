// Content script: finds the login form, shows the Rahsa icon in the username
// field, and fills only after the user picks an account (PRD §3C).
//
// Rules enforced here (the app enforces the origin rules again):
//  1. https only.
//  2. Fill only after a user action (click, keyboard) or a one-shot intent
//     created by clicking "Masuk" in the app.
//  3. Never fill hidden/invisible fields or inside a cross-origin iframe.
//  4. Auto-submit only when nothing else is left empty, there is no captcha,
//     and the origin has not changed since the dropdown opened.

import type { AccountHint, AccountsReply, Credential, FillReply, PageReadyReply, ToContent, VaultState } from "./messages";
import { findLoginFields, hasCaptcha, isVisible, otherEmptyFields, submitControl, type LoginFields } from "./form";

declare global {
  interface Window {
    __rahsaLoaded?: boolean;
  }
}

(function main() {
  if (window.__rahsaLoaded) return;
  window.__rahsaLoaded = true;
  if (location.protocol !== "https:") return;
  if (window !== window.top) {
    try {
      // Throws for cross-origin frames.
      if (window.top!.location.origin !== location.origin) return;
    } catch {
      return;
    }
  }

  const send = <T>(msg: unknown) => chrome.runtime.sendMessage(msg) as Promise<T>;

  let fields: LoginFields | null = null;
  let host: HTMLElement | null = null;
  let shadow: ShadowRoot | null = null;
  let menuOpen = false;
  let openedOrigin = location.origin;
  let hasAccounts = false;

  // ------------------------------------------------------------------ fill

  const setValue = (el: HTMLInputElement, value: string) => {
    el.focus();
    const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")!.set!;
    setter.call(el, value);
    el.dispatchEvent(new Event("input", { bubbles: true }));
    el.dispatchEvent(new Event("change", { bubbles: true }));
  };

  /** Returns a failure reason, or null on success. */
  function fill(cred: Credential, submit: boolean): string | null {
    if (location.origin !== openedOrigin) return "origin_changed";
    const f = findLoginFields(document);
    if (!f) return "no_form";
    if (!isVisible(f.password) || (f.username && !isVisible(f.username))) return "hidden_field";
    if (f.username) setValue(f.username, cred.username);
    setValue(f.password, cred.password);
    for (const cf of cred.custom_fields) {
      if (!cf.selector) continue;
      const el = document.querySelector(cf.selector);
      if (el instanceof HTMLInputElement && isVisible(el)) setValue(el, cf.value);
      else if (el instanceof HTMLSelectElement && isVisible(el)) {
        el.value = cf.value;
        el.dispatchEvent(new Event("change", { bubbles: true }));
      }
    }
    cred.password = "";
    if (!submit) return null;
    if (hasCaptcha(document, f)) return "captcha";
    if (otherEmptyFields(f).length > 0) return "other_fields";
    if (location.origin !== openedOrigin) return "origin_changed";
    const control = submitControl(f);
    if (control) control.click();
    else if (f.form) f.form.requestSubmit();
    else return "no_submit";
    return null;
  }

  // -------------------------------------------------------------- dropdown

  const CSS = `
    :host { all: initial; }
    .icon { position: fixed; width: 22px; height: 22px; border-radius: 6px; border: 0; padding: 0; cursor: pointer;
      background: transparent; display: grid; place-items: center; box-shadow: 0 1px 3px rgba(0,0,0,.35); border-radius: 50%; z-index: 2147483647; }
    .icon:focus-visible { outline: 2px solid #2f5fd0; outline-offset: 2px; }
    .menu { position: fixed; min-width: 240px; max-width: 340px; background: #fff; color: #1c2b4d; border-radius: 10px;
      border: 1px solid #d3d9e1; box-shadow: 0 16px 40px -12px rgba(0,0,0,.45); padding: 6px; z-index: 2147483647;
      font: 14px/1.35 "Segoe UI", system-ui, sans-serif; }
    .head { font-size: 12px; color: #5a667a; padding: 6px 8px 4px; display: flex; align-items: center; gap: 6px; }
    button.item { all: unset; display: grid; gap: 1px; width: 100%; box-sizing: border-box; padding: 8px 10px; border-radius: 7px; cursor: pointer; }
    button.item:hover, button.item:focus-visible { background: #f8ecd0; }
    .label { font-weight: 600; }
    .hint { font-size: 12px; color: #5a667a; }
    .msg { padding: 8px 10px; color: #5a667a; }
    .action { all: unset; display: block; margin: 4px 6px 6px; padding: 7px 10px; text-align: center; border-radius: 7px;
      background: #1c2b4d; color: #eef0f5; cursor: pointer; font-weight: 600; }
    @media (prefers-color-scheme: dark) {
      .menu { background: #132039; color: #e7ecf2; border-color: #283a60; }
      .head, .hint, .msg { color: #98a4b5; }
      button.item:hover, button.item:focus-visible { background: #3a3120; }
      .action { background: #e7ecf2; color: #10161f; }
    }`;

  const ICON_SVG = `<svg width="22" height="22" viewBox="172 172 680 680" aria-hidden="true"><path fill="#C58A2A" fill-rule="evenodd" d="M172 512A340 340 0 1 0 852 512A340 340 0 1 0 172 512ZM372 343.0A43.0 43.0 0 0 1 458 343.0V681.0A43.0 43.0 0 0 1 372 681.0ZM494 300H588.0A112.0 112.0 0 0 1 588.0 524H494A8 8 0 0 1 486 516V308A8 8 0 0 1 494 300ZM541.3 412.0A40 40 0 1 0 621.3 412.0A40 40 0 1 0 541.3 412.0ZM532.09 559.96A111.1 46 47.6 1 1 681.91 724.04A111.1 46 47.6 1 1 532.09 559.96Z"/></svg>`;

  function ensureHost() {
    if (host && host.isConnected) return;
    host = document.createElement("rahsa-ui");
    shadow = host.attachShadow({ mode: "closed" });
    const style = document.createElement("style");
    style.textContent = CSS;
    shadow.append(style);
    document.documentElement.append(host);
  }

  function anchor(): HTMLInputElement | null {
    return fields?.username ?? fields?.password ?? null;
  }

  function placeIcon() {
    const el = anchor();
    if (!el || !hasAccounts || !isVisible(el)) {
      shadow?.querySelector(".icon")?.remove();
      return;
    }
    ensureHost();
    let icon = shadow!.querySelector<HTMLButtonElement>(".icon");
    if (!icon) {
      icon = document.createElement("button");
      icon.className = "icon";
      icon.title = "Isi dengan Rahsa (Ctrl+Shift+L)";
      icon.setAttribute("aria-label", "Pilih akun Rahsa");
      icon.innerHTML = ICON_SVG;
      icon.addEventListener("mousedown", (e) => e.preventDefault());
      icon.addEventListener("click", (e) => {
        if (!e.isTrusted) return;
        toggleMenu();
      });
      shadow!.append(icon);
    }
    const r = el.getBoundingClientRect();
    icon.style.left = `${r.right - 28}px`;
    icon.style.top = `${r.top + (r.height - 22) / 2}px`;
  }

  function closeMenu() {
    shadow?.querySelector(".menu")?.remove();
    menuOpen = false;
  }

  async function toggleMenu() {
    if (menuOpen) return closeMenu();
    await openMenu();
  }

  async function openMenu() {
    fields = findLoginFields(document) ?? fields;
    const el = anchor();
    if (!el) return;
    ensureHost();
    closeMenu();
    menuOpen = true;
    openedOrigin = location.origin;
    const menu = document.createElement("div");
    menu.className = "menu";
    menu.setAttribute("role", "menu");
    const r = el.getBoundingClientRect();
    menu.style.left = `${Math.max(8, Math.min(r.left, innerWidth - 348))}px`;
    menu.style.top = `${Math.min(r.bottom + 6, innerHeight - 120)}px`;
    shadow!.append(menu);

    const head = document.createElement("div");
    head.className = "head";
    head.textContent = "Rahsa";
    menu.append(head);

    const reply = await send<AccountsReply>({ kind: "accounts" }).catch(() => ({ state: "offline" as VaultState, accounts: [] }));
    renderMenu(menu, reply);
  }

  function message(menu: HTMLElement, text: string) {
    const m = document.createElement("div");
    m.className = "msg";
    m.textContent = text;
    menu.append(m);
  }

  function renderMenu(menu: HTMLElement, reply: AccountsReply) {
    if (reply.state === "locked") {
      message(menu, "Vault Rahsa terkunci.");
      const b = document.createElement("button");
      b.className = "action";
      b.textContent = "Buka kunci";
      b.addEventListener("click", (e) => {
        if (!e.isTrusted) return;
        closeMenu();
        void send({ kind: "unlock" });
      });
      menu.append(b);
      b.focus();
      return;
    }
    if (reply.state !== "ready") {
      message(menu, reply.state === "offline" ? "Buka aplikasi Rahsa dulu, lalu coba lagi." : "Ekstensi belum dipasangkan. Klik ikon Rahsa di toolbar.");
      return;
    }
    if (reply.accounts.length === 0) {
      message(menu, "Belum ada akun untuk situs ini.");
      const b = document.createElement("button");
      b.className = "action";
      b.textContent = "Tambah akun untuk situs ini";
      b.addEventListener("click", (e) => e.isTrusted && (closeMenu(), void send({ kind: "open_app" })));
      menu.append(b);
      return;
    }
    const items: HTMLButtonElement[] = reply.accounts.map((a: AccountHint) => {
      const b = document.createElement("button");
      b.className = "item";
      b.setAttribute("role", "menuitem");
      const l = document.createElement("span");
      l.className = "label";
      l.textContent = a.label;
      const h = document.createElement("span");
      h.className = "hint";
      h.textContent = a.username_hint;
      b.append(l, h);
      b.addEventListener("click", (e) => {
        if (!e.isTrusted) return; // synthetic clicks from page scripts can't trigger a fill
        void pick(a.id);
      });
      menu.append(b);
      return b;
    });
    items[0]?.focus();
    menu.addEventListener("keydown", (e) => {
      const i = items.indexOf(shadow!.activeElement as HTMLButtonElement);
      if (e.key === "ArrowDown") items[(i + 1) % items.length].focus();
      else if (e.key === "ArrowUp") items[(i - 1 + items.length) % items.length].focus();
      else if (e.key === "Escape") {
        closeMenu();
        anchor()?.focus();
      } else return;
      e.preventDefault();
    });
  }

  async function pick(accountId: string) {
    closeMenu();
    const r = await send<FillReply>({ kind: "fill", account_id: accountId });
    if (!r.ok || !r.credential) return;
    const reason = fill(r.credential, r.credential.auto_submit);
    if (reason && reason !== "other_fields") void send({ kind: "report", ok: false, reason, account_id: accountId });
  }

  // ---------------------------------------------------------- detection

  async function refresh() {
    const f = findLoginFields(document);
    if (!f) {
      fields = null;
      placeIcon();
      return;
    }
    const changed = !fields || fields.password !== f.password;
    fields = f;
    if (changed) {
      const reply = await send<AccountsReply>({ kind: "accounts" }).catch(() => null);
      hasAccounts = !!reply && (reply.state !== "ready" || reply.accounts.length > 0);
    }
    placeIcon();
  }

  let scheduled = false;
  const schedule = () => {
    if (scheduled) return;
    scheduled = true;
    setTimeout(() => {
      scheduled = false;
      void refresh();
    }, 250);
  };
  new MutationObserver(schedule).observe(document.documentElement, { childList: true, subtree: true, attributes: true, attributeFilter: ["type", "style", "class", "hidden"] });
  addEventListener("scroll", () => placeIcon(), { passive: true, capture: true });
  addEventListener("resize", () => placeIcon(), { passive: true });
  document.addEventListener("mousedown", (e) => {
    if (menuOpen && e.target !== host) closeMenu();
  });

  // ------------------------------------------------- "Masuk" from the app

  async function waitForForm(ms: number): Promise<LoginFields | null> {
    const end = Date.now() + ms;
    while (Date.now() < end) {
      const f = findLoginFields(document);
      if (f) return f;
      await new Promise((r) => setTimeout(r, 300));
    }
    return null;
  }

  async function onPageReady() {
    if (window !== window.top) return;
    const r = await send<PageReadyReply>({ kind: "page_ready" }).catch(() => ({}) as PageReadyReply);
    if (!r.fill || !r.account_id) return;
    openedOrigin = location.origin;
    const f = await waitForForm(10_000);
    if (!f) return void send({ kind: "report", ok: false, reason: "no_form", account_id: r.account_id });
    const reason = fill(r.fill, !!r.submit);
    void send({ kind: "report", ok: !reason, reason: reason ?? undefined, account_id: r.account_id });
  }

  chrome.runtime.onMessage.addListener((msg: ToContent & { origin?: string }) => {
    if (msg.kind === "open_dropdown") {
      if (window === window.top) void refresh().then(openMenu);
    } else if (msg.kind === "fill_now") {
      if (window !== window.top || msg.origin !== location.origin) return;
      openedOrigin = location.origin;
      const reason = fill(msg.credential, msg.submit);
      if (reason && reason !== "other_fields") void send({ kind: "report", ok: false, reason, account_id: msg.account_id });
    } else if (msg.kind === "state_changed") {
      closeMenu();
      fields = null;
      void refresh();
    }
  });

  void refresh();
  void onPageReady();
})();
