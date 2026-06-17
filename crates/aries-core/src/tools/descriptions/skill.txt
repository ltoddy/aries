加载并注入一个由系统提示词中 `available_skills` 列出的专用 skill。

- `name` 必须与 `available_skills` 中的某个 skill 名称完全一致；否则会以「未在 available_skills 中」的错误返回。
- 调用后返回 `<skill_content>` 块，包含：
  - skill 的正文（指令）；
  - skill 所在目录的 base URI；
  - 通过 `walk_dir` 列出的目录下文件清单（仅文件，目录被忽略），每项形如 `<file>...</file>`。
- skill 内部以相对路径引用的脚本/资源（如 `scripts/`、`reference/`），都相对于 base 目录解析。
