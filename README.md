<div align="center">
  <img src="public/logo.jpg" width="120" alt="AIO Coding Hub Logo" />

# AIO Coding Hub

**本地 AI CLI 统一网关** — 让 Claude Code / Codex / Gemini CLI 请求走同一个入口

[![Release](https://img.shields.io/github/v/release/dyndynjyxa/aio-coding-hub?style=flat-square)](https://github.com/dyndynjyxa/aio-coding-hub/releases)
[![License](https://img.shields.io/badge/license-MIT-blue?style=flat-square)](LICENSE)
[![Platform](https://img.shields.io/badge/platform-Windows%20|%20macOS%20|%20Linux-lightgrey?style=flat-square)](#安装)

简体中文 | [English](./README_EN.md)

</div>

> **🙏 致谢**
> 本项目借鉴并参考了以下优秀开源项目理念：
> - [cc-switch](https://github.com/farion1231/cc-switch)
> - [claude-code-hub](https://github.com/ding113/claude-code-hub)
> - [code-switch-R](https://github.com/Rogers-F/code-switch-R)

---

## 为什么需要它？

| 痛点 | AIO Coding Hub 的解决方案 |
|------|--------------------------|
| 每个 CLI 都要单独配置 `base_url` 和 API Key | **统一入口** — 所有 CLI 走 `127.0.0.1` 本机网关 |
| 上游不稳定时请求直接失败 | **智能 Failover** — 自动切换 Provider，熔断保护 |
| 不知道请求去了哪里、用了多少 Token | **全链路可观测** — Trace 追踪、Console 日志、用量统计 |
| 切换 Provider 要改多个配置文件 | **一键代理** — 开关即切换，自动备份原配置 |

---

## 核心特性

<table>
<tr>
<td width="50%">

### 🔀 统一网关代理

- 单一入口 `127.0.0.1:37123`
- 支持 Claude Code、Codex、Gemini CLI
- OpenAI 兼容层 `/v1/*`
- 自动注入鉴权，CLI 无需保存真实 Key

</td>
<td width="50%">

### ⚡ 智能路由与容错

- 多 Provider 优先级排序
- 自动 Failover（网络错误/401/403/429/5xx）
- 熔断器模式防止雪崩
- 会话粘滞保证对话一致性

</td>
</tr>
<tr>
<td width="50%">

### 📊 可观测性与统计

- 请求 Trace（`x-trace-id`）
- 实时 Console 日志
- Token 用量统计与成本估算
- 用量热力图可视化

</td>
<td width="50%">

### 🎛️ 桌面级体验

- 原生跨平台（Windows / macOS / Linux）
- 系统托盘常驻
- 开机自启动（可选）
- CLI 配置一键开关

</td>
</tr>
<tr>
<td width="50%">

### 🔍 渠道验证与模型鉴别

- **多维度验证模板**
  - `max_tokens=5` 截断测试 + cache_creation 细分字段检测
  - Extended Thinking 输出 + 签名验证 + 结构字段完整性
- **官方渠道特征检测**
  - 模型一致性（请求 vs 响应模型）
  - 输出长度精准控制验证
  - 多轮对话暗号传递验证
  - SSE 流式响应 stop_reason 检查
  - Response ID / Service Tier / Tool Support 等结构字段
- **批量验证与历史记录**（1-50 次可配置）

</td>
<td width="50%">

### 🔐 安全与隐私

- 所有数据本地存储
- API Key 加密保存
- 无需联网验证 License
- 开源可审计

</td>
</tr>
</table>

---

## 架构概览

```
┌─────────────────────────────────────────────────────────────────┐
│                        AIO Coding Hub                           │
├─────────────────────────────────────────────────────────────────┤
│  ┌─────────┐   ┌─────────┐   ┌─────────┐                       │
│  │ Claude  │   │  Codex  │   │ Gemini  │  ← AI CLI Tools       │
│  │  Code   │   │   CLI   │   │   CLI   │                       │
│  └────┬────┘   └────┬────┘   └────┬────┘                       │
│       │             │             │                             │
│       └─────────────┼─────────────┘                             │
│                     ▼                                           │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │              Local Gateway (127.0.0.1:37123)             │  │
│  │  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐ │  │
│  │  │ Failover │  │ Circuit  │  │ Session  │  │  Usage   │ │  │
│  │  │  Engine  │  │ Breaker  │  │ Manager  │  │ Tracker  │ │  │
│  │  └──────────┘  └──────────┘  └──────────┘  └──────────┘ │  │
│  └──────────────────────────────────────────────────────────┘  │
│                     │                                           │
│       ┌─────────────┼─────────────┐                             │
│       ▼             ▼             ▼                             │
│  ┌─────────┐   ┌─────────┐   ┌─────────┐                       │
│  │Provider │   │Provider │   │Provider │  ← Upstream APIs      │
│  │    A    │   │    B    │   │    C    │                       │
│  └─────────┘   └─────────┘   └─────────┘                       │
└─────────────────────────────────────────────────────────────────┘
```

---

## 安装

### 从 Release 下载（推荐）

前往 [Releases](https://github.com/dyndynjyxa/aio-coding-hub/releases) 下载对应平台的安装包：

| 平台 | 安装包 |
|------|--------|
| **Windows** | `.exe` (NSIS) 或 `.msi` |
| **macOS** | `.dmg` |
| **Linux** | `.deb` / `.AppImage` |

<details>
<summary>macOS 安全提示</summary>

若遇到"无法打开/来源未验证"提示，执行：

```bash
sudo xattr -cr /Applications/"AIO Coding Hub.app"
```

</details>

### 从源码构建

```bash
# 前置：Node.js 18+、pnpm、Rust 1.70+
git clone https://github.com/dyndynjyxa/aio-coding-hub.git
cd aio-coding-hub
pnpm install
pnpm tauri:build
```

---

## 快速开始

**3 步完成配置：**

```
1️⃣  Providers 页 → 添加上游（官方 API / 自建代理 / 公司网关）
2️⃣  Home 页 → 打开目标 CLI 的"代理"开关
3️⃣  终端发起请求 → Console/Usage 查看 Trace 与用量
```

**验证网关运行：**

```bash
curl http://127.0.0.1:37123/health
# 预期输出: {"status":"ok"}
```

---

## 技术栈

| 层级 | 技术 |
|------|------|
| **前端** | React 19 · TypeScript · Tailwind CSS · Vite |
| **桌面框架** | Tauri 2 |
| **后端** | Rust · Axum (HTTP Gateway) |
| **数据库** | SQLite (rusqlite) |
| **通信** | Tauri IPC · Server-Sent Events |

---

## 文档

| 文档 | 说明 |
|------|------|
| [使用指南](docs/usage.md) | 完整配置流程与网关入口说明 |
| [CLI 代理机制](docs/cli-proxy.md) | 配置文件变更与备份策略 |
| [数据与安全](docs/data-and-security.md) | 数据存储位置与安全提示 |
| [常见问题](docs/troubleshooting.md) | FAQ 与排障指南 |
| [开发指南](docs/development.md) | 本地开发与质量门禁 |
| [发版说明](docs/releasing.md) | 版本发布与自动更新 |

---

## 不适用场景

- 公网部署 / 远程访问 / 多租户
- 企业级 RBAC 权限管理

> 本项目定位为 **单机桌面工具 + 本地网关**，所有数据保存在本机用户目录。

---

## 参与贡献

欢迎提交 Issue 和 PR！项目采用 [Conventional Commits](https://www.conventionalcommits.org/) 规范。

```bash
# PR 标题格式
feat(ui): add usage heatmap
fix(gateway): handle timeout correctly
docs: update installation guide
```

详见 [CONTRIBUTING.md](CONTRIBUTING.md)（如有）。

---

## 许可证

[MIT License](LICENSE)

---

<div align="center">

**如果觉得有用，请给个 ⭐ Star！**

</div>
