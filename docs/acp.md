# 在支持 Agent Client Protocol 的 IDE 中使用

### JetBrains IDE

![jetbrains acp](assets/jetbrains-acp.png)

添加配置文件 `~/.jetbrains/acp.json`:

```json
{
  "agent_servers": {
    "aries": {
      "command": "aries",
      "args": [
        "acp"
      ]
    }
  }
}
```

### Zed

![zed acp](assets/zed-acp.png)

在 `~/.config/zed/settings.json` 添加配置:

```json
{
  "agent_servers": {
    "aries": {
      "type": "custom",
      "command": "aries",
      "args": [
        "acp"
      ]
    }
  }
}
```
