1. 不要写注释.
2. 使用 `use` 或 `pub use` 引入项目自身模块时，路径必须以 `crate::` 或 `self::` 开头。
3. 不要使用 pub(crate)、pub(super)、pub(in ...) 等受限可见性；需要公开的项目代码统一使用 pub。
