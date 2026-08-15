# 持久化与统计

## 持久化

基于 SQLite（toasty）持久化三类数据：

- `Session`
- `ToolCall`
- `TokenAudit`

## 统计

- `aries stats tool`：统计近 30 天工具调用次数。
- `aries stats bash`：统计近 30 天 bash 命令使用频率（基于 tree-sitter 解析）。

## 清理

`aries gc` 清理 30 天前的过期记录。
