import { execFileSync } from "node:child_process";
import { chmodSync, existsSync } from "node:fs";
import { join } from "node:path";

function runGit(args) {
  return execFileSync("git", args, { encoding: "utf8" }).trim();
}

function isTruthyEnv(value) {
  if (!value) return false;
  return !["0", "false", "no", "off"].includes(String(value).toLowerCase());
}

if (isTruthyEnv(process.env.SKIP_GIT_HOOKS) || isTruthyEnv(process.env.CI)) {
  process.exit(0);
}

let repoRoot;
try {
  repoRoot = runGit(["rev-parse", "--show-toplevel"]);
} catch {
  // Not a git worktree (e.g. CI artifact, package tarball). Don't fail install.
  process.exit(0);
}

const hooksDir = join(repoRoot, ".githooks");
const preCommitHook = join(hooksDir, "pre-commit");
const prePushHook = join(hooksDir, "pre-push");

if (!existsSync(preCommitHook)) {
  // Repo might be packaged without hooks; don't fail install.
  process.exit(0);
}

if (!existsSync(prePushHook)) {
  process.exit(0);
}

try {
  runGit(["config", "core.hooksPath", ".githooks"]);
} catch {
  // Don't fail install if git config isn't writable.
  process.exit(0);
}

try {
  chmodSync(preCommitHook, 0o755);
  chmodSync(prePushHook, 0o755);
} catch {
  // chmod can fail on some platforms/filesystems; core.hooksPath is the important part.
}

console.log("Git hooks installed:");
console.log("- core.hooksPath = .githooks");
console.log("- .githooks/pre-commit is executable");
console.log("- .githooks/pre-push is executable");
