// Proves the extension's session crypto (src/crypto.ts) matches the app's
// (crates/rahsa-core/src/session.rs): same fixed keys → same session key,
// same pairing code, same ciphertext. Runs the Rust test to get its output.
import * as esbuild from "esbuild";
import { execFileSync } from "node:child_process";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const here = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const rust = execFileSync("cargo", ["test", "-q", "-p", "rahsa-core", "cross_check_vector", "--", "--nocapture"], {
  cwd: resolve(here, ".."),
  encoding: "utf8",
});
const m = rust.match(/VECTOR key=(\S+) code=(\d{6}) ct=(\S+)/);
if (!m) throw new Error("Rust vector not found in output:\n" + rust);
const [, rKey, rCode, rCt] = m;

const bundle = await esbuild.build({
  stdin: {
    contents: `
      import { deriveSessionKey, pairingCode, publicKey, b64, AAD_APP_TO_EXT } from "./src/crypto";
      import { xchacha20poly1305 } from "@noble/ciphers/chacha.js";
      const fill = (b) => new Uint8Array(32).fill(b);
      const appS = fill(0x11), extS = fill(0x22), appE = fill(0x33), extE = fill(0x44);
      const key = deriveSessionKey(extS, publicKey(appS), extE, publicKey(appE), new Uint8Array(16).fill(0x55), new Uint8Array(16).fill(0x66));
      const ct = xchacha20poly1305(key, new Uint8Array(24).fill(0x77), AAD_APP_TO_EXT).encrypt(new TextEncoder().encode("rahsa"));
      export const out = { key: b64(key), code: pairingCode(publicKey(extS), publicKey(appS)), ct: b64(ct) };`,
    resolveDir: here,
    loader: "ts",
  },
  bundle: true,
  format: "esm",
  platform: "node",
  write: false,
});
const { out } = await import("data:text/javascript;base64," + Buffer.from(bundle.outputFiles[0].text).toString("base64"));

const checks = [
  ["session key", out.key, rKey],
  ["pairing code", out.code, rCode],
  ["ciphertext", out.ct, rCt],
];
let failed = 0;
for (const [name, ts, rs] of checks) {
  const ok = ts === rs;
  if (!ok) failed++;
  console.log(`${ok ? "ok  " : "FAIL"}  ${name}${ok ? "" : `\n      rust=${rs}\n      ts  =${ts}`}`);
}
process.exit(failed ? 1 : 0);
