## TODO

- [x] 完善 hooks
- [x] 日志. chat-history 与 context 分离
- [x] 支持长期记忆
- [x] 支持使用 SWE-bench 进行评测
- [x] island
- [x] 支持 mcp
- [x] custom agent 支持
- [x] 支持 plugin
- [x] 优化 Tool 的实现
  - [x] Write/Edit/MultiEdit 写入后通知 LSP 重建索引 (ToolContext + did_change/did_save)
  - [x] Write tool 输出结构化 diff (WriteKind / Hunk / additions / deletions)
  - [x] UpdatePlan 支持 active_form、校验空字段、全部完成自动清空
  - [x] Skill tool 增强：allowed-tools、强制调用规则
  - [x] Read tool 流式读取、limit 参数、行号输出
  - [x] 抽取 `diff` 到共享 `tools/diff.rs`（Write/Edit/MultiEdit 复用）
  - [x] ToolContext 新增 read_state + guard_write（写前校验文件已完整读取且未被外部修改）
- [x] 支持 Bare mode 快速启动 Agent
- [ ] 从错误中恢复
  - 输出被截断 (Anthropic 的接口可以设置 max_tokens)
  - 上下文超长
  - [x] 429 status code 限流等
- [x] Read 工具读取文件时，自动注入所在目录的 AGENTS.md 作为 system-reminder，去重避免重复发送
- [ ] acp 协议 v2 支持 (https://agentclientprotocol.com/protocol/v2/overview 等什么时候不再是 draft 了, 立马跟进实现)
- [x] 优化长期记忆系统
  - [x] 历史与上下文持久化异步化：extend 改为 channel + 后台 task 写文件，不再阻塞 prompt 流程
  - [x] 记忆提取后台化：prompt 后 spawn 后台 task 提取记忆（复用 sift）
  - [x] 长期记忆重构：extractor 拆分为 MemoryRetriever（召回相关记忆）与 MemoryAgent（后台提取记忆写盘），selector 命名统一收敛为 retriever（Memory / as_retriever_line / retrieve / RetrievedMemories）
- [x] 上下文压缩：prompt 前按预估 token 预判压缩，prompt 后按实际 token 复核压缩（pre_compact / post_compact），micro_compact 保留更多最近消息
- [x] 代码优化：prompt 流程提取 helper（fire_stop / append_messages / session_hook 等），agent.prompt 改 &self，callback 改 Fn + Clone
- [x] 优化 mcp 启动 (超时，连接速度，连接异常)
- [ ] 支持 worktree
- [ ] 支持 Workflow
- [ ] agent (参考 https://github.com/vercel/eve 对 agent 的目录结构设计来支持)
- [ ] token 使用优化
- [ ] 支持设置模型 effort
- [ ] 支持 design.md
- [ ] 支持 cron
- [x] 可视化数据库保存的数据（`aries stats tool` / `aries stats bash`：工具调用次数、bash 命令使用频率）
- [ ] command
- [ ] 通过 rsync 实现本机的 sandbox
- [ ] 支持 Browser Use
- [ ] 支持 Computer Use
- [ ] 支持自我进化 (想要达到的目标, 从单体进化逐步走向群体进化,单体进化: 一个 Agent 变聪明了, 另一个 Agent 不知道, 希望逐步走向进化成果可以像基因一样跨个体共享, 群体遗传!)
  - hermes agent 实现自进化有两种方式:
    1. 动态 Skill 生成, 每次完成一轮对话,后台会启动一个审查 Agent (这段对话有什么值得记忆的经验? 这个任务模式值值得抽象成 skill ? 整个执行过程有什么可改进的,如果值得就把经验结构化成一个 skill 包)，如果连续跑了十论都没有生成新的 skill 系统会提示: 你是不是改把最近学到的经验整理一下. 同时在未来遇到新的边界 case 以生成的 skill 也会再次更新.
    2. RL 训练, 使用更强大模型，批量生成高质量的 Agent 执行轨迹, 清洗压缩成训练数据，然后使用 GRPO 算法训练小模型.
  - GEP 架构 参考项目: https://github.com/EvoMap/evolver
- [ ] 遥测与 tracing (记录每个工具调用预估耗费的 token, 用于统计，针对工具做优化)
- [ ] 优化 island，展示 token 消耗与类似 Github 的活跃图
- [ ] for harness engineering and autonomous agent
- [ ] 长程任务
- [ ] 对环境进行感知
- [ ] 是否需要实现投机解码 ?
- [ ] 实现 MoA (Mixture of Agent)
- [ ] 支持 worktree (我在想如果通过 rsync 实现 sandbox 之后是不是就不需要支持 worktree 了？)
- [ ] 支持 handoff
- [ ] 长期记忆改成 全局记忆以及项目记忆
- [ ] 支持 Agent Teams
- [ ] 支持 checkpoint
- [ ] 支持 remote control
- [ ] Bun 被 Rust 重写这个事情，超大规模多 Agent 协同也是一个挑战场景。

## 可观测性

Agent 系统有独特的复杂性:
- 不确定性: 相同的输入产生不同的输出.
- 多步骤: 用户一个请求可能触发非常多次的 LLM 调用
- 成本不透明: Token 消耗不固定.

1. Prompt 追踪：记录和追踪所有 Prompt（包括动态生成的）
2. Tool Call 追踪：监控 Agent 调用的所有外部工具
   - 工具调用成功率
   - 工具调用延迟分布，识别慢工具
   - 哪些工具容易出错.
   - 哪些工具被调用的最多，以及是否有优化空间.
3. Trace 链路追踪：完整的调用链路，从输入到输出
   - 问题定位.
   - 性能优化. 例如查找哪个 span 慢.
   - 成本分析.
   - 质量评估: 对比成功的 trace 与失败的 trace 找出差异
4. Token 追踪：Token 消耗、成本、延迟的实时监控
   - 模型选择优化.
   - Prompt 缓存.
   - 输出长度控制.

## 讨论

- 如何量化评估 Agent 性能好坏
- 如何保证 Agent 行为一致性.
- 工具调用失败，如何感知，如何处理。
- 如何感知注意力丢失。
- Agent 可观测性如何建设
