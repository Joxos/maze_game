// 从 Cargo.lock 读取全部 registry 依赖，用 Node.js 自带的 OpenSSL TLS
// （本机 schannel 沙箱不可用）从 static.crates.io 下载 .crate 文件到 vendor/，
// 校验 SHA256 与 Cargo.lock checksum 一致，并生成 cargo vendor 格式的
// .cargo-checksum.json。之后 cargo 可完全离线构建。
//
// 用法：在 maze_game/ 目录下执行  node tools/fetch-vendor.mjs

import { createHash } from "node:crypto";
import { mkdirSync, writeFileSync, readFileSync } from "node:fs";
import { join } from "node:path";
import https from "node:https";

const root = process.cwd();
const lockText = readFileSync(join(root, "Cargo.lock"), "utf8");

// ---- 解析 Cargo.lock 的 [[package]] 块 ----
const packages = [];
const blockRe = /\[\[package\]\]([\s\S]*?)(?=\[\[package\]\]|$)/g;
let m;
while ((m = blockRe.exec(lockText))) {
  const block = m[1];
  const name = block.match(/^name = "([^"]+)"/m)?.[1];
  const version = block.match(/^version = "([^"]+)"/m)?.[1];
  const checksum = block.match(/^checksum = "([^"]+)"/m)?.[1];
  if (name && version && checksum) packages.push({ name, version, checksum });
}
console.log(`packages to fetch: ${packages.length}`);

const vendorDir = join(root, "vendor");
mkdirSync(vendorDir, { recursive: true });

function download(url) {
  return new Promise((resolve, reject) => {
    const req = https.get(url, { headers: { "User-Agent": "cargo/1.97.1" } }, (res) => {
      if (res.statusCode !== 200) {
        res.resume();
        reject(new Error(`HTTP ${res.statusCode}`));
        return;
      }
      const chunks = [];
      res.on("data", (c) => chunks.push(c));
      res.on("end", () => resolve(Buffer.concat(chunks)));
    });
    req.on("error", reject);
  });
}

const CONCURRENCY = 6;
let index = 0;
let ok = 0;
let fail = 0;

async function worker() {
  while (true) {
    const i = index++;
    if (i >= packages.length) return;
    const p = packages[i];
    const dir = join(vendorDir, `${p.name}-${p.version}`);
    const file = join(dir, `${p.name}-${p.version}.crate`);
    const url = `https://static.crates.io/crates/${p.name}/${p.name}-${p.version}.crate`;
    try {
      let buf = null;
      for (let attempt = 0; attempt < 3 && !buf; attempt++) {
        try {
          buf = await download(url);
        } catch (e) {
          if (attempt === 2) throw e;
          await new Promise((r) => setTimeout(r, 800 * (attempt + 1)));
        }
      }
      const hash = createHash("sha256").update(buf).digest("hex");
      if (hash !== p.checksum) {
        throw new Error(`checksum mismatch: got ${hash}, want ${p.checksum}`);
      }
      mkdirSync(dir, { recursive: true });
      writeFileSync(file, buf);
      writeFileSync(join(dir, ".cargo-checksum.json"), JSON.stringify({ files: {}, package: p.checksum }));
      ok++;
      console.log(`OK   ${p.name}-${p.version}`);
    } catch (e) {
      fail++;
      console.log(`FAIL ${p.name}-${p.version}: ${e.message}`);
    }
  }
}

await Promise.all(Array.from({ length: CONCURRENCY }, worker));
console.log(`done: ok=${ok} fail=${fail} total=${packages.length}`);
process.exit(fail ? 1 : 0);
