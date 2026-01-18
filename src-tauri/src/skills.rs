use crate::app_paths;
use crate::db;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use std::collections::{BTreeMap, HashSet};
use std::io::{Cursor, Write};
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::Manager;

const MANAGED_MARKER_FILE: &str = ".aio-coding-hub.managed";
const REPO_BRANCH_FILE: &str = ".aio-coding-hub.repo-branch";
const REPO_SNAPSHOT_MARKER_FILE: &str = ".aio-coding-hub.repo-snapshot";

#[derive(Debug, Clone, Serialize)]
pub struct SkillRepoSummary {
    pub id: i64,
    pub git_url: String,
    pub branch: String,
    pub enabled: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct InstalledSkillSummary {
    pub id: i64,
    pub skill_key: String,
    pub name: String,
    pub description: String,
    pub source_git_url: String,
    pub source_branch: String,
    pub source_subdir: String,
    pub enabled_claude: bool,
    pub enabled_codex: bool,
    pub enabled_gemini: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct AvailableSkillSummary {
    pub name: String,
    pub description: String,
    pub source_git_url: String,
    pub source_branch: String,
    pub source_subdir: String,
    pub installed: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct SkillsPaths {
    pub ssot_dir: String,
    pub repos_dir: String,
    pub cli_dir: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct LocalSkillSummary {
    pub dir_name: String,
    pub path: String,
    pub name: String,
    pub description: String,
}

fn now_unix_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn now_unix_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0)
}

fn enabled_to_int(enabled: bool) -> i64 {
    if enabled {
        1
    } else {
        0
    }
}

fn normalize_name(name: &str) -> String {
    name.trim().to_lowercase()
}

fn normalize_repo_branch(branch: &str) -> String {
    let branch = branch.trim();
    if branch.is_empty() || branch.eq_ignore_ascii_case("auto") {
        "auto".to_string()
    } else {
        branch.to_string()
    }
}

fn canonical_git_url_key(input: &str) -> String {
    let raw = input.trim();
    if raw.is_empty() {
        return String::new();
    }

    // scp-like: git@github.com:owner/repo(.git)
    if let Some(rest) = raw.strip_prefix("git@") {
        let (host, path) = match rest.split_once(':') {
            Some((host, path)) => (host, path),
            None => (rest, ""),
        };

        let host = host
            .trim()
            .trim_end_matches('/')
            .split(':')
            .next()
            .unwrap_or(host)
            .to_ascii_lowercase();
        let mut path = path.trim().trim_matches('/').to_string();
        if path.to_ascii_lowercase().ends_with(".git") {
            path.truncate(path.len().saturating_sub(4));
        }

        if path.is_empty() {
            return host;
        }

        return format!("{}/{}", host, path.to_ascii_lowercase());
    }

    // Strip scheme if present (https://, ssh://, git://, etc.)
    let mut rest = raw;
    if let Some(pos) = raw.find("://") {
        rest = &raw[(pos + 3)..];
    }

    // Strip userinfo (git@) when it appears before the first slash
    if let Some(at) = rest.find('@') {
        let slash = rest.find('/').unwrap_or(rest.len());
        if at < slash {
            rest = &rest[(at + 1)..];
        }
    }

    let rest = rest.trim().trim_matches('/');
    let (host, path) = match rest.split_once('/') {
        Some((host, path)) => (host, path),
        None => (rest, ""),
    };

    let host = host
        .trim()
        .trim_end_matches('/')
        .split(':')
        .next()
        .unwrap_or(host)
        .to_ascii_lowercase();

    let mut path = path.trim().trim_matches('/').to_string();
    if path.to_ascii_lowercase().ends_with(".git") {
        path.truncate(path.len().saturating_sub(4));
    }

    // Common user behavior: pasting GitHub browser URLs like /owner/repo/tree/main/...
    if host.ends_with("github.com") {
        let segs: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        if segs.len() >= 2 {
            path = format!("{}/{}", segs[0], segs[1]);
        }
    }

    if path.is_empty() {
        host
    } else {
        format!("{}/{}", host, path.to_ascii_lowercase())
    }
}

fn parse_github_owner_repo(input: &str) -> Option<(String, String)> {
    let key = canonical_git_url_key(input);
    if key.is_empty() {
        return None;
    }
    let (host, path) = key.split_once('/')?;
    if !host.ends_with("github.com") {
        return None;
    }
    let mut segs = path.split('/').filter(|s| !s.is_empty());
    let owner = segs.next()?.to_string();
    let repo = segs.next()?.to_string();
    if owner.is_empty() || repo.is_empty() {
        return None;
    }
    Some((owner, repo))
}

fn validate_cli_key(cli_key: &str) -> Result<(), String> {
    match cli_key {
        "claude" | "codex" | "gemini" => Ok(()),
        _ => Err(format!("SEC_INVALID_INPUT: unknown cli_key={cli_key}")),
    }
}

fn home_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    app.path()
        .home_dir()
        .map_err(|e| format!("failed to resolve home dir: {e}"))
}

fn ssot_skills_root(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(app_paths::app_data_dir(app)?.join("skills"))
}

fn repos_root(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(app_paths::app_data_dir(app)?.join("skill-repos"))
}

fn cli_skills_root(app: &tauri::AppHandle, cli_key: &str) -> Result<PathBuf, String> {
    validate_cli_key(cli_key)?;
    let home = home_dir(app)?;
    match cli_key {
        "claude" => Ok(home.join(".claude").join("skills")),
        "codex" => Ok(home.join(".codex").join("skills")),
        "gemini" => Ok(home.join(".gemini").join("skills")),
        _ => Err(format!("SEC_INVALID_INPUT: unknown cli_key={cli_key}")),
    }
}

fn fnv1a64(input: &str) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for b in input.as_bytes() {
        hash ^= *b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn repo_cache_dir(app: &tauri::AppHandle, git_url: &str, branch: &str) -> Result<PathBuf, String> {
    let root = repos_root(app)?;
    let key = format!("{}#{}", git_url.trim(), branch.trim());
    Ok(root.join(format!("{:016x}", fnv1a64(&key))))
}

struct RepoLockGuard {
    path: PathBuf,
    file: Option<std::fs::File>,
}

impl RepoLockGuard {
    fn acquire(path: PathBuf) -> Result<Self, String> {
        fn is_stale(lock_path: &Path, stale_after: Duration) -> bool {
            let Ok(meta) = std::fs::metadata(lock_path) else {
                return false;
            };
            let Ok(modified) = meta.modified() else {
                return false;
            };
            let Ok(age) = SystemTime::now().duration_since(modified) else {
                return false;
            };
            age > stale_after
        }

        let stale_after = Duration::from_secs(120);
        let deadline = SystemTime::now() + Duration::from_secs(30);

        loop {
            match std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
            {
                Ok(mut file) => {
                    let _ = writeln!(
                        file,
                        "pid={} ts_nanos={}",
                        std::process::id(),
                        now_unix_nanos()
                    );
                    return Ok(Self {
                        path,
                        file: Some(file),
                    });
                }
                Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
                    if is_stale(&path, stale_after) {
                        let _ = std::fs::remove_file(&path);
                        continue;
                    }
                    if SystemTime::now() > deadline {
                        return Err(format!(
                            "SKILL_REPO_LOCK_TIMEOUT: failed to acquire repo lock {}",
                            path.display()
                        ));
                    }
                    std::thread::sleep(Duration::from_millis(50));
                    continue;
                }
                Err(err) => {
                    return Err(format!(
                        "SKILL_REPO_LOCK_ERROR: failed to create repo lock {}: {err}",
                        path.display()
                    ));
                }
            }
        }
    }
}

impl Drop for RepoLockGuard {
    fn drop(&mut self) {
        let _ = self.file.take();
        let _ = std::fs::remove_file(&self.path);
    }
}

fn lock_path_for_repo_dir(dir: &Path) -> PathBuf {
    dir.with_extension("lock")
}

fn remove_path_if_exists(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }
    if path.is_dir() {
        std::fs::remove_dir_all(path)
            .map_err(|e| format!("failed to remove {}: {e}", path.display()))?;
        return Ok(());
    }
    std::fs::remove_file(path).map_err(|e| format!("failed to remove {}: {e}", path.display()))
}

fn run_git(mut cmd: Command) -> Result<(), String> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let out = cmd
        .output()
        .map_err(|e| format!("SKILL_GIT_NOT_FOUND: failed to execute git: {e}"))?;
    if out.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let msg = if !stderr.is_empty() { stderr } else { stdout };
    Err(format!("SKILL_GIT_ERROR: {msg}"))
}

fn run_git_capture(mut cmd: Command) -> Result<String, String> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let out = cmd
        .output()
        .map_err(|e| format!("SKILL_GIT_NOT_FOUND: failed to execute git: {e}"))?;
    if out.status.success() {
        return Ok(String::from_utf8_lossy(&out.stdout).trim().to_string());
    }
    let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let msg = if !stderr.is_empty() { stderr } else { stdout };
    Err(format!("SKILL_GIT_ERROR: {msg}"))
}

fn is_remote_branch_not_found(err: &str) -> bool {
    let e = err.to_ascii_lowercase();
    (e.contains("remote branch") && e.contains("not found"))
        || e.contains("couldn't find remote ref")
        || e.contains("could not find remote ref")
}

fn read_repo_branch(dir: &Path) -> Option<String> {
    let path = dir.join(REPO_BRANCH_FILE);
    let text = std::fs::read_to_string(&path).ok()?;
    let branch = text.trim().to_string();
    if branch.is_empty() {
        return None;
    }
    Some(branch)
}

fn write_repo_branch(dir: &Path, branch: &str) -> Result<(), String> {
    let path = dir.join(REPO_BRANCH_FILE);
    std::fs::write(&path, format!("{}\n", branch.trim()))
        .map_err(|e| format!("failed to write {}: {e}", path.display()))?;
    Ok(())
}

fn detect_checked_out_branch(dir: &Path) -> Result<String, String> {
    let mut cmd = Command::new("git");
    cmd.arg("-C")
        .arg(dir)
        .arg("rev-parse")
        .arg("--abbrev-ref")
        .arg("HEAD");
    let out = run_git_capture(cmd)?;
    let branch = out.trim().to_string();
    if branch.is_empty() || branch == "HEAD" {
        return Err("SKILL_GIT_ERROR: failed to detect current branch".to_string());
    }
    Ok(branch)
}

fn build_github_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(60))
        .user_agent(format!("aio-coding-hub/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| format!("SKILL_HTTP_ERROR: failed to build http client: {e}"))
}

fn github_api_url(segments: &[&str]) -> Result<reqwest::Url, String> {
    let mut url = reqwest::Url::parse("https://api.github.com")
        .map_err(|e| format!("SKILL_GITHUB_URL_ERROR: {e}"))?;
    {
        let mut ps = url
            .path_segments_mut()
            .map_err(|_| "SKILL_GITHUB_URL_ERROR: invalid github api base url".to_string())?;
        for seg in segments {
            ps.push(seg);
        }
    }
    Ok(url)
}

fn github_default_branch(
    client: &reqwest::Client,
    owner: &str,
    repo: &str,
) -> Result<String, String> {
    let url = github_api_url(&["repos", owner, repo])?;
    let client = client.clone();
    tauri::async_runtime::block_on(async move {
        let resp = client
            .get(url)
            .header("Accept", "application/vnd.github+json")
            .send()
            .await
            .map_err(|e| format!("SKILL_HTTP_ERROR: github request failed: {e}"))?;

        let status = resp.status();
        let body = resp
            .text()
            .await
            .map_err(|e| format!("SKILL_HTTP_ERROR: failed to read github response: {e}"))?;

        if status == reqwest::StatusCode::NOT_FOUND {
            return Err("SKILL_GITHUB_REPO_NOT_FOUND: repository not found".to_string());
        }
        if status == reqwest::StatusCode::FORBIDDEN {
            return Err(
                "SKILL_GITHUB_FORBIDDEN: github request forbidden (rate limit?)".to_string(),
            );
        }
        if !status.is_success() {
            return Err(format!(
                "SKILL_GITHUB_HTTP_ERROR: github returned http status {}",
                status
            ));
        }

        let root: serde_json::Value = serde_json::from_str(&body)
            .map_err(|e| format!("SKILL_GITHUB_PARSE_ERROR: github json parse failed: {e}"))?;
        let branch = root
            .get("default_branch")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim();
        if branch.is_empty() {
            return Err("SKILL_GITHUB_PARSE_ERROR: missing default_branch".to_string());
        }
        Ok(branch.to_string())
    })
}

fn github_download_zipball(
    client: &reqwest::Client,
    owner: &str,
    repo: &str,
    r#ref: &str,
) -> Result<Vec<u8>, String> {
    let url = github_api_url(&["repos", owner, repo, "zipball", r#ref])?;
    let client = client.clone();
    tauri::async_runtime::block_on(async move {
        let resp = client
            .get(url)
            .header("Accept", "application/vnd.github+json")
            .send()
            .await
            .map_err(|e| format!("SKILL_HTTP_ERROR: github zip download failed: {e}"))?;

        let status = resp.status();
        if status == reqwest::StatusCode::NOT_FOUND {
            return Err("SKILL_GITHUB_REF_NOT_FOUND: branch/ref not found".to_string());
        }
        if status == reqwest::StatusCode::FORBIDDEN {
            return Err(
                "SKILL_GITHUB_FORBIDDEN: github request forbidden (rate limit?)".to_string(),
            );
        }
        if !status.is_success() {
            return Err(format!(
                "SKILL_GITHUB_HTTP_ERROR: github returned http status {}",
                status
            ));
        }

        let bytes = resp
            .bytes()
            .await
            .map_err(|e| format!("SKILL_HTTP_ERROR: failed to read github zip body: {e}"))?;
        Ok(bytes.to_vec())
    })
}

fn unzip_repo_zip(zip_bytes: &[u8], dst_dir: &Path) -> Result<PathBuf, String> {
    std::fs::create_dir_all(dst_dir)
        .map_err(|e| format!("failed to create {}: {e}", dst_dir.display()))?;

    let mut archive = zip::ZipArchive::new(Cursor::new(zip_bytes))
        .map_err(|e| format!("SKILL_ZIP_ERROR: failed to open zip archive: {e}"))?;

    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| format!("SKILL_ZIP_ERROR: failed to read zip entry: {e}"))?;
        let name = file.name().replace('\\', "/");
        if name.is_empty() {
            continue;
        }

        let rel = Path::new(&name);
        if rel.is_absolute() {
            return Err("SKILL_ZIP_ERROR: invalid zip entry path (absolute)".to_string());
        }
        for comp in rel.components() {
            match comp {
                Component::CurDir | Component::Normal(_) => {}
                Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                    return Err("SKILL_ZIP_ERROR: invalid zip entry path".to_string());
                }
            }
        }

        let out_path = dst_dir.join(rel);
        if file.is_dir() {
            std::fs::create_dir_all(&out_path)
                .map_err(|e| format!("failed to create {}: {e}", out_path.display()))?;
            continue;
        }

        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("failed to create {}: {e}", parent.display()))?;
        }

        let mut out_file = std::fs::File::create(&out_path)
            .map_err(|e| format!("failed to create {}: {e}", out_path.display()))?;
        std::io::copy(&mut file, &mut out_file)
            .map_err(|e| format!("failed to write {}: {e}", out_path.display()))?;
    }

    let mut top_dirs = Vec::new();
    let mut top_files = 0_usize;
    let entries = std::fs::read_dir(dst_dir)
        .map_err(|e| format!("failed to read dir {}: {e}", dst_dir.display()))?;
    for entry in entries {
        let entry =
            entry.map_err(|e| format!("failed to read dir entry {}: {e}", dst_dir.display()))?;
        let path = entry.path();
        if path.is_dir() {
            top_dirs.push(path);
        } else {
            top_files += 1;
        }
    }

    if top_dirs.len() != 1 || top_files != 0 {
        return Err(format!(
            "SKILL_ZIP_ERROR: expected single root directory in zip (dirs={}, files={})",
            top_dirs.len(),
            top_files
        ));
    }

    Ok(top_dirs.remove(0))
}

fn repo_snapshot_marker_path(dir: &Path) -> PathBuf {
    dir.join(REPO_SNAPSHOT_MARKER_FILE)
}

fn write_repo_snapshot_marker(dir: &Path, git_url: &str, branch: &str) -> Result<(), String> {
    let path = repo_snapshot_marker_path(dir);
    let content = format!(
        "aio-coding-hub\nmode=snapshot\ngit_url={}\nbranch={}\n",
        git_url.trim(),
        branch.trim()
    );
    std::fs::write(&path, content)
        .map_err(|e| format!("failed to write marker {}: {e}", path.display()))?;
    Ok(())
}

fn ensure_github_repo_snapshot(
    app: &tauri::AppHandle,
    git_url: &str,
    owner: &str,
    repo: &str,
    branch: &str,
    refresh: bool,
) -> Result<PathBuf, String> {
    let dir = repo_cache_dir(app, git_url, branch)?;
    let snapshot_marker = repo_snapshot_marker_path(&dir);
    let git_dir = dir.join(".git");

    if !refresh && (snapshot_marker.exists() || git_dir.exists()) {
        return Ok(dir);
    }

    if let Some(parent) = dir.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("failed to create {}: {e}", parent.display()))?;
    }

    let _lock = RepoLockGuard::acquire(lock_path_for_repo_dir(&dir))?;

    let snapshot_marker = repo_snapshot_marker_path(&dir);
    let git_dir = dir.join(".git");
    if !refresh && (snapshot_marker.exists() || git_dir.exists()) {
        return Ok(dir);
    }

    // Self-heal: if the repo cache dir exists but isn't a git repo or a valid snapshot, remove it.
    if dir.exists() && !git_dir.exists() && !snapshot_marker.exists() {
        remove_path_if_exists(&dir)?;
    }

    let client = build_github_client()?;

    let mut effective_branch = String::new();
    let mut zip_bytes: Option<Vec<u8>> = None;
    let mut last_err: Option<String> = None;

    if branch == "auto" {
        // Common default branches: avoid GitHub API unless needed (rate limits).
        for candidate in ["main", "master"] {
            match github_download_zipball(&client, owner, repo, candidate) {
                Ok(bytes) => {
                    effective_branch = candidate.to_string();
                    zip_bytes = Some(bytes);
                    break;
                }
                Err(err) => {
                    last_err = Some(err);
                }
            }
        }

        if zip_bytes.is_none() {
            match github_default_branch(&client, owner, repo) {
                Ok(default_branch) => {
                    match github_download_zipball(&client, owner, repo, &default_branch) {
                        Ok(bytes) => {
                            effective_branch = default_branch;
                            zip_bytes = Some(bytes);
                        }
                        Err(err) => {
                            last_err = Some(err);
                        }
                    }
                }
                Err(err) => {
                    last_err = Some(err);
                }
            }
        }
    } else {
        match github_download_zipball(&client, owner, repo, branch) {
            Ok(bytes) => {
                effective_branch = branch.to_string();
                zip_bytes = Some(bytes);
            }
            Err(err) => {
                last_err = Some(err);
            }
        }
    }

    let Some(zip_bytes) = zip_bytes else {
        return Err(last_err.unwrap_or_else(|| {
            "SKILL_GITHUB_DOWNLOAD_FAILED: failed to download github zip".to_string()
        }));
    };
    if effective_branch.is_empty() {
        return Err("SKILL_GITHUB_BRANCH_ERROR: failed to resolve branch".to_string());
    }

    let parent = dir
        .parent()
        .ok_or_else(|| "SEC_INVALID_INPUT: invalid repo cache dir".to_string())?;
    let dir_name = dir
        .file_name()
        .and_then(|v| v.to_str())
        .unwrap_or("repo")
        .to_string();
    let nonce = now_unix_nanos();

    let staging = parent.join(format!(".{dir_name}.staging-{nonce}"));
    let _ = remove_path_if_exists(&staging);
    std::fs::create_dir_all(&staging)
        .map_err(|e| format!("failed to create {}: {e}", staging.display()))?;

    let extracted_root = match unzip_repo_zip(&zip_bytes, &staging) {
        Ok(v) => v,
        Err(err) => {
            let _ = remove_path_if_exists(&staging);
            return Err(err);
        }
    };

    write_repo_branch(&extracted_root, &effective_branch)?;
    write_repo_snapshot_marker(&extracted_root, git_url, &effective_branch)?;

    // Atomic-ish swap: move old dir away, then move new dir into place.
    let backup = parent.join(format!(".{dir_name}.old-{nonce}"));
    if dir.exists() && std::fs::rename(&dir, &backup).is_err() {
        if let Err(err) = remove_path_if_exists(&dir) {
            let _ = remove_path_if_exists(&staging);
            return Err(format!(
                "SKILL_REPO_BUSY: failed to replace {}: {err}",
                dir.display()
            ));
        }
    }

    if let Err(err) = std::fs::rename(&extracted_root, &dir) {
        let _ = remove_path_if_exists(&staging);
        if backup.exists() {
            let _ = std::fs::rename(&backup, &dir);
        }
        return Err(format!(
            "SKILL_REPO_UPDATE_FAILED: failed to activate repo snapshot {}: {err}",
            dir.display()
        ));
    }

    let _ = remove_path_if_exists(&backup);
    let _ = remove_path_if_exists(&staging);
    Ok(dir)
}

fn ensure_git_repo_cache(
    app: &tauri::AppHandle,
    git_url: &str,
    branch: &str,
    refresh: bool,
) -> Result<PathBuf, String> {
    let dir = repo_cache_dir(app, git_url, branch)?;
    let git_dir = dir.join(".git");

    if !refresh && git_dir.exists() {
        return Ok(dir);
    }

    if let Some(parent) = dir.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("failed to create {}: {e}", parent.display()))?;
    }

    let _lock = RepoLockGuard::acquire(lock_path_for_repo_dir(&dir))?;

    let git_dir = dir.join(".git");
    if !refresh && git_dir.exists() {
        return Ok(dir);
    }

    if !git_dir.exists() {
        // Self-heal: a previous failed clone can leave the dir behind without .git.
        if dir.exists() {
            remove_path_if_exists(&dir)?;
        }

        if branch == "auto" {
            let mut cmd = Command::new("git");
            cmd.arg("clone")
                .arg("--depth")
                .arg("1")
                .arg(git_url)
                .arg(&dir);
            run_git(cmd)?;

            if let Ok(actual_branch) = detect_checked_out_branch(&dir) {
                write_repo_branch(&dir, &actual_branch)?;
            } else {
                write_repo_branch(&dir, branch)?;
            }

            return Ok(dir);
        }

        let mut cmd = Command::new("git");
        cmd.arg("clone")
            .arg("--depth")
            .arg("1")
            .arg("--branch")
            .arg(branch)
            .arg(git_url)
            .arg(&dir);
        match run_git(cmd) {
            Ok(()) => {
                write_repo_branch(&dir, branch)?;
                return Ok(dir);
            }
            Err(err) => {
                if !is_remote_branch_not_found(&err) {
                    return Err(err);
                }

                remove_path_if_exists(&dir)?;

                let mut cmd = Command::new("git");
                cmd.arg("clone")
                    .arg("--depth")
                    .arg("1")
                    .arg(git_url)
                    .arg(&dir);
                run_git(cmd)?;

                if let Ok(actual_branch) = detect_checked_out_branch(&dir) {
                    write_repo_branch(&dir, &actual_branch)?;
                } else {
                    write_repo_branch(&dir, branch)?;
                }

                return Ok(dir);
            }
        }
    }

    if !refresh {
        return Ok(dir);
    }

    let mut effective_branch = read_repo_branch(&dir).unwrap_or_else(|| branch.to_string());
    if effective_branch == "auto" {
        if let Ok(actual_branch) = detect_checked_out_branch(&dir) {
            effective_branch = actual_branch;
            write_repo_branch(&dir, &effective_branch)?;
        }
    }

    let mut cmd = Command::new("git");
    cmd.arg("-C")
        .arg(&dir)
        .arg("fetch")
        .arg("origin")
        .arg(&effective_branch)
        .arg("--depth")
        .arg("1");
    if let Err(err) = run_git(cmd) {
        if !is_remote_branch_not_found(&err) {
            return Err(err);
        }

        remove_path_if_exists(&dir)?;

        let mut cmd = Command::new("git");
        cmd.arg("clone")
            .arg("--depth")
            .arg("1")
            .arg(git_url)
            .arg(&dir);
        run_git(cmd)?;

        if let Ok(actual_branch) = detect_checked_out_branch(&dir) {
            write_repo_branch(&dir, &actual_branch)?;
        } else {
            write_repo_branch(&dir, branch)?;
        }

        return Ok(dir);
    }

    let mut cmd = Command::new("git");
    cmd.arg("-C")
        .arg(&dir)
        .arg("checkout")
        .arg("-B")
        .arg(&effective_branch)
        .arg(format!("origin/{effective_branch}"));
    run_git(cmd)?;

    let mut cmd = Command::new("git");
    cmd.arg("-C")
        .arg(&dir)
        .arg("reset")
        .arg("--hard")
        .arg(format!("origin/{effective_branch}"));
    run_git(cmd)?;

    Ok(dir)
}

fn ensure_repo_cache(
    app: &tauri::AppHandle,
    git_url: &str,
    branch: &str,
    refresh: bool,
) -> Result<PathBuf, String> {
    let git_url = git_url.trim();
    if git_url.is_empty() {
        return Err("SEC_INVALID_INPUT: git_url is required".to_string());
    }

    let branch = normalize_repo_branch(branch);

    if let Some((owner, repo)) = parse_github_owner_repo(git_url) {
        return ensure_github_repo_snapshot(app, git_url, &owner, &repo, &branch, refresh);
    }

    ensure_git_repo_cache(app, git_url, &branch, refresh)
}

fn validate_relative_subdir(subdir: &str) -> Result<(), String> {
    let subdir = subdir.trim();
    if subdir.is_empty() {
        return Err("SEC_INVALID_INPUT: source_subdir is required".to_string());
    }

    let p = Path::new(subdir);
    if p.is_absolute() {
        return Err("SEC_INVALID_INPUT: source_subdir must be relative".to_string());
    }

    for comp in p.components() {
        match comp {
            Component::CurDir | Component::Normal(_) => {}
            Component::ParentDir => {
                return Err("SEC_INVALID_INPUT: source_subdir must not contain '..'".to_string())
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err("SEC_INVALID_INPUT: source_subdir must be relative".to_string())
            }
        }
    }

    Ok(())
}

fn read_to_string(path: &Path) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("failed to read {}: {e}", path.display()))
}

fn strip_quotes(input: &str) -> &str {
    let s = input.trim();
    if s.len() >= 2 {
        let bytes = s.as_bytes();
        let first = bytes[0] as char;
        let last = bytes[s.len() - 1] as char;
        if (first == '"' && last == '"') || (first == '\'' && last == '\'') {
            return &s[1..s.len() - 1];
        }
    }
    s
}

fn parse_front_matter(text: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('#') {
            continue;
        }
        let Some((k, v)) = line.split_once(':') else {
            continue;
        };
        let key = k.trim().to_string();
        let value = strip_quotes(v).trim().to_string();
        if key.is_empty() {
            continue;
        }
        out.insert(key, value);
    }
    out
}

fn parse_skill_md(skill_md_path: &Path) -> Result<(String, String), String> {
    let text = read_to_string(skill_md_path)?;
    let text = text.trim_start();
    let mut lines = text.lines();
    let Some(first) = lines.next() else {
        return Err("SEC_INVALID_INPUT: SKILL.md is empty".to_string());
    };
    if first.trim() != "---" {
        return Err("SEC_INVALID_INPUT: SKILL.md front matter is required".to_string());
    }

    let mut fm = String::new();
    for line in lines {
        if line.trim() == "---" {
            break;
        }
        fm.push_str(line);
        fm.push('\n');
    }

    let map = parse_front_matter(&fm);
    let name = map.get("name").cloned().unwrap_or_default();
    let desc = map.get("description").cloned().unwrap_or_default();

    if name.trim().is_empty() {
        return Err("SEC_INVALID_INPUT: SKILL.md missing 'name'".to_string());
    }

    Ok((name.trim().to_string(), desc.trim().to_string()))
}

fn find_skill_md_files(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];

    while let Some(dir) = stack.pop() {
        let entries = std::fs::read_dir(&dir)
            .map_err(|e| format!("failed to read dir {}: {e}", dir.display()))?;
        for entry in entries {
            let entry =
                entry.map_err(|e| format!("failed to read dir entry {}: {e}", dir.display()))?;
            let path = entry.path();
            let file_name = entry.file_name();
            let file_name = file_name.to_string_lossy();

            if path.is_dir() {
                if file_name == ".git" {
                    continue;
                }
                stack.push(path);
                continue;
            }

            if file_name.eq_ignore_ascii_case("SKILL.md") {
                out.push(path);
            }
        }
    }

    Ok(out)
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dst).map_err(|e| format!("failed to create {}: {e}", dst.display()))?;
    let entries =
        std::fs::read_dir(src).map_err(|e| format!("failed to read dir {}: {e}", src.display()))?;
    for entry in entries {
        let entry =
            entry.map_err(|e| format!("failed to read dir entry {}: {e}", src.display()))?;
        let path = entry.path();
        let file_name = entry.file_name();
        let dst_path = dst.join(&file_name);
        if path.is_dir() {
            copy_dir_recursive(&path, &dst_path)?;
            continue;
        }
        std::fs::copy(&path, &dst_path).map_err(|e| {
            format!(
                "failed to copy {} -> {}: {e}",
                path.display(),
                dst_path.display()
            )
        })?;
    }
    Ok(())
}

fn write_marker(dir: &Path) -> Result<(), String> {
    let path = dir.join(MANAGED_MARKER_FILE);
    std::fs::write(&path, "aio-coding-hub\n")
        .map_err(|e| format!("failed to write marker {}: {e}", path.display()))
}

fn remove_marker(dir: &Path) {
    let path = dir.join(MANAGED_MARKER_FILE);
    let _ = std::fs::remove_file(path);
}

fn is_managed_dir(dir: &Path) -> bool {
    dir.join(MANAGED_MARKER_FILE).exists()
}

fn validate_dir_name(dir_name: &str) -> Result<String, String> {
    let dir_name = dir_name.trim();
    if dir_name.is_empty() {
        return Err("SEC_INVALID_INPUT: dir_name is required".to_string());
    }

    let p = Path::new(dir_name);
    let mut count = 0;
    for comp in p.components() {
        count += 1;
        match comp {
            Component::Normal(_) => {}
            Component::CurDir
            | Component::ParentDir
            | Component::RootDir
            | Component::Prefix(_) => {
                return Err(
                    "SEC_INVALID_INPUT: dir_name must be a single directory name".to_string(),
                )
            }
        }
    }

    if count != 1 {
        return Err("SEC_INVALID_INPUT: dir_name must be a single directory name".to_string());
    }

    Ok(dir_name.to_string())
}

pub fn local_list(app: &tauri::AppHandle, cli_key: &str) -> Result<Vec<LocalSkillSummary>, String> {
    validate_cli_key(cli_key)?;
    let root = cli_skills_root(app, cli_key)?;
    if !root.exists() {
        return Ok(Vec::new());
    }

    let entries = std::fs::read_dir(&root)
        .map_err(|e| format!("failed to read dir {}: {e}", root.display()))?;

    let mut out = Vec::new();
    for entry in entries {
        let entry =
            entry.map_err(|e| format!("failed to read dir entry {}: {e}", root.display()))?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        if is_managed_dir(&path) {
            continue;
        }

        let dir_name = path
            .file_name()
            .and_then(|v| v.to_str())
            .unwrap_or("")
            .to_string();
        if dir_name.is_empty() {
            continue;
        }

        let skill_md = path.join("SKILL.md");
        if !skill_md.exists() {
            continue;
        }

        let (name, description) = match parse_skill_md(&skill_md) {
            Ok((name, description)) => (name, description),
            Err(_) => (dir_name.clone(), String::new()),
        };

        out.push(LocalSkillSummary {
            dir_name,
            path: path.to_string_lossy().to_string(),
            name,
            description,
        });
    }

    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

pub fn import_local(
    app: &tauri::AppHandle,
    cli_key: &str,
    dir_name: &str,
) -> Result<InstalledSkillSummary, String> {
    validate_cli_key(cli_key)?;
    ensure_skills_roots(app)?;

    let dir_name = validate_dir_name(dir_name)?;

    let cli_root = cli_skills_root(app, cli_key)?;
    let local_dir = cli_root.join(&dir_name);
    if !local_dir.exists() {
        return Err(format!("SKILL_LOCAL_NOT_FOUND: {}", local_dir.display()));
    }
    if !local_dir.is_dir() {
        return Err("SEC_INVALID_INPUT: local skill path is not a directory".to_string());
    }
    if is_managed_dir(&local_dir) {
        return Err("SKILL_ALREADY_MANAGED: skill already managed by aio-coding-hub".to_string());
    }

    let skill_md = local_dir.join("SKILL.md");
    if !skill_md.exists() {
        return Err("SEC_INVALID_INPUT: SKILL.md not found in local skill dir".to_string());
    }

    let (name, description) = match parse_skill_md(&skill_md) {
        Ok(v) => v,
        Err(_) => (dir_name.clone(), String::new()),
    };
    let normalized_name = normalize_name(&name);

    let mut conn = db::open_connection(app)?;
    if skill_key_exists(&conn, &dir_name)? {
        return Err("SKILL_IMPORT_CONFLICT: same skill_key already exists".to_string());
    }

    let now = now_unix_seconds();
    let ssot_dir = ssot_skills_root(app)?.join(&dir_name);
    if ssot_dir.exists() {
        return Err("SKILL_IMPORT_CONFLICT: ssot dir already exists".to_string());
    }

    let enabled_flags = match cli_key {
        "claude" => (true, false, false),
        "codex" => (false, true, false),
        "gemini" => (false, false, true),
        _ => return Err(format!("SEC_INVALID_INPUT: unknown cli_key={cli_key}")),
    };

    let tx = conn
        .transaction()
        .map_err(|e| format!("DB_ERROR: failed to start transaction: {e}"))?;

    tx.execute(
        r#"
INSERT INTO skills(
  skill_key,
  name,
  normalized_name,
  description,
  source_git_url,
  source_branch,
  source_subdir,
  enabled_claude,
  enabled_codex,
  enabled_gemini,
  created_at,
  updated_at
) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
"#,
        params![
            dir_name,
            name.trim(),
            normalized_name,
            description,
            format!("local://{cli_key}"),
            "local",
            dir_name,
            enabled_to_int(enabled_flags.0),
            enabled_to_int(enabled_flags.1),
            enabled_to_int(enabled_flags.2),
            now,
            now
        ],
    )
    .map_err(|e| format!("DB_ERROR: failed to insert imported skill: {e}"))?;

    let skill_id = tx.last_insert_rowid();

    if let Err(err) = copy_dir_recursive(&local_dir, &ssot_dir) {
        let _ = std::fs::remove_dir_all(&ssot_dir);
        let _ = tx.execute("DELETE FROM skills WHERE id = ?1", params![skill_id]);
        return Err(err);
    }

    if let Err(err) = write_marker(&local_dir) {
        let _ = std::fs::remove_dir_all(&ssot_dir);
        let _ = tx.execute("DELETE FROM skills WHERE id = ?1", params![skill_id]);
        return Err(err);
    }

    if let Err(err) = tx.commit() {
        let _ = std::fs::remove_dir_all(&ssot_dir);
        remove_marker(&local_dir);
        return Err(format!("DB_ERROR: failed to commit: {err}"));
    }

    get_skill_by_id(&conn, skill_id)
}

fn remove_managed_dir(dir: &Path) -> Result<(), String> {
    if !dir.exists() {
        return Ok(());
    }
    if !is_managed_dir(dir) {
        return Err(format!(
            "SKILL_REMOVE_BLOCKED_UNMANAGED: target exists but is not managed: {}",
            dir.display()
        ));
    }
    std::fs::remove_dir_all(dir).map_err(|e| format!("failed to remove {}: {e}", dir.display()))?;
    Ok(())
}

fn skill_key_exists(conn: &Connection, key: &str) -> Result<bool, String> {
    let exists: Option<i64> = conn
        .query_row(
            "SELECT id FROM skills WHERE skill_key = ?1",
            params![key],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| format!("DB_ERROR: failed to query skill_key: {e}"))?;
    Ok(exists.is_some())
}

fn suggest_key(name: &str) -> String {
    let mut out = String::new();
    let mut prev_dash = false;
    for ch in name.trim().chars() {
        let lower = ch.to_ascii_lowercase();
        if lower.is_ascii_alphanumeric() {
            out.push(lower);
            prev_dash = false;
            continue;
        }
        if lower == '_' || lower == '-' {
            if !out.is_empty() && !prev_dash {
                out.push('-');
                prev_dash = true;
            }
            continue;
        }
        if !out.is_empty() && !prev_dash {
            out.push('-');
            prev_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        "skill".to_string()
    } else {
        out
    }
}

fn generate_unique_skill_key(conn: &Connection, name: &str) -> Result<String, String> {
    let base = suggest_key(name);
    if !skill_key_exists(conn, &base)? {
        return Ok(base);
    }
    for idx in 2..1000 {
        let candidate = format!("{base}-{idx}");
        if !skill_key_exists(conn, &candidate)? {
            return Ok(candidate);
        }
    }
    Ok(format!("skill-{}", now_unix_seconds()))
}

fn row_to_repo(row: &rusqlite::Row<'_>) -> Result<SkillRepoSummary, rusqlite::Error> {
    Ok(SkillRepoSummary {
        id: row.get("id")?,
        git_url: row.get("git_url")?,
        branch: row.get("branch")?,
        enabled: row.get::<_, i64>("enabled")? != 0,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

fn row_to_installed(row: &rusqlite::Row<'_>) -> Result<InstalledSkillSummary, rusqlite::Error> {
    Ok(InstalledSkillSummary {
        id: row.get("id")?,
        skill_key: row.get("skill_key")?,
        name: row.get("name")?,
        description: row.get("description")?,
        source_git_url: row.get("source_git_url")?,
        source_branch: row.get("source_branch")?,
        source_subdir: row.get("source_subdir")?,
        enabled_claude: row.get::<_, i64>("enabled_claude")? != 0,
        enabled_codex: row.get::<_, i64>("enabled_codex")? != 0,
        enabled_gemini: row.get::<_, i64>("enabled_gemini")? != 0,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

pub fn repos_list(app: &tauri::AppHandle) -> Result<Vec<SkillRepoSummary>, String> {
    let conn = db::open_connection(app)?;
    let mut stmt = conn
        .prepare(
            r#"
SELECT
  id,
  git_url,
  branch,
  enabled,
  created_at,
  updated_at
FROM skill_repos
ORDER BY updated_at DESC, id DESC
"#,
        )
        .map_err(|e| format!("DB_ERROR: failed to prepare repo list query: {e}"))?;

    let rows = stmt
        .query_map([], row_to_repo)
        .map_err(|e| format!("DB_ERROR: failed to query repos: {e}"))?;

    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| format!("DB_ERROR: failed to read repo row: {e}"))?);
    }

    // De-dup repos by canonical git URL for a clearer UX.
    // Keeps the newest record (query is already ordered by updated_at DESC).
    let mut seen = HashSet::new();
    let mut deduped = Vec::new();
    for row in out {
        let key = canonical_git_url_key(&row.git_url);
        let key = if key.is_empty() {
            row.git_url.trim().to_ascii_lowercase()
        } else {
            key
        };
        if seen.insert(key) {
            deduped.push(row);
        }
    }

    Ok(deduped)
}

pub fn repo_upsert(
    app: &tauri::AppHandle,
    repo_id: Option<i64>,
    git_url: &str,
    branch: &str,
    enabled: bool,
) -> Result<SkillRepoSummary, String> {
    let git_url = git_url.trim();
    if git_url.is_empty() {
        return Err("SEC_INVALID_INPUT: git_url is required".to_string());
    }
    let branch = normalize_repo_branch(branch);

    let conn = db::open_connection(app)?;
    let now = now_unix_seconds();

    match repo_id {
        None => {
            let canonical = canonical_git_url_key(git_url);
            let canonical = if canonical.is_empty() {
                git_url.to_ascii_lowercase()
            } else {
                canonical
            };

            let mut stmt = conn
                .prepare(
                    r#"
SELECT id, git_url, branch
FROM skill_repos
ORDER BY updated_at DESC, id DESC
"#,
                )
                .map_err(|e| format!("DB_ERROR: failed to prepare repo lookup: {e}"))?;

            let rows = stmt
                .query_map([], |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                })
                .map_err(|e| format!("DB_ERROR: failed to query repos: {e}"))?;

            let mut matches = Vec::new();
            for row in rows {
                let (id, existing_url, existing_branch) =
                    row.map_err(|e| format!("DB_ERROR: failed to read repo row: {e}"))?;
                let key = canonical_git_url_key(&existing_url);
                let key = if key.is_empty() {
                    existing_url.trim().to_ascii_lowercase()
                } else {
                    key
                };
                if key == canonical {
                    matches.push((id, existing_url, existing_branch));
                }
            }

            if !matches.is_empty() {
                let mut target_id = matches[0].0;
                for (id, existing_url, existing_branch) in &matches {
                    if existing_url.trim() == git_url
                        && normalize_repo_branch(existing_branch) == branch
                    {
                        target_id = *id;
                        break;
                    }
                }

                conn.execute(
                    r#"
UPDATE skill_repos
SET
  git_url = ?1,
  branch = ?2,
  enabled = ?3,
  updated_at = ?4
WHERE id = ?5
"#,
                    params![git_url, branch, enabled_to_int(enabled), now, target_id],
                )
                .map_err(|e| format!("DB_ERROR: failed to update skill repo: {e}"))?;

                return get_repo_by_id(&conn, target_id);
            }

            conn.execute(
                r#"
INSERT INTO skill_repos(
  git_url,
  branch,
  enabled,
  created_at,
  updated_at
) VALUES (?1, ?2, ?3, ?4, ?5)
"#,
                params![git_url, branch, enabled_to_int(enabled), now, now],
            )
            .map_err(|e| format!("DB_ERROR: failed to insert skill repo: {e}"))?;

            let id = conn.last_insert_rowid();
            get_repo_by_id(&conn, id)
        }
        Some(id) => {
            conn.execute(
                r#"
UPDATE skill_repos
SET
  git_url = ?1,
  branch = ?2,
  enabled = ?3,
  updated_at = ?4
WHERE id = ?5
"#,
                params![git_url, branch, enabled_to_int(enabled), now, id],
            )
            .map_err(|e| format!("DB_ERROR: failed to update skill repo: {e}"))?;
            get_repo_by_id(&conn, id)
        }
    }
}

fn get_repo_by_id(conn: &Connection, repo_id: i64) -> Result<SkillRepoSummary, String> {
    conn.query_row(
        r#"
SELECT
  id,
  git_url,
  branch,
  enabled,
  created_at,
  updated_at
FROM skill_repos
WHERE id = ?1
"#,
        params![repo_id],
        row_to_repo,
    )
    .optional()
    .map_err(|e| format!("DB_ERROR: failed to query repo: {e}"))?
    .ok_or_else(|| "DB_NOT_FOUND: skill repo not found".to_string())
}

pub fn repo_delete(app: &tauri::AppHandle, repo_id: i64) -> Result<(), String> {
    let conn = db::open_connection(app)?;
    let changed = conn
        .execute("DELETE FROM skill_repos WHERE id = ?1", params![repo_id])
        .map_err(|e| format!("DB_ERROR: failed to delete skill repo: {e}"))?;
    if changed == 0 {
        return Err("DB_NOT_FOUND: skill repo not found".to_string());
    }
    Ok(())
}

pub fn installed_list(app: &tauri::AppHandle) -> Result<Vec<InstalledSkillSummary>, String> {
    let conn = db::open_connection(app)?;
    let mut stmt = conn
        .prepare(
            r#"
SELECT
  id,
  skill_key,
  name,
  normalized_name,
  description,
  source_git_url,
  source_branch,
  source_subdir,
  enabled_claude,
  enabled_codex,
  enabled_gemini,
  created_at,
  updated_at
FROM skills
ORDER BY updated_at DESC, id DESC
"#,
        )
        .map_err(|e| format!("DB_ERROR: failed to prepare installed list query: {e}"))?;

    let rows = stmt
        .query_map([], row_to_installed)
        .map_err(|e| format!("DB_ERROR: failed to list skills: {e}"))?;

    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| format!("DB_ERROR: failed to read skill row: {e}"))?);
    }
    Ok(out)
}

fn installed_source_set(conn: &Connection) -> Result<HashSet<String>, String> {
    let mut stmt = conn
        .prepare(
            r#"
SELECT source_git_url, source_branch, source_subdir
FROM skills
"#,
        )
        .map_err(|e| format!("DB_ERROR: failed to prepare installed source query: {e}"))?;
    let rows = stmt
        .query_map([], |row| {
            let url: String = row.get(0)?;
            let branch: String = row.get(1)?;
            let subdir: String = row.get(2)?;
            Ok(format!("{}#{}#{}", url, branch, subdir))
        })
        .map_err(|e| format!("DB_ERROR: failed to query installed sources: {e}"))?;

    let mut set = HashSet::new();
    for row in rows {
        set.insert(row.map_err(|e| format!("DB_ERROR: failed to read installed source row: {e}"))?);
    }
    Ok(set)
}

pub fn discover_available(
    app: &tauri::AppHandle,
    refresh: bool,
) -> Result<Vec<AvailableSkillSummary>, String> {
    fn subdir_score(source_subdir: &str) -> i32 {
        let subdir = source_subdir.trim_matches('/').to_ascii_lowercase();
        let mut score = 0;

        if subdir.starts_with(".claude/skills/") {
            score += 100;
        }
        if subdir.starts_with(".codex/skills/") {
            score += 100;
        }
        if subdir.starts_with(".gemini/skills/") {
            score += 100;
        }

        if subdir.starts_with("skills/") {
            score += 80;
        }

        if subdir.starts_with("cli/assets/") || subdir.contains("/cli/assets/") {
            score -= 120;
        }
        if subdir.starts_with("assets/") || subdir.contains("/assets/") {
            score -= 30;
        }
        if subdir.starts_with("examples/") || subdir.contains("/examples/") {
            score -= 20;
        }

        score
    }

    fn prefer_candidate(a: &AvailableSkillSummary, b: &AvailableSkillSummary) -> bool {
        if a.installed != b.installed {
            return b.installed;
        }

        let score_a = subdir_score(&a.source_subdir);
        let score_b = subdir_score(&b.source_subdir);
        if score_a != score_b {
            return score_b > score_a;
        }

        if a.source_subdir.len() != b.source_subdir.len() {
            return b.source_subdir.len() < a.source_subdir.len();
        }

        b.source_subdir < a.source_subdir
    }

    let conn = db::open_connection(app)?;

    let installed_sources = installed_source_set(&conn)?;

    let mut stmt = conn
        .prepare(
            r#"
SELECT git_url, branch
FROM skill_repos
WHERE enabled = 1
ORDER BY updated_at DESC, id DESC
"#,
        )
        .map_err(|e| format!("DB_ERROR: failed to prepare repo query: {e}"))?;

    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|e| format!("DB_ERROR: failed to query enabled repos: {e}"))?;

    let mut repos = Vec::new();
    let mut seen_repos = HashSet::new();
    for row in rows {
        let (git_url, branch) =
            row.map_err(|e| format!("DB_ERROR: failed to read repo row: {e}"))?;
        let key = canonical_git_url_key(&git_url);
        let key = if key.is_empty() {
            git_url.trim().to_ascii_lowercase()
        } else {
            key
        };
        if seen_repos.insert(key) {
            repos.push((git_url, branch));
        }
    }

    let mut out = Vec::new();
    for (git_url, branch) in repos {
        let repo_dir = ensure_repo_cache(app, &git_url, &branch, refresh)?;
        let skill_mds = find_skill_md_files(&repo_dir)?;

        let mut best_by_name: BTreeMap<String, AvailableSkillSummary> = BTreeMap::new();

        for skill_md in skill_mds {
            let skill_dir = skill_md
                .parent()
                .ok_or_else(|| "SEC_INVALID_INPUT: invalid SKILL.md path".to_string())?;

            let (name, description) = match parse_skill_md(&skill_md) {
                Ok(v) => v,
                Err(_) => continue,
            };

            let subdir_rel = skill_dir.strip_prefix(&repo_dir).map_err(|_| {
                "SEC_INVALID_INPUT: failed to compute skill relative path".to_string()
            })?;
            let source_subdir = subdir_rel
                .to_string_lossy()
                .replace('\\', "/")
                .trim_matches('/')
                .to_string();

            if source_subdir.is_empty() {
                continue;
            }

            let installed =
                installed_sources.contains(&format!("{}#{}#{}", git_url, branch, source_subdir));

            let candidate = AvailableSkillSummary {
                name,
                description,
                source_git_url: git_url.clone(),
                source_branch: branch.clone(),
                source_subdir,
                installed,
            };

            let key = normalize_name(&candidate.name);
            match best_by_name.get_mut(&key) {
                None => {
                    best_by_name.insert(key, candidate);
                }
                Some(existing) => {
                    if prefer_candidate(existing, &candidate) {
                        *existing = candidate;
                    }
                }
            }
        }

        out.extend(best_by_name.into_values());
    }

    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

fn ensure_skills_roots(app: &tauri::AppHandle) -> Result<(), String> {
    std::fs::create_dir_all(ssot_skills_root(app)?)
        .map_err(|e| format!("failed to create ssot skills dir: {e}"))?;
    std::fs::create_dir_all(repos_root(app)?)
        .map_err(|e| format!("failed to create repos dir: {e}"))?;
    Ok(())
}

fn sync_to_cli(
    app: &tauri::AppHandle,
    cli_key: &str,
    skill_key: &str,
    ssot_dir: &Path,
) -> Result<(), String> {
    let cli_root = cli_skills_root(app, cli_key)?;
    std::fs::create_dir_all(&cli_root)
        .map_err(|e| format!("failed to create {}: {e}", cli_root.display()))?;
    let target = cli_root.join(skill_key);

    if target.exists() {
        if !is_managed_dir(&target) {
            return Err(format!(
                "SKILL_TARGET_EXISTS_UNMANAGED: {}",
                target.display()
            ));
        }
        std::fs::remove_dir_all(&target)
            .map_err(|e| format!("failed to remove {}: {e}", target.display()))?;
    }

    copy_dir_recursive(ssot_dir, &target)?;
    write_marker(&target)?;
    Ok(())
}

fn remove_from_cli(app: &tauri::AppHandle, cli_key: &str, skill_key: &str) -> Result<(), String> {
    let cli_root = cli_skills_root(app, cli_key)?;
    let target = cli_root.join(skill_key);
    if !target.exists() {
        return Ok(());
    }
    remove_managed_dir(&target)
}

fn get_skill_by_id(conn: &Connection, skill_id: i64) -> Result<InstalledSkillSummary, String> {
    conn.query_row(
        r#"
SELECT
  id,
  skill_key,
  name,
  normalized_name,
  description,
  source_git_url,
  source_branch,
  source_subdir,
  enabled_claude,
  enabled_codex,
  enabled_gemini,
  created_at,
  updated_at
FROM skills
WHERE id = ?1
"#,
        params![skill_id],
        row_to_installed,
    )
    .optional()
    .map_err(|e| format!("DB_ERROR: failed to query skill: {e}"))?
    .ok_or_else(|| "DB_NOT_FOUND: skill not found".to_string())
}

pub fn install(
    app: &tauri::AppHandle,
    git_url: &str,
    branch: &str,
    source_subdir: &str,
    enabled_claude: bool,
    enabled_codex: bool,
    enabled_gemini: bool,
) -> Result<InstalledSkillSummary, String> {
    ensure_skills_roots(app)?;
    validate_relative_subdir(source_subdir)?;

    let mut conn = db::open_connection(app)?;
    let now = now_unix_seconds();

    // Ensure source not already installed.
    let existing_id: Option<i64> = conn
        .query_row(
            r#"
SELECT id
FROM skills
WHERE source_git_url = ?1 AND source_branch = ?2 AND source_subdir = ?3
LIMIT 1
"#,
            params![git_url.trim(), branch.trim(), source_subdir.trim()],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| format!("DB_ERROR: failed to query skill by source: {e}"))?;
    if existing_id.is_some() {
        return Err("SKILL_ALREADY_INSTALLED: skill already installed".to_string());
    }

    let repo_dir = ensure_repo_cache(app, git_url, branch, true)?;
    let src_dir = repo_dir.join(source_subdir.trim());
    if !src_dir.exists() {
        return Err(format!("SKILL_SOURCE_NOT_FOUND: {}", src_dir.display()));
    }

    let skill_md = src_dir.join("SKILL.md");
    if !skill_md.exists() {
        return Err("SEC_INVALID_INPUT: SKILL.md not found in source_subdir".to_string());
    }

    let (name, description) = parse_skill_md(&skill_md)?;
    let normalized_name = normalize_name(&name);

    let tx = conn
        .transaction()
        .map_err(|e| format!("DB_ERROR: failed to start transaction: {e}"))?;

    let skill_key = generate_unique_skill_key(&tx, &name)?;
    let ssot_root = ssot_skills_root(app)?;
    let ssot_dir = ssot_root.join(&skill_key);
    if ssot_dir.exists() {
        return Err("SKILL_CONFLICT: ssot dir already exists".to_string());
    }

    tx.execute(
        r#"
INSERT INTO skills(
  skill_key,
  name,
  normalized_name,
  description,
  source_git_url,
  source_branch,
  source_subdir,
  enabled_claude,
  enabled_codex,
  enabled_gemini,
  created_at,
  updated_at
) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
"#,
        params![
            skill_key,
            name.trim(),
            normalized_name,
            description,
            git_url.trim(),
            branch.trim(),
            source_subdir.trim(),
            enabled_to_int(enabled_claude),
            enabled_to_int(enabled_codex),
            enabled_to_int(enabled_gemini),
            now,
            now
        ],
    )
    .map_err(|e| format!("DB_ERROR: failed to insert skill: {e}"))?;

    let skill_id = tx.last_insert_rowid();

    // FS: copy to SSOT first.
    if let Err(err) = copy_dir_recursive(&src_dir, &ssot_dir) {
        let _ = std::fs::remove_dir_all(&ssot_dir);
        let _ = tx.execute("DELETE FROM skills WHERE id = ?1", params![skill_id]);
        return Err(err);
    }

    // FS: sync to enabled CLIs.
    let sync_steps = [
        ("claude", enabled_claude),
        ("codex", enabled_codex),
        ("gemini", enabled_gemini),
    ];

    for (cli_key, enabled) in sync_steps {
        if !enabled {
            continue;
        }
        if let Err(err) = sync_to_cli(app, cli_key, &skill_key, &ssot_dir) {
            let _ = remove_from_cli(app, "claude", &skill_key);
            let _ = remove_from_cli(app, "codex", &skill_key);
            let _ = remove_from_cli(app, "gemini", &skill_key);
            let _ = std::fs::remove_dir_all(&ssot_dir);
            let _ = tx.execute("DELETE FROM skills WHERE id = ?1", params![skill_id]);
            return Err(err);
        }
    }

    if let Err(err) = tx.commit() {
        let _ = remove_from_cli(app, "claude", &skill_key);
        let _ = remove_from_cli(app, "codex", &skill_key);
        let _ = remove_from_cli(app, "gemini", &skill_key);
        let _ = std::fs::remove_dir_all(&ssot_dir);
        return Err(format!("DB_ERROR: failed to commit: {err}"));
    }

    get_skill_by_id(&conn, skill_id)
}

pub fn set_enabled(
    app: &tauri::AppHandle,
    skill_id: i64,
    cli_key: &str,
    enabled: bool,
) -> Result<InstalledSkillSummary, String> {
    validate_cli_key(cli_key)?;

    let conn = db::open_connection(app)?;
    let now = now_unix_seconds();

    let current = get_skill_by_id(&conn, skill_id)?;
    let ssot_root = ssot_skills_root(app)?;
    let ssot_dir = ssot_root.join(&current.skill_key);
    if !ssot_dir.exists() {
        return Err("SKILL_SSOT_MISSING: ssot skill dir not found".to_string());
    }

    if enabled {
        sync_to_cli(app, cli_key, &current.skill_key, &ssot_dir)?;
    } else {
        remove_from_cli(app, cli_key, &current.skill_key)?;
    }

    let column = match cli_key {
        "claude" => "enabled_claude",
        "codex" => "enabled_codex",
        "gemini" => "enabled_gemini",
        _ => return Err(format!("SEC_INVALID_INPUT: unknown cli_key={cli_key}")),
    };

    let sql = format!("UPDATE skills SET {column} = ?1, updated_at = ?2 WHERE id = ?3");
    conn.execute(&sql, params![enabled_to_int(enabled), now, skill_id])
        .map_err(|e| format!("DB_ERROR: failed to update skill enabled: {e}"))?;

    get_skill_by_id(&conn, skill_id)
}

pub fn uninstall(app: &tauri::AppHandle, skill_id: i64) -> Result<(), String> {
    let conn = db::open_connection(app)?;
    let skill = get_skill_by_id(&conn, skill_id)?;

    // Safety: ensure we will only delete managed dirs.
    let cli_roots = [
        ("claude", cli_skills_root(app, "claude")?),
        ("codex", cli_skills_root(app, "codex")?),
        ("gemini", cli_skills_root(app, "gemini")?),
    ];
    for (_cli, root) in &cli_roots {
        let target = root.join(&skill.skill_key);
        if target.exists() && !is_managed_dir(&target) {
            return Err(format!(
                "SKILL_REMOVE_BLOCKED_UNMANAGED: {}",
                target.display()
            ));
        }
    }

    remove_from_cli(app, "claude", &skill.skill_key)?;
    remove_from_cli(app, "codex", &skill.skill_key)?;
    remove_from_cli(app, "gemini", &skill.skill_key)?;

    let ssot_dir = ssot_skills_root(app)?.join(&skill.skill_key);
    if ssot_dir.exists() {
        std::fs::remove_dir_all(&ssot_dir)
            .map_err(|e| format!("failed to remove {}: {e}", ssot_dir.display()))?;
    }

    let changed = conn
        .execute("DELETE FROM skills WHERE id = ?1", params![skill_id])
        .map_err(|e| format!("DB_ERROR: failed to delete skill: {e}"))?;
    if changed == 0 {
        return Err("DB_NOT_FOUND: skill not found".to_string());
    }
    Ok(())
}

pub fn paths_get(app: &tauri::AppHandle, cli_key: &str) -> Result<SkillsPaths, String> {
    validate_cli_key(cli_key)?;
    let ssot = ssot_skills_root(app)?;
    let repos = repos_root(app)?;
    let cli = cli_skills_root(app, cli_key)?;

    Ok(SkillsPaths {
        ssot_dir: ssot.to_string_lossy().to_string(),
        repos_dir: repos.to_string_lossy().to_string(),
        cli_dir: cli.to_string_lossy().to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::{github_api_url, now_unix_nanos, parse_github_owner_repo, unzip_repo_zip};
    use std::io::{Cursor, Write};
    use std::path::PathBuf;

    fn make_temp_dir(prefix: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("{prefix}-{}", now_unix_nanos()));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    #[test]
    fn parse_github_owner_repo_handles_common_urls() {
        assert_eq!(
            parse_github_owner_repo("https://github.com/owner/repo.git"),
            Some(("owner".to_string(), "repo".to_string()))
        );
        assert_eq!(
            parse_github_owner_repo("git@github.com:Owner/Repo.git"),
            Some(("owner".to_string(), "repo".to_string()))
        );
        assert_eq!(
            parse_github_owner_repo("https://github.com/owner/repo/tree/main/skills"),
            Some(("owner".to_string(), "repo".to_string()))
        );
        assert_eq!(
            parse_github_owner_repo("https://gitlab.com/owner/repo"),
            None
        );
    }

    #[test]
    fn github_api_url_encodes_branch_path_segments() {
        let url = github_api_url(&["repos", "owner", "repo", "zipball", "feature/x"]).expect("url");
        let s = url.to_string();
        assert!(
            s.contains("feature%2Fx"),
            "expected encoded branch in url, got: {s}"
        );
    }

    #[test]
    fn unzip_repo_zip_rejects_path_traversal_entries() {
        let mut buf = Cursor::new(Vec::new());
        let mut zip = zip::ZipWriter::new(&mut buf);
        let opts = zip::write::FileOptions::<()>::default();

        zip.add_directory("repo/", opts).expect("add dir");
        zip.start_file("..\\evil.txt", opts).expect("start file");
        zip.write_all(b"evil").expect("write");
        zip.finish().expect("finish zip");

        let bytes = buf.into_inner();
        let out_dir = make_temp_dir("aio-unzip-test");
        let err = unzip_repo_zip(&bytes, &out_dir).unwrap_err();

        assert!(
            err.starts_with("SKILL_ZIP_ERROR:"),
            "unexpected error: {err}"
        );

        let _ = std::fs::remove_dir_all(&out_dir);
    }

    #[test]
    fn unzip_repo_zip_accepts_backslash_paths_inside_repo() {
        let mut buf = Cursor::new(Vec::new());
        let mut zip = zip::ZipWriter::new(&mut buf);
        let opts = zip::write::FileOptions::<()>::default();

        zip.add_directory("repo\\", opts).expect("add dir");
        zip.add_directory("repo\\nested\\", opts)
            .expect("add nested dir");
        zip.start_file("repo\\nested\\SKILL.md", opts)
            .expect("start file");
        zip.write_all(b"---\nname: Test\n---\n").expect("write");
        zip.finish().expect("finish zip");

        let bytes = buf.into_inner();
        let out_dir = make_temp_dir("aio-unzip-test-ok");
        let repo_root = unzip_repo_zip(&bytes, &out_dir).expect("unzip");

        assert!(repo_root.join("nested").join("SKILL.md").exists());

        let _ = std::fs::remove_dir_all(&out_dir);
    }
}
