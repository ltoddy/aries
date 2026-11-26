1. 不要在代码中添加注释。
2. 使用 `use` 或 `pub use` 引入项目自身模块时，路径必须以 `crate::` 或 `self::` 开头。
3. 不要使用 `pub(crate)`、`pub(super)`、`pub(in ...)` 等受限可见性；需要公开的项目代码统一使用 `pub`。
4. 不要主动添加单元测试。
5. 所有结构体和枚举都必须通过构造函数创建。
6. `apps/` 和 `crates/` 共同使用的第三方依赖，统一在 `[workspace.dependencies]` 中声明；其他依赖在各自的 `Cargo.toml` 中声明。
7. 错误处理使用 `thiserror`，不要使用 `anyhow`。需要转换错误类型时，先通过 `map_err` 显式转换，再使用 `?`，不要依赖 `?` 隐式转换。
