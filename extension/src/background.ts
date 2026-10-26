// Background service worker: owns the single native-messaging connection to
// the Rahsa app, the end-to-end session, and the content-script registration.
// It never stores passwords: a credential is requested for one account, for
// the frame that asked, after a user action, and handed straight back.

import {
  AAD_APP_TO_EXT,
  AAD_EXT_TO_APP,
  b64,
  deriveSessionKey,
  newSecret,
  open,
  pairingCode,
  publicKey,
  random,
  seal,
  unb64,
} from "./crypto";
import type {
  AccountHint,
  AccountsReply,
  Credential,
  FillReply,
  PageReadyReply,
  PopupState,
  ToBackground,
  ToContent,
  VaultState,
} from "./messages";

const HOST = "id.rahsa.host";
const SCRIPT_ID = "rahsa-content";
const REQUEST_TIMEOUT_MS = 15_000;

type Json = Record<string, any>;

let port: chrome.runtime.Port | null = null;
let state: VaultState = "offline";
let sessionKey: Uint8Array | null = null;
let pairCode: string | null = null;
let pairingSecret: Uint8Array | null = null;
let pairingAppPub: Uint8Array | null = null;
let lastError: string | null = null;
let connecting: Promise<void> | null = null;
const pending = new Map<string, { resolve: (v: Json) => void; timer: ReturnType<typeof setTimeout> }>();

const rid = () => b64(random(12));

interface Stored {
  ext_secret?: string;
  app_pub?: string;
}

const getStored = () => chrome.storage.local.get(["ext_secret", "app_pub"]) as Promise<Stored>;

// ---------------------------------------------------------------- transport

function post(msg: Json) {
  if (!port) throw new Error("offline");
  port.postMessage(msg);
}

function waitFor(id: string): Promise<Json> {
  return new Promise((resolve) => {
    const timer = setTimeout(() => {
      pending.delete(id);
      resolve({ type: "error", error: "timeout" });
    }, REQUEST_TIMEOUT_MS);
    pending.set(id, { resolve, timer });
  });
}

function settle(id: string, value: Json) {
  const p = pending.get(id);
  if (!p) return;
  clearTimeout(p.timer);
  pending.delete(id);
  p.resolve(value);
}

async function plain(req: Json): Promise<Json> {
  const id = rid();
  const reply = waitFor(id);
  post({ ...req, id });
  return reply;
}

async function request(req: Json): Promise<Json> {
  await ensureConnected();
  if (!sessionKey) return { type: "error", error: state };
  const id = rid();
  const reply = waitFor(id);
  const sealed = seal(sessionKey, AAD_EXT_TO_APP, JSON.stringify({ ...req, id, nonce: b64(random(18)) }));
  post({ type: "enc", ...sealed });
  return reply;
}

function setState(s: VaultState) {
  if (state === s) return;
  state = s;
  updateActionIcon();
  broadcast({ kind: "state_changed", state: s });
  if (s === "ready") void syncContentScripts();
}

function onFrame(msg: Json) {
  if (msg.type === "enc" && sessionKey) {
    let inner: Json;
    try {
      inner = JSON.parse(open(sessionKey, AAD_APP_TO_EXT, msg.n, msg.c));
    } catch {
      return; // forged or corrupted frame: ignore
    }
    if (inner.id) return settle(inner.id, inner);
    if (inner.type === "locked") setState("locked");
    else if (inner.type === "unlocked") setState("ready");
    else if (inner.type === "vault_changed") void syncContentScripts();
    else if (inner.type === "open_login") void openLogin(inner.url, inner.origins, inner.patterns, !!inner.fresh);
    return;
  }
  if (msg.type === "pair_result") return void finishPairing(!!msg.ok);
  if (msg.type === "host_error") {
    lastError = msg.error;
    return;
  }
  if (msg.type === "error" && msg.error === "not_paired" && !msg.id) {
    void chrome.storage.local.remove(["ext_secret", "app_pub"]);
    sessionKey = null;
    setState("unpaired");
    return;
  }
  if (msg.id) settle(msg.id, msg);
}

function disconnect() {
  lastError = chrome.runtime.lastError?.message ?? lastError;
  port = null;
  sessionKey?.fill(0);
  sessionKey = null;
  pairCode = null;
  for (const [id] of pending) settle(id, { type: "error", error: "offline" });
  setState("offline");
}

async function connect(): Promise<void> {
  lastError = null;
  try {
    port = chrome.runtime.connectNative(HOST);
  } catch (e) {
    lastError = String(e);
    return;
  }
  port.onMessage.addListener(onFrame);
  port.onDisconnect.addListener(disconnect);

  const hello = await plain({ type: "hello", protocol: 1 });
  if (hello.type !== "hello") return;
  const stored = await getStored();
  if (hello.paired && stored.ext_secret && stored.app_pub) {
    await startSession(stored);
  } else {
    setState("unpaired");
  }
}

async function ensureConnected(): Promise<void> {
  if (port && state !== "offline") return;
  connecting ??= connect().finally(() => (connecting = null));
  await connecting;
}

async function startSession(stored: Stored) {
  const extSecret = unb64(stored.ext_secret!);
  const eph = newSecret();
  const nonce = random(16);
  const r = await plain({
    type: "session_init",
    ext_pub: b64(publicKey(extSecret)),
    eph_pub: b64(publicKey(eph)),
    nonce: b64(nonce),
  });
  if (r.type !== "session") {
    if (r.error === "not_paired") {
      await chrome.storage.local.remove(["ext_secret", "app_pub"]);
      setState("unpaired");
    }
    return;
  }
  sessionKey = deriveSessionKey(extSecret, unb64(stored.app_pub!), eph, unb64(r.eph_pub), nonce, unb64(r.nonce));
  eph.fill(0);
  extSecret.fill(0);
  setState(r.state === "unlocked" ? "ready" : "locked");
}

async function startPairing(): Promise<string | null> {
  await ensureConnected();
  if (!port) return null;
  pairingSecret = newSecret();
  const extPub = publicKey(pairingSecret);
  const r = await plain({ type: "pair_start", ext_pub: b64(extPub) });
  if (r.type !== "pair_pending") return null;
  pairingAppPub = unb64(r.app_pub);
  // The code is computed locally from the key we actually received, so a
  // tampered key shows a different code than the app.
  pairCode = pairingCode(extPub, pairingAppPub);
  setState("pairing");
  return pairCode;
}

async function finishPairing(ok: boolean) {
  pairCode = null;
  if (ok && pairingSecret && pairingAppPub) {
    const stored = { ext_secret: b64(pairingSecret), app_pub: b64(pairingAppPub) };
    await chrome.storage.local.set(stored);
    await startSession(stored);
  } else {
    setState("unpaired");
  }
  pairingSecret = null;
  pairingAppPub = null;
}

// ------------------------------------------------------- content scripts

async function syncContentScripts() {
  const r = await request({ type: "list_origins" });
  if (r.type !== "origins") return;
  const patterns: string[] = r.patterns;
  await chrome.storage.local.set({ patterns });
  await reportPermissions(patterns);
  const granted: string[] = [];
  for (const p of patterns) {
    if (await chrome.permissions.contains({ origins: [p] })) granted.push(p);
  }
  const existing = await chrome.scripting.getRegisteredContentScripts({ ids: [SCRIPT_ID] });
  if (granted.length === 0) {
    if (existing.length) await chrome.scripting.unregisterContentScripts({ ids: [SCRIPT_ID] });
    return;
  }
  const def: chrome.scripting.RegisteredContentScript = {
    id: SCRIPT_ID,
    matches: granted,
    js: ["content.js"],
    runAt: "document_idle",
    allFrames: true,
    persistAcrossSessions: true,
  };
  if (existing.length) await chrome.scripting.updateContentScripts([def]);
  else await chrome.scripting.registerContentScripts([def]);
}

async function missingPermissions(): Promise<string[]> {
  const { patterns = [] } = (await chrome.storage.local.get("patterns")) as { patterns?: string[] };
  const out: string[] = [];
  for (const p of patterns) {
    if (!(await chrome.permissions.contains({ origins: [p] }))) out.push(p);
  }
  return out;
}

async function broadcast(msg: ToContent) {
  const tabs = await chrome.tabs.query({ url: "https://*/*" });
  for (const t of tabs) {
    if (t.id !== undefined) chrome.tabs.sendMessage(t.id, msg).catch(() => {});
  }
}

/** Tell the app which sites we may run on, so "Masuk" can explain a missing grant. */
async function reportPermissions(patterns?: string[]) {
  patterns ??= ((await chrome.storage.local.get("patterns")) as { patterns?: string[] }).patterns ?? [];
  const granted: string[] = [];
  for (const p of patterns) {
    if (await chrome.permissions.contains({ origins: [p] })) granted.push(p);
  }
  await request({ type: "permissions", granted });
}

// Granting or revoking in chrome://extensions takes effect without the popup.
chrome.permissions.onAdded.addListener(() => void syncContentScripts());
chrome.permissions.onRemoved.addListener(() => void syncContentScripts());

/**
 * "Masuk" from the app: log the site out by closing its tabs and clearing its
 * cookies/storage, then open the login page in a fresh tab. The content
 * script then picks up the one-shot intent and fills the chosen account.
 */
async function openLogin(url: string, origins: string[], patterns: string[], fresh: boolean) {
  if (!url.startsWith("https://")) return;
  // Open the new tab first so closing the old ones never closes Chrome's last window.
  const tab = await chrome.tabs.create({ url: "about:blank", active: true });
  if (fresh) {
    const old = await chrome.tabs.query({ url: patterns });
    const ids = old.map((t) => t.id).filter((id): id is number => id !== undefined && id !== tab.id);
    if (ids.length) await chrome.tabs.remove(ids);
    // Cookies are cleared for each origin's whole registrable domain.
    if (origins.length === 0) throw new Error("no origins");
    await chrome.browsingData.remove(
      { origins: origins as [string, ...string[]] },
      { cookies: true, localStorage: true, indexedDB: true, cacheStorage: true, serviceWorkers: true, fileSystems: true },
    );
  }
  await chrome.tabs.update(tab.id!, { url });
  if (tab.windowId !== undefined) await chrome.windows.update(tab.windowId, { focused: true });
}

// ------------------------------------------------------------- badge/icon

function updateActionIcon() {
  const locked = state !== "ready";
  void chrome.action.setIcon({
    path: {
      16: `icons/${locked ? "locked-" : ""}16.png`,
      32: `icons/${locked ? "locked-" : ""}32.png`,
    },
  });
  if (locked) void chrome.action.setBadgeText({ text: "" });
}

async function updateBadge(tabId: number, url?: string) {
  if (state !== "ready" || !url?.startsWith("https://")) {
    return chrome.action.setBadgeText({ tabId, text: "" });
  }
  const r = await request({ type: "list_accounts", origin: new URL(url).origin, tab_id: tabId });
  const n = r.type === "accounts" ? r.accounts.length : 0;
  await chrome.action.setBadgeBackgroundColor({ color: "#C58A2A" });
  await chrome.action.setBadgeText({ tabId, text: n ? String(n) : "" });
}

chrome.tabs.onActivated.addListener(async ({ tabId }) => {
  const tab = await chrome.tabs.get(tabId).catch(() => null);
  if (tab) void updateBadge(tabId, tab.url);
});
chrome.tabs.onUpdated.addListener((tabId, info, tab) => {
  if (info.status === "complete") void updateBadge(tabId, tab.url);
});

// --------------------------------------------------------------- messages

async function credentialFor(accountId: string, origin: string, tabId: number): Promise<FillReply> {
  const r = await request({ type: "get_credential", account_id: accountId, origin, tab_id: tabId });
  if (r.type !== "credential") return { ok: false, error: r.error ?? "failed" };
  const credential: Credential = {
    username: r.username,
    password: r.password,
    custom_fields: r.custom_fields ?? [],
    auto_submit: !!r.auto_submit,
  };
  return { ok: true, credential };
}

async function accountsFor(origin: string, tabId: number): Promise<AccountHint[]> {
  const r = await request({ type: "list_accounts", origin, tab_id: tabId });
  return r.type === "accounts" ? r.accounts : [];
}

async function fromContent(msg: ToBackground, sender: chrome.runtime.MessageSender): Promise<unknown> {
  // The origin comes from Chrome, not from the page or the content script.
  const origin = sender.origin;
  const tabId = sender.tab?.id;
  if (!origin?.startsWith("https://") || tabId === undefined) return { state: "offline", accounts: [] };
  await ensureConnected();

  switch (msg.kind) {
    case "accounts":
      return { state, accounts: state === "ready" ? await accountsFor(origin, tabId) : [] } satisfies AccountsReply;
    case "fill":
      return credentialFor(msg.account_id, origin, tabId);
    case "page_ready": {
      if (state !== "ready") return {};
      const r = await request({ type: "pending_login", origin, tab_id: tabId });
      if (!r.account_id) return {};
      const c = await credentialFor(r.account_id, origin, tabId);
      return (c.ok ? { fill: c.credential, submit: !!r.submit, account_id: r.account_id } : {}) satisfies PageReadyReply;
    }
    case "unlock":
      return request({ type: "request_unlock" });
    case "open_app":
      return request({ type: "open_app" });
    case "report":
      return request({ type: "login_report", ok: msg.ok, reason: msg.reason, account_id: msg.account_id, origin });
    default:
      return null;
  }
}

async function injectInto(tabId: number) {
  await chrome.scripting.executeScript({ target: { tabId }, files: ["content.js"] });
}

async function fromPopup(msg: ToBackground): Promise<unknown> {
  switch (msg.kind) {
    case "popup_state": {
      await ensureConnected();
      let origin: string | null = null;
      try {
        const u = new URL(msg.url);
        if (u.protocol === "https:") origin = u.origin;
      } catch {
        /* not a web page */
      }
      const accounts = state === "ready" && origin ? await accountsFor(origin, msg.tab_id) : [];
      return {
        state,
        pair_code: pairCode,
        origin,
        accounts,
        missing_permissions: state === "ready" ? await missingPermissions() : [],
        error: lastError,
      } satisfies PopupState;
    }
    case "pair_start":
      return startPairing();
    case "popup_fill": {
      const origin = new URL(msg.url).origin;
      const r = await credentialFor(msg.account_id, origin, msg.tab_id);
      if (!r.ok || !r.credential) return r;
      await injectInto(msg.tab_id);
      const fill: ToContent & { origin: string } = {
        kind: "fill_now",
        credential: r.credential,
        submit: r.credential.auto_submit,
        account_id: msg.account_id,
        origin,
      };
      await chrome.tabs.sendMessage(msg.tab_id, fill, { frameId: 0 });
      return { ok: true };
    }
    case "unlock":
      return request({ type: "request_unlock" });
    case "open_app":
      return request({ type: "open_app" });
    case "grant_done":
      await syncContentScripts();
      return true;
    default:
      return null;
  }
}

chrome.runtime.onMessage.addListener((msg: ToBackground, sender, sendResponse) => {
  if (sender.id !== chrome.runtime.id) return false;
  const isPopup = !sender.tab && sender.url?.startsWith(chrome.runtime.getURL("popup.html"));
  const work = isPopup ? fromPopup(msg) : fromContent(msg, sender);
  work.then(sendResponse, (e) => sendResponse({ ok: false, error: String(e) }));
  return true;
});

// Ctrl+Shift+L: open the account dropdown on the current page.
chrome.commands.onCommand.addListener(async (command, tab) => {
  if (command !== "fill-login" || tab?.id === undefined) return;
  try {
    await chrome.tabs.sendMessage(tab.id, { kind: "open_dropdown" } satisfies ToContent);
  } catch {
    // No content script on this page yet: activeTab lets us inject once.
    await injectInto(tab.id).catch(() => {});
    await chrome.tabs.sendMessage(tab.id, { kind: "open_dropdown" } satisfies ToContent).catch(() => {});
  }
});

chrome.runtime.onStartup.addListener(() => void ensureConnected());
chrome.runtime.onInstalled.addListener(() => void ensureConnected());
void ensureConnected();
