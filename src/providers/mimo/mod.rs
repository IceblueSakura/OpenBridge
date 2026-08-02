//! Xiaomi MiMo Provider 的编译期实现入口。
//!
//! 当前只公开静态定义，尚未注册 Model、Target、Route 或 Public Model。

mod definition;

pub(crate) use definition::ADAPTER;
pub use definition::CONTRACT;
