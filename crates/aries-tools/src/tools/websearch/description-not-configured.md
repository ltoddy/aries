WebSearch 工具当前不可用：环境变量 `TAVILY_API_KEY` 未配置。

要启用网络搜索功能，需要先设置该环境变量（Tavily API key）后重新启动程序。例如在 shell 中：

```bash
export TAVILY_API_KEY="your_tavily_api_key"
```

也可以把该变量写入 shell 配置文件（如 `~/.zshrc`、`~/.bashrc`），使其永久生效。

在配置完成之前，请勿尝试使用本工具，并告知用户需要设置 `TAVILY_API_KEY` 环境变量才能使用网络搜索。
