import { invokeTauriOrNull } from "./tauriInvoke";
import type { CliKey } from "./providers";

export type SkillRepoSummary = {
  id: number;
  git_url: string;
  branch: string;
  enabled: boolean;
  created_at: number;
  updated_at: number;
};

export type InstalledSkillSummary = {
  id: number;
  skill_key: string;
  name: string;
  description: string;
  source_git_url: string;
  source_branch: string;
  source_subdir: string;
  enabled_claude: boolean;
  enabled_codex: boolean;
  enabled_gemini: boolean;
  created_at: number;
  updated_at: number;
};

export type AvailableSkillSummary = {
  name: string;
  description: string;
  source_git_url: string;
  source_branch: string;
  source_subdir: string;
  installed: boolean;
};

export type SkillsPaths = {
  ssot_dir: string;
  repos_dir: string;
  cli_dir: string;
};

export type LocalSkillSummary = {
  dir_name: string;
  path: string;
  name: string;
  description: string;
};

export async function skillReposList() {
  return invokeTauriOrNull<SkillRepoSummary[]>("skill_repos_list");
}

export async function skillRepoUpsert(input: {
  repo_id?: number | null;
  git_url: string;
  branch: string;
  enabled: boolean;
}) {
  return invokeTauriOrNull<SkillRepoSummary>("skill_repo_upsert", {
    repoId: input.repo_id ?? null,
    gitUrl: input.git_url,
    branch: input.branch,
    enabled: input.enabled,
  });
}

export async function skillRepoDelete(repoId: number) {
  return invokeTauriOrNull<boolean>("skill_repo_delete", { repoId });
}

export async function skillsInstalledList() {
  return invokeTauriOrNull<InstalledSkillSummary[]>("skills_installed_list");
}

export async function skillsDiscoverAvailable(refresh: boolean) {
  return invokeTauriOrNull<AvailableSkillSummary[]>("skills_discover_available", {
    refresh,
  });
}

export async function skillInstall(input: {
  git_url: string;
  branch: string;
  source_subdir: string;
  enabled_claude: boolean;
  enabled_codex: boolean;
  enabled_gemini: boolean;
}) {
  return invokeTauriOrNull<InstalledSkillSummary>("skill_install", {
    gitUrl: input.git_url,
    branch: input.branch,
    sourceSubdir: input.source_subdir,
    enabledClaude: input.enabled_claude,
    enabledCodex: input.enabled_codex,
    enabledGemini: input.enabled_gemini,
  });
}

export async function skillSetEnabled(input: {
  skill_id: number;
  cli_key: CliKey;
  enabled: boolean;
}) {
  return invokeTauriOrNull<InstalledSkillSummary>("skill_set_enabled", {
    skillId: input.skill_id,
    cliKey: input.cli_key,
    enabled: input.enabled,
  });
}

export async function skillUninstall(skillId: number) {
  return invokeTauriOrNull<boolean>("skill_uninstall", { skillId });
}

export async function skillsLocalList(cliKey: CliKey) {
  return invokeTauriOrNull<LocalSkillSummary[]>("skills_local_list", { cliKey });
}

export async function skillImportLocal(input: { cli_key: CliKey; dir_name: string }) {
  return invokeTauriOrNull<InstalledSkillSummary>("skill_import_local", {
    cliKey: input.cli_key,
    dirName: input.dir_name,
  });
}

export async function skillsPathsGet(cliKey: CliKey) {
  return invokeTauriOrNull<SkillsPaths>("skills_paths_get", { cliKey });
}
