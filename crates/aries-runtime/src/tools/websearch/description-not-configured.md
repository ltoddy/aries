通过 Tavily 搜索公开网络并返回标题、URL 和摘要；当前不可用，因为 `TAVILY_API_KEY` 未配置或为空，调用会直接报配置错误。

需要用户在运行 Aries 的环境中设置 `TAVILY_API_KEY`，并重新启动程序后再搜索。配置完成前不要反复调用；若已有明确的网页 URL，可在 Firecrawl 可用时使用 `WebFetch` 读取，查询外部编程资料可尝试 `CodeSearch`。
