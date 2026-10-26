import { invoke } from "@tauri-apps/api/core";

export type MatchMode = "exact" | "subdomain";

export interface Origin {
  scheme: string;
  host: string;
  port: number | null;
  match: MatchMode;
}

export interface Institution {
  id: string;
  name: string;
  login_url: string;
  origins: Origin[];
  icon: string | null;
  open_incognito: boolean;
  fresh_session: boolean;
}

export interface CustomFieldMeta {
  name: string;
  value: string;
  selector: string | null;
  secret: boolean;
}

export interface AccountMeta {
  id: string;
  institution_id: string | null;
  label: string;
  username: string;
  origins: Origin[];
  notes: string;
  custom_fields: CustomFieldMeta[];
  tags: string[];
  auto_submit: boolean;
  created_at: number;
  updated_at: number;
  last_used_at: number | null;
  password_changed_at: number | null;
  history_count: number;
}

export interface Overview {
  institutions: Institution[];
  accounts: AccountMeta[];
  trash_count: number;
  backup_due: boolean;
}

export interface AppStatus {
  vault_exists: boolean;
  unlocked: boolean;
  vault_path: string;
  retry_after_secs: number;
  language: "id" | "en";
}

export interface GeneratorOptions {
  length: number;
  lower: boolean;
  upper: boolean;
  digits: boolean;
  symbols: boolean;
  min_digits: number;
  min_symbols: number;
  min_upper: number;
  min_lower: number;
  avoid_ambiguous: boolean;
  symbol_set: string | null;
}

export interface Settings {
  auto_lock_minutes: number;
  lock_on_screen_lock: boolean;
  clipboard_clear_seconds: number;
  hello_enabled: boolean;
  generator_defaults: GeneratorOptions;
  language: "id" | "en";
  last_backup_at: number | null;
}

export interface OriginInput {
  url: string;
  match_mode: MatchMode;
}

export interface InstitutionInput {
  id: string | null;
  name: string;
  login_url: string;
  extra_origins: OriginInput[];
  icon: string | null;
  open_incognito: boolean;
  fresh_session: boolean;
}

export interface CustomFieldInput {
  name: string;
  value: string;
  selector: string | null;
  secret: boolean;
}

export interface AccountInput {
  id: string | null;
  institution_id: string | null;
  label: string;
  username: string;
  password: string | null;
  origins: OriginInput[];
  notes: string;
  custom_fields: CustomFieldInput[];
  tags: string[];
  auto_submit: boolean;
}

export interface TrashEntry {
  account: AccountMeta;
  deleted_at: number;
}

export interface Pairing {
  extension_id: string;
  paired_at: number;
  last_seen_at: number | null;
  connected: boolean;
}

export interface ExtensionInfo {
  extension_id: string;
  host_registered: boolean;
  connected: boolean;
  chrome_found: boolean;
  pending_code: string | null;
}

export interface Toast {
  kind: "info" | "success" | "warning" | "error";
  message: string;
  action: { kind: "incognito"; account_id: string } | null;
}

export const api = {
  status: () => invoke<AppStatus>("app_status"),
  createVault: (password: string) => invoke<void>("create_vault", { password }),
  unlock: (password: string) => invoke<void>("unlock", { password }),
  lock: () => invoke<void>("lock"),
  overview: () => invoke<Overview>("overview"),
  revealPassword: (id: string) => invoke<string>("reveal_password", { id }),
  revealCustomField: (id: string, name: string) => invoke<string>("reveal_custom_field", { id, name }),
  passwordHistory: (id: string) => invoke<{ password: string; changed_at: number }[]>("password_history", { id }),
  saveInstitution: (input: InstitutionInput) => invoke<string>("save_institution", { input }),
  deleteInstitution: (id: string) => invoke<void>("delete_institution", { id }),
  saveAccount: (input: AccountInput) => invoke<string>("save_account", { input }),
  deleteAccount: (id: string) => invoke<void>("delete_account", { id }),
  listTrash: () => invoke<TrashEntry[]>("list_trash"),
  restoreTrash: (id: string) => invoke<void>("restore_trash", { id }),
  purgeTrashItem: (id: string) => invoke<void>("purge_trash_item", { id }),
  generatePassword: (options: GeneratorOptions) => invoke<string>("generate_password", { options }),
  copySecret: (id: string, field: "username" | "password") => invoke<number>("copy_secret", { id, field }),
  copyGenerated: (text: string) => invoke<number>("copy_generated", { text }),
  masuk: (accountId: string, incognito?: boolean) => invoke<void>("masuk", { accountId, incognito }),
  getSettings: () => invoke<Settings>("get_settings"),
  updateSettings: (settings: Settings) => invoke<void>("update_settings", { settings }),
  exportBackup: (path: string, masterPassword: string) => invoke<void>("export_backup", { path, masterPassword }),
  restoreBackup: (path: string, masterPassword: string) => invoke<number>("restore_backup", { path, masterPassword }),
  changeMasterPassword: (old: string, next: string) => invoke<void>("change_master_password", { old, new: next }),
  saveEmergencyKit: (path: string) => invoke<void>("save_emergency_kit", { path }),
  listPairings: () => invoke<Pairing[]>("list_pairings"),
  revokePairing: (extensionId: string) => invoke<void>("revoke_pairing", { extensionId }),
  respondPairing: (approve: boolean) => invoke<void>("respond_pairing", { approve }),
  extensionInfo: () => invoke<ExtensionInfo>("extension_info"),
};

export function errorText(e: unknown): string {
  return typeof e === "string" ? e : e instanceof Error ? e.message : String(e);
}

export function originLabel(o: Origin): string {
  return o.port && o.port !== 443 ? `${o.host}:${o.port}` : o.host;
}
