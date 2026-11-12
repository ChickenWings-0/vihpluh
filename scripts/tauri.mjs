// Runs the Tauri CLI. `dev` gets a repo-local data root so a development build never touches
// personal config (ARCHITECTURE §9.1).
import { spawn } from "node:child_process";
import { createRequire } from "node:module";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const require = createRequire(import.meta.url);
const cli = join(dirname(require.resolve("@tauri-apps/cli/package.json")), "tauri.js");

const args = process.argv.slice(2);
const env = { ...process.env };
if (args[0] === "dev" && !env.VIHPLUH_HOME) {
  env.VIHPLUH_HOME = join(root, ".local");
}

const child = spawn(process.execPath, [cli, ...args], { cwd: root, env, stdio: "inherit" });
child.on("exit", (code, signal) => {
  process.exit(signal ? 1 : (code ?? 1));
});
