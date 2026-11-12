// Regenerates src/ipc/bindings from the Rust DTOs by running the ts-rs export tests.
// The output directory comes from TS_RS_EXPORT_DIR in .cargo/config.toml.
import { spawnSync } from "node:child_process";
import { readdirSync, rmSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const out = join(root, "src", "ipc", "bindings");

for (const name of readdirSync(out)) {
  if (name.endsWith(".ts")) rmSync(join(out, name));
}

const result = spawnSync(
  "cargo",
  ["test", "--manifest-path", "src-tauri/Cargo.toml", "--lib", "export_bindings"],
  { cwd: root, stdio: "inherit" },
);
if (result.status !== 0) process.exit(result.status ?? 1);

const count = readdirSync(out).filter((name) => name.endsWith(".ts")).length;
console.log(`Generated ${count} binding files in src/ipc/bindings`);
