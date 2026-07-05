# Aries SWE-bench 评测脚本

使用 [Multi-SWE-bench](https://github.com/multi-swe-bench/multi-swe-bench) 数据集评测 aries agent 的代码修复能力。

## 前置要求

- Python 3.13+
- [uv](https://github.com/astral-sh/uv)
- [just](https://github.com/casey/just)
- Docker（用于运行评测 harness）
- `aries` CLI 已安装并可用

## 安装依赖

```bash
uv sync
```

## 使用

### 下载数据集

```bash
uv run aries_swe_bench.py download
```

### 查看所有 instance ID

```bash
uv run aries_swe_bench.py list-ids
```

### 运行评测生成 patch

```bash
# 运行前 5 个任务
uv run aries_swe_bench.py run --limit 5

# 运行指定 instance
uv run aries_swe_bench.py run --instance-ids cli__cli-10388
```

运行完成后会生成 `predictions.jsonl` 文件。

### 执行评分（需要 Docker）

```bash
just evaluate
```

## 常用命令

| 命令 | 说明 |
|------|------|
| `just lint` | 代码检查 |
| `just fmt` | 代码格式化 |
| `just evaluate` | 运行评测 harness 对 predictions.jsonl 评分 |
