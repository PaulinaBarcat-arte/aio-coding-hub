import type { CliKey } from "../services/providers";

export type CliItem = {
  key: CliKey;
  name: string;
  desc: string;
};

export const CLIS: CliItem[] = [
  { key: "claude", name: "Claude Code", desc: "Claude CLI / Claude Code" },
  { key: "codex", name: "Codex", desc: "OpenAI Codex CLI" },
  { key: "gemini", name: "Gemini", desc: "Google Gemini CLI" },
];

export function cliShortLabel(cliKey: string) {
  if (cliKey === "claude") return "Claude";
  if (cliKey === "codex") return "Codex";
  if (cliKey === "gemini") return "Gemini";
  return cliKey;
}

export function cliBadgeTone(cliKey: string) {
  if (cliKey === "claude")
    return "bg-slate-100 text-slate-600 group-hover:bg-white group-hover:border-slate-200 border border-transparent";
  if (cliKey === "codex")
    return "bg-slate-100 text-slate-600 group-hover:bg-white group-hover:border-slate-200 border border-transparent";
  if (cliKey === "gemini")
    return "bg-slate-100 text-slate-600 group-hover:bg-white group-hover:border-slate-200 border border-transparent";
  return "bg-slate-100 text-slate-600 border border-transparent";
}
