# Aries

[![CI](https://github.com/ltoddy/aries/actions/workflows/ci.yml/badge.svg)](https://github.com/ltoddy/aries/actions/workflows/ci.yml)
![Rust](https://img.shields.io/badge/rust-nightly-blue)
![License](https://img.shields.io/badge/license-MIT-green)
![Rust](https://img.shields.io/badge/rust-2024-orange)

## 项目名称来源

本项目命名为 **Aries**（白羊座），因为项目创建时正值白羊座时期（3月21日 - 4月19日）。象征着全新的开始与充满活力的探索。

## Apps

### Aries CLI (`aries`)

终端 AI 助手，通过 Agent Client Protocol 与各类 IDE 集成，支持模型管理、网络访问等功能。

![terminal](docs/assets/aries-cli-terminal.png)

### Aries Desktop (`aries-desktop`)

原生 macOS 终端模拟器，内置 AI 能力。基于 GPUI 构建，提供高性能的原生体验。

![orbit](docs/assets/aries-desktop.png)

以 Agent 为核心，统一承接终端、桌面与 IDE 的交互，通过上下文管理、工具执行和子智能体协作完成任务。

```mermaid
flowchart TB
    subgraph interaction[用户交互]
        CLI["终端助手"]
        Desktop["桌面应用"]
        IDE["IDE 集成"]
    end

    subgraph orchestration[智能体编排]
        Session["会话管理<br/>消息交互 / 流式反馈 / 历史恢复"]
        Agent["主智能体<br/>任务理解 / 推理 / 执行调度"]
        Context["上下文管理<br/>项目指令 / 历史压缩 / 长期记忆"]
        Subagents["子智能体<br/>任务委派 / 独立执行"]
        Extensions["能力扩展<br/>Agents / Skills / Slash Commands / Hooks"]
    end

    subgraph execution[工具执行]
        LocalTools["本地操作<br/>文件读写 / 代码搜索 / 命令执行 / 工作区隔离"]
        RemoteTools["外部能力<br/>网络检索 / 网页读取 / MCP 工具"]
    end

    subgraph resources[资源与数据]
        Models["模型服务"]
        Workspace["本地工作区"]
        Services["网络与 MCP 服务"]
        Storage["本地数据<br/>会话历史 / 记忆 / 用量与执行记录"]
    end

    CLI <--> Session
    Desktop <--> Session
    IDE <-->|ACP| Session
    Session <--> Agent
    Session --> Storage
    Agent --> Context
    Context <--> Storage
    Extensions --> Agent
    Agent <--> Models
    Agent <-->|任务与结果| Subagents
    Agent --> LocalTools
    Agent --> RemoteTools
    Subagents --> LocalTools
    Subagents --> RemoteTools
    LocalTools --> Workspace
    RemoteTools --> Services
    Desktop -->|终端交互| Workspace
```

## Rust toolchain

本项目使用 **nightly** 工具链（Rust 2024 edition）。

## 安装

目前仅支持源码安装，还未发布到 crates.io 上。

### Aries CLI

> just install-cli

或:

> cargo install --path apps/aries-cli --force --locked

### Aries Desktop

仅支持 macOS。

源码安装:

> cargo install --path apps/aries-desktop --force --locked

DMG 安装:

> just desktop-dmg
>
> open target/bundle/Aries-Desktop-*.dmg

## 使用

### 首次使用:

执行: `aries setup`

后续增加或者删除模型, 使用 `aries model add`, `aries model rm` 命令。

切换模型使用: `aries model default`。

查看当前所有模型: `aries model list`.

## 在支持 Agent Client Protocol 的 IDE 中使用

> 详见 [docs/acp.md](docs/acp.md)

## 支持访问网络

> 详见 [docs/web-tool.md](docs/web-tool.md)

## 评测

评测相关脚本位于 [scripts](scripts/) 目录下，目前包含 SWE-bench 评测等工具。
