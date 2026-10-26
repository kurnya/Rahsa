// Messages between the content script / popup and the background worker.

export type VaultState = "offline" | "unpaired" | "pairing" | "locked" | "ready";

export interface AccountHint {
  id: string;
  label: string;
  username_hint: string;
}

export interface Credential {
  username: string;
  password: string;
  custom_fields: { name: string; value: string; selector: string | null }[];
  auto_submit: boolean;
}

export type ToBackground =
  | { kind: "accounts" }
  | { kind: "fill"; account_id: string }
  | { kind: "page_ready" }
  | { kind: "unlock" }
  | { kind: "open_app" }
  | { kind: "report"; ok: boolean; reason?: string; account_id?: string }
  // popup only
  | { kind: "popup_state"; tab_id: number; url: string }
  | { kind: "pair_start" }
  | { kind: "popup_fill"; tab_id: number; url: string; account_id: string }
  | { kind: "grant_done" };

export type ToContent =
  | { kind: "open_dropdown" }
  | { kind: "fill_now"; credential: Credential; submit: boolean; account_id: string }
  | { kind: "state_changed"; state: VaultState };

export interface AccountsReply {
  state: VaultState;
  accounts: AccountHint[];
}

export interface FillReply {
  ok: boolean;
  error?: string;
  credential?: Credential;
}

export interface PageReadyReply {
  fill?: Credential;
  submit?: boolean;
  account_id?: string;
}

export interface PopupState {
  state: VaultState;
  pair_code: string | null;
  origin: string | null;
  accounts: AccountHint[];
  missing_permissions: string[];
  error: string | null;
}
