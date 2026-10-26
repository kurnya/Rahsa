// Bundles each entry to a self-contained IIFE (content scripts registered via
// chrome.scripting cannot be ES modules) and copies static files to dist/.
import * as esbuild from "esbuild";
import { cpSync, mkdirSync, rmSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const here = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const dist = resolve(here, "dist");
rmSync(dist, { recursive: true, force: true });
mkdirSync(dist, { recursive: true });
cpSync(resolve(here, "public"), dist, { recursive: true });

const options = {
  entryPoints: ["background", "content", "popup"].map((n) => resolve(here, `src/${n}.ts`)),
  outdir: dist,
  bundle: true,
  format: "iife",
  target: "chrome120",
  minify: false,
  legalComments: "none",
  logLevel: "info",
};

if (process.argv.includes("--watch")) {
  const ctx = await esbuild.context(options);
  await ctx.watch();
} else {
  await esbuild.build(options);
}
