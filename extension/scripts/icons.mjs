// Renders the "locked" toolbar icons from assets/rahsa-logo/rahsa-icon-locked.svg.
// The normal icons are copied as-is from assets/rahsa-logo/chrome-extension.
import { Resvg } from "@resvg/resvg-js";
import { copyFileSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const logo = resolve(root, "assets/rahsa-logo");
for (const s of [16, 32, 48, 128]) {
  copyFileSync(resolve(logo, `chrome-extension/icon${s}.png`), resolve(root, `extension/public/icons/${s}.png`));
  const png = new Resvg(readFileSync(resolve(logo, "rahsa-icon-locked.svg")), { fitTo: { mode: "width", value: s } }).render().asPng();
  writeFileSync(resolve(root, `extension/public/icons/locked-${s}.png`), png);
}
console.log("icons written");
