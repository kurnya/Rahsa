// Mirror of crates/rahsa-core/src/session.rs. Any change here must keep
// `scripts/crypto-vector.mjs` (which pins the Rust output) passing.

import { x25519 } from "@noble/curves/ed25519.js";
import { xchacha20poly1305 } from "@noble/ciphers/chacha.js";
import { hkdf } from "@noble/hashes/hkdf.js";
import { sha256 } from "@noble/hashes/sha2.js";

const enc = new TextEncoder();
const dec = new TextDecoder();

export const SESSION_INFO = enc.encode("rahsa-session-v1");
export const PAIR_DOMAIN = enc.encode("rahsa-pair-v1");
export const AAD_EXT_TO_APP = enc.encode("rahsa:e2a");
export const AAD_APP_TO_EXT = enc.encode("rahsa:a2e");

export function random(n: number): Uint8Array {
  return crypto.getRandomValues(new Uint8Array(n));
}

export function b64(bytes: Uint8Array): string {
  let s = "";
  for (const b of bytes) s += String.fromCharCode(b);
  return btoa(s);
}

export function unb64(s: string): Uint8Array {
  const bin = atob(s);
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
  return out;
}

function concat(...parts: Uint8Array[]): Uint8Array {
  const out = new Uint8Array(parts.reduce((n, p) => n + p.length, 0));
  let o = 0;
  for (const p of parts) {
    out.set(p, o);
    o += p.length;
  }
  return out;
}

export function newSecret(): Uint8Array {
  return x25519.utils.randomSecretKey();
}

export function publicKey(secret: Uint8Array): Uint8Array {
  return x25519.getPublicKey(secret);
}

export function pairingCode(extPub: Uint8Array, appPub: Uint8Array): string {
  const d = sha256(concat(PAIR_DOMAIN, extPub, appPub));
  const n = ((d[0] << 24) | (d[1] << 16) | (d[2] << 8) | d[3]) >>> 0;
  return String(n % 1_000_000).padStart(6, "0");
}

/** Extension side of `derive_session_key`: same DHs, roles swapped. */
export function deriveSessionKey(
  extStatic: Uint8Array,
  appStaticPub: Uint8Array,
  extEph: Uint8Array,
  appEphPub: Uint8Array,
  extNonce: Uint8Array,
  appNonce: Uint8Array,
): Uint8Array {
  const dh1 = x25519.getSharedSecret(extStatic, appStaticPub);
  const dh2 = x25519.getSharedSecret(extEph, appEphPub);
  if (dh1.every((b) => b === 0) || dh2.every((b) => b === 0)) throw new Error("invalid public key");
  const key = hkdf(sha256, concat(dh1, dh2), concat(extNonce, appNonce), SESSION_INFO, 32);
  dh1.fill(0);
  dh2.fill(0);
  return key;
}

export function seal(key: Uint8Array, aad: Uint8Array, plaintext: string): { n: string; c: string } {
  const nonce = random(24);
  const ct = xchacha20poly1305(key, nonce, aad).encrypt(enc.encode(plaintext));
  return { n: b64(nonce), c: b64(ct) };
}

export function open(key: Uint8Array, aad: Uint8Array, n: string, c: string): string {
  const pt = xchacha20poly1305(key, unb64(n), aad).decrypt(unb64(c));
  const s = dec.decode(pt);
  pt.fill(0);
  return s;
}
