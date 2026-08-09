# 从网络获取

目前有两个内置的 Tool 可以用来获取网络内容: WebFetch 与 WebSearch

## WebFetch Tool 说明

WebFetch Tool 使用 Firecrawl v2 抓取网页，并直接返回 Firecrawl 生成的 Markdown。

默认连接 Firecrawl 云服务。可配置 API Key：

```bash
export FIRECRAWL_API_KEY="fc-your_api_key"
```

使用自托管 Firecrawl 时配置服务地址；API Key 可选：

```bash
export FIRECRAWL_API_URL="http://localhost:3002"
export FIRECRAWL_API_KEY="optional_api_key"
```

## WebSearch Tool 说明

在 crates/aries-tools/src/tools/websearch/ 目录下实现了 WebSearch Tool 通过调用 tavily 的接口来实现

调用 tavily 接口需要配置 api_key , 打开 https://www.tavily.com/ 登陆, 每个人默认都有一定的免费额度.

然后配置环境变量 TAVILY_API_KEY 之后， WebSearch Tool 就可以使用了.

例如在你的 .zshrc 文件或者 .bashrc 文件中添加:

```bash
export TAVILY_API_KEY="your_tavily_api_key"
```
