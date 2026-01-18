import fs from "node:fs";
import path from "node:path";

const ROOT = process.cwd();
const TARGET_DIR = path.join(ROOT, "src-tauri", "src");
const PATTERN = /Instant::now\(\)\s*-\s*/;

function isRustSource(filePath) {
  return filePath.endsWith(".rs");
}

function walk(dirPath, out) {
  const entries = fs.readdirSync(dirPath, { withFileTypes: true });
  for (const entry of entries) {
    const fullPath = path.join(dirPath, entry.name);
    if (entry.isDirectory()) {
      walk(fullPath, out);
      continue;
    }
    if (entry.isFile() && isRustSource(fullPath)) {
      out.push(fullPath);
    }
  }
}

function findViolations(filePath) {
  const text = fs.readFileSync(filePath, "utf8");
  const lines = text.split(/\r?\n/);
  const hits = [];
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    if (!PATTERN.test(line)) continue;
    hits.push({ lineNumber: i + 1, line });
  }
  return hits;
}

if (!fs.existsSync(TARGET_DIR)) {
  console.error(`Expected directory not found: ${TARGET_DIR}`);
  process.exit(2);
}

const files = [];
walk(TARGET_DIR, files);

const violations = [];
for (const filePath of files) {
  const hits = findViolations(filePath);
  if (hits.length === 0) continue;
  violations.push({ filePath, hits });
}

if (violations.length === 0) {
  process.exit(0);
}

console.error(
  "Forbidden pattern detected: `Instant::now() - <Duration>` (can panic on underflow).\n" +
    "Use `Instant::checked_sub(...)` or `saturating_duration_since(...)` style patterns instead.\n"
);

for (const v of violations) {
  const rel = path.relative(ROOT, v.filePath);
  for (const hit of v.hits) {
    console.error(`${rel}:${hit.lineNumber}: ${hit.line.trim()}`);
  }
}

process.exit(1);

