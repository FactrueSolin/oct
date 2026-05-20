import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { webcrypto } from "node:crypto";
import ts from "typescript";

if (!globalThis.crypto) {
  Object.defineProperty(globalThis, "crypto", { value: webcrypto });
}

const source = await readFile(new URL("../src/index.ts", import.meta.url), "utf8");
const { outputText } = ts.transpileModule(source, {
  compilerOptions: {
    target: ts.ScriptTarget.ES2021,
    module: ts.ModuleKind.ES2022,
  },
});
const worker = (await import(`data:text/javascript;charset=utf-8,${encodeURIComponent(outputText)}`)).default;

async function sha256Hex(bytes) {
  const hashBuffer = await crypto.subtle.digest("SHA-256", bytes);
  return Array.from(new Uint8Array(hashBuffer))
    .map((b) => b.toString(16).padStart(2, "0"))
    .join("");
}

const oneMiB = 1024 * 1024;
const files = [];
for (let i = 0; i < 11; i += 1) {
  const content = new Uint8Array(oneMiB);
  content.fill(65 + i);
  files.push({
    path: `configs/file-${i}.txt`,
    contentBase64: Buffer.from(content).toString("base64"),
    sha256: await sha256Hex(content),
    size: content.length,
  });
}

const writes = [];
const response = await worker.fetch(
  new Request("https://example.test/api/v1/config", {
    method: "PUT",
    headers: {
      Authorization: "Bearer " + "A".repeat(32),
      "Content-Type": "application/json",
    },
    body: JSON.stringify({
      schemaVersion: 1,
      createdAt: "2000-01-01T00:00:00.000Z",
      platform: "test",
      files,
      totalBytes: 1,
      bundleHash: "client-forged-small-total",
    }),
  }),
  {
    OCT_KV: {
      async get() {
        return null;
      },
      async put(key, value) {
        writes.push([key, value]);
      },
    },
  }
);

const body = await response.json();
assert.equal(response.status, 400);
assert.equal(body.error.code, "BUNDLE_TOO_LARGE");
assert.equal(writes.length, 0);

console.log("serverTotalBytes bundle limit verification passed");
