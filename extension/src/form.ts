// Login-form detection (F-09). Pure DOM logic, no extension APIs, so it can be
// exercised against trap pages in tests.

export interface LoginFields {
  password: HTMLInputElement;
  username: HTMLInputElement | null;
  form: HTMLFormElement | null;
}

const USERNAME_TYPES = new Set(["text", "email", "tel", "number", ""]);

/** Visible to a human: rendered, non-zero size, not transparent, on screen-ish. */
export function isVisible(el: Element): boolean {
  if (!(el instanceof HTMLElement)) return false;
  if (el.hidden || (el instanceof HTMLInputElement && el.type === "hidden")) return false;
  if (el instanceof HTMLInputElement && el.disabled) return false;
  const rects = el.getClientRects();
  if (rects.length === 0) return false;
  const r = el.getBoundingClientRect();
  if (r.width < 4 || r.height < 4) return false;
  // Far off-canvas tricks (left: -9999px).
  if (r.right < 0 || r.bottom < 0 || r.left > innerWidth + 2000 || r.top > document.documentElement.scrollHeight + 2000) return false;
  for (let n: Element | null = el; n; n = n.parentElement) {
    const s = getComputedStyle(n);
    if (s.display === "none" || s.visibility === "hidden" || s.visibility === "collapse") return false;
    if (parseFloat(s.opacity) < 0.1) return false;
    if (s.clipPath && s.clipPath !== "none" && /inset\(\s*50%|circle\(\s*0/.test(s.clipPath)) return false;
  }
  const cs = getComputedStyle(el);
  if (cs.color === cs.backgroundColor && cs.color !== "rgba(0, 0, 0, 0)") return false;
  return true;
}

function inputs(root: ParentNode): HTMLInputElement[] {
  return [...root.querySelectorAll("input")];
}

export function findLoginFields(doc: Document): LoginFields | null {
  const passwords = inputs(doc).filter((i) => i.type === "password" && isVisible(i));
  // Exactly the login case: one visible password field (registration and
  // change-password forms have two or three; we leave those alone).
  const candidates = passwords.filter((p) => {
    const scope = p.form ?? doc;
    return inputs(scope).filter((i) => i.type === "password" && isVisible(i)).length === 1;
  });
  const password = candidates[0];
  if (!password) return null;
  const form = password.form;
  const scope: ParentNode = form ?? doc;
  const all = inputs(scope);
  const idx = all.indexOf(password);
  let username: HTMLInputElement | null = null;
  // Nearest visible text-like input *before* the password field.
  for (let i = idx - 1; i >= 0; i--) {
    const c = all[i];
    if (USERNAME_TYPES.has(c.type) && isVisible(c)) {
      username = c;
      break;
    }
  }
  return { password, username, form };
}

const CAPTCHA_RE = /captcha|recaptcha|hcaptcha|turnstile|kode\s*keamanan|security\s*code/i;

export function hasCaptcha(doc: Document, f: LoginFields): boolean {
  const scope: ParentNode = f.form ?? doc;
  if ([...doc.querySelectorAll("iframe")].some((fr) => CAPTCHA_RE.test(fr.src) || CAPTCHA_RE.test(fr.title))) return true;
  if (doc.querySelector(".g-recaptcha, .h-captcha, .cf-turnstile, [data-sitekey]")) return true;
  for (const el of scope.querySelectorAll("input, img")) {
    const attrs = [el.id, el.getAttribute("name"), el.getAttribute("placeholder"), el.getAttribute("alt"), el.getAttribute("src"), el.className]
      .filter(Boolean)
      .join(" ");
    if (CAPTCHA_RE.test(attrs) && isVisible(el)) return true;
  }
  return false;
}

/** Visible, empty, user-editable fields in the form other than the ones we fill. */
export function otherEmptyFields(f: LoginFields): HTMLElement[] {
  if (!f.form) return [];
  const skip = new Set<Element>([f.password, ...(f.username ? [f.username] : [])]);
  const out: HTMLElement[] = [];
  for (const el of f.form.querySelectorAll<HTMLInputElement | HTMLSelectElement | HTMLTextAreaElement>("input, select, textarea")) {
    if (skip.has(el) || !isVisible(el)) continue;
    if (el instanceof HTMLInputElement && ["checkbox", "radio", "submit", "button", "image", "reset", "file", "hidden"].includes(el.type)) continue;
    if (!el.value) out.push(el);
  }
  return out;
}

export function submitControl(f: LoginFields): HTMLElement | null {
  const scope: ParentNode | null = f.form ?? f.password.closest("div, section, main");
  if (!scope) return null;
  const explicit = scope.querySelector<HTMLElement>('button[type="submit"], input[type="submit"], input[type="image"]');
  if (explicit && isVisible(explicit)) return explicit;
  const buttons = [...scope.querySelectorAll<HTMLButtonElement>("button")].filter(
    (b) => isVisible(b) && (!b.getAttribute("type") || b.type === "submit"),
  );
  if (f.form && buttons[0]) return buttons[0];
  // Formless (SPA) pages often use type="button"; accept any visible button
  // whose text says it logs in.
  return (
    [...scope.querySelectorAll<HTMLElement>("button, [role=button], input[type=button]")].find(
      (b) => isVisible(b) && /masuk|login|log in|sign in|submit/i.test(b.textContent || (b as HTMLInputElement).value || ""),
    ) ?? null
  );
}
