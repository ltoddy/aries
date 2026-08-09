# WebSearch Tool 说明

在 crates/aries-tools/src/tools/websearch/ 目录下实现了 WebSearch Tool 通过调用 tavily 的接口来实现

调用 tavily 接口需要配置 api_key , 打开 https://www.tavily.com/ 登陆, 每个人默认都有一定的免费额度.

然后配置环境变量 TAVILY_API_KEY 之后， WebSearch Tool 就可以使用了.

例如在你的 .zshrc 文件或者 .bashrc 文件中添加:

```bash
export TAVILY_API_KEY="your_tavily_api_key"
```
