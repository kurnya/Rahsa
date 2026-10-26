// Trap pages for the form detector (PRD §11 "Uji halaman jebakan").
// Each case swaps the body and records what the detector sees.
import { findLoginFields, hasCaptcha, otherEmptyFields, submitControl } from "../src/form";

interface Case {
  name: string;
  html: string;
  expect: { found: boolean; username?: string | null; captcha?: boolean; otherEmpty?: number; submit?: string | null };
}

const cases: Case[] = [
  {
    name: "normal login",
    html: `<form><input id="u" type="text"><input id="p" type="password"><button id="b">Masuk</button></form>`,
    expect: { found: true, username: "u", captcha: false, otherEmpty: 0, submit: "b" },
  },
  {
    name: "email username, explicit submit",
    html: `<form><input id="u" type="email"><input id="p" type="password"><input id="s" type="submit" value="Login"></form>`,
    expect: { found: true, username: "u", submit: "s" },
  },
  { name: "password display:none", html: `<form><input type="text"><input type="password" style="display:none"></form>`, expect: { found: false } },
  { name: "password visibility:hidden", html: `<form><input type="text"><input type="password" style="visibility:hidden"></form>`, expect: { found: false } },
  { name: "password opacity:0", html: `<form><input type="text"><input type="password" style="opacity:0"></form>`, expect: { found: false } },
  { name: "password off-screen", html: `<form><input type="text"><input type="password" style="position:absolute;left:-9999px"></form>`, expect: { found: false } },
  { name: "password 1x1 px", html: `<form><input type="text"><input type="password" style="width:1px;height:1px;padding:0;border:0"></form>`, expect: { found: false } },
  { name: "parent hidden", html: `<div style="display:none"><form><input type="text"><input type="password"></form></div>`, expect: { found: false } },
  { name: "type=hidden only", html: `<form><input type="hidden" name="password"></form>`, expect: { found: false } },
  {
    name: "registration form (two passwords) is ignored",
    html: `<form><input type="text"><input type="password"><input type="password"></form>`,
    expect: { found: false },
  },
  {
    name: "hidden decoy username is skipped",
    html: `<form><input id="real" type="text"><input id="decoy" type="text" style="display:none"><input type="password"></form>`,
    expect: { found: true, username: "real" },
  },
  {
    name: "recaptcha blocks auto-submit",
    html: `<form><input id="u" type="text"><input type="password"><div class="g-recaptcha" data-sitekey="x"></div><button>Masuk</button></form>`,
    expect: { found: true, username: "u", captcha: true },
  },
  {
    name: "captcha text input",
    html: `<form><input id="u" type="text"><input type="password"><img src="/captcha.php" width="80" height="30"><input name="captcha" type="text"><button>Masuk</button></form>`,
    expect: { found: true, captcha: true },
  },
  {
    name: "extra empty field blocks auto-submit",
    html: `<form><input id="u" type="text"><input type="password"><select id="role"><option value=""></option><option>Operator</option></select><button>Masuk</button></form>`,
    expect: { found: true, otherEmpty: 1 },
  },
  {
    name: "formless SPA login",
    html: `<div><input id="u" type="text"><input type="password"><button id="b" type="button">Login</button></div>`,
    expect: { found: true, username: "u", submit: "b" },
  },
];

const results = cases.map((c) => {
  document.body.innerHTML = c.html;
  const f = findLoginFields(document);
  const got = {
    found: !!f,
    username: f ? (f.username?.id || null) : undefined,
    captcha: f ? hasCaptcha(document, f) : undefined,
    otherEmpty: f ? otherEmptyFields(f).length : undefined,
    submit: f ? (submitControl(f)?.id || null) : undefined,
  };
  const failures = Object.entries(c.expect).filter(([k, v]) => (got as Record<string, unknown>)[k] !== v);
  return { name: c.name, ok: failures.length === 0, failures: failures.map(([k, v]) => `${k}: expected ${v}, got ${(got as Record<string, unknown>)[k]}`) };
});
document.body.innerHTML = "";
const pre = document.createElement("pre");
pre.id = "results";
pre.textContent = JSON.stringify(results);
document.body.append(pre);
