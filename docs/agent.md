aries 的智能体基于 `rig` 框架实现。

## 模式 Mode

内置 4 种模式，通过不同 preamble 与工具集区分职责：

| 模式      | 名称      | 说明                                                           |
|-----------|-----------|----------------------------------------------------------------|
| `build`   | Builder   | 默认主智能体，直接使用工具执行任务，并在需要时委托子智能体     |
| `plan`    | Planner   | 计划模式，不允许使用编辑工具                                   |
| `general` | Assistant | 通用智能体，用于研究复杂问题与多步任务，可并行执行多个工作单元 |
| `explore` | Explorer  | 只读的快速代码库探索智能体                                     |

## 多智能体委托

`agent` 工具可启动子智能体（子进程）自主完成多步任务，支持 `build` / `plan` / `general` / `explore` 等模式。

## 自定义 agent

支持通过配置文件定义自定义 agent，详见 [extensions.md](extensions.md)。

## 模型提供商

支持 4 种 provider：

- Anthropic
- Azure
- DeepSeek
- OpenAI

可配置 API Key、base URL、max_tokens 等，详见 `aries model add`。
