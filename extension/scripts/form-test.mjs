// Runs test/form-cases.ts inside real headless Chrome (layout and computed
// styles matter for the visibility rules, so jsdom would not be meaningful).
import * as esbuild from "esbuild";
import { execFileSync } from "node:child_process";
import { existsSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const here = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const chrome = [
  process.env.CHROME_PATH,
  "C:/Program Files/Google/Chrome/Application/chrome.exe",
  "C:/Program Files (x86)/Google/Chrome/Application/chrome.exe",
  `${process.env.LOCALAPPDATA}/Google/Chrome/Application/chrome.exe`,
].find((p) => p && existsSync(p));
if (!chrome) throw new Error("Chrome not found; set CHROME_PATH");

const out = mkdtempSync(join(tmpdir(), "rahsa-form-"));
const js = await esbuild.build({ entryPoints: [resolve(here, "test/form-cases.ts")], bundle: true, format: "iife", write: false });
writeFileSync(join(out, "cases.js"), js.outputFiles[0].text);
writeFileSync(join(out, "index.html"), `<!doctype html><meta charset="utf-8"><body style="margin:0;width:900px"></body><script src="cases.js"></script>`);

const dom = execFileSync(chrome, ["--headless=new", "--disable-gpu", "--no-first-run", `--user-data-dir=${join(out, "profile")}`, "--window-size=1200,900", "--dump-dom", pathToFileURL(join(out, "index.html")).href], { encoding: "utf8", timeout: 60_000 });
const m = dom.match(/<pre id="results">([\s\S]*?)<\/pre>/);
if (!m) throw new Error("no results in DOM:\n" + dom.slice(0, 500));
const results = JSON.parse(m[1].replace(/&quot;/g, '"').replace(/&amp;/g, "&").replace(/&lt;/g, "<").replace(/&gt;/g, ">"));
let failed = 0;
for (const r of results) {
  console.log(`${r.ok ? "ok  " : "FAIL"}  ${r.name}${r.ok ? "" : "\n      " + r.failures.join("\n      ")}`);
  if (!r.ok) failed++;
}
console.log(`\n${results.length - failed}/${results.length} passed`);
process.exit(failed ? 1 : 0);
