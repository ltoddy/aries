1. 不要写注释.
2. 使用 `use` 或 `pub use` 引入项目自身模块时，路径必须以 `crate::` 或 `self::` 开头。
3. 不要使用 pub(crate)、pub(super)、pub(in ...) 等受限可见性；需要公开的项目代码统一使用 pub。
4. 不要主动添加单元测试。
5. Cargo.toml 中的第三方依赖库, 如果 apps 目录与 crates 目录下的代码都是用，那么设置为 workspace, 否则各自用各自的.
6. 只有在程序初始化阶段才可以使用 `.expect()` 方法, 其他的在 `apps/` 中使用 `anyhow/`, 在 `crates/` 中使用 `thiserror`.