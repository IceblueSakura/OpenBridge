//! OpenBridge 请求协议与能力模型。
//!
//! 本模块只定义 provider-independent 的协议和能力值对象；它不负责解析 HTTP、选择
//! route 或改写请求正文，避免协议事实和 provider 实现耦合。

mod capability;
mod request;

pub use capability::{ApiCapabilities, EndpointCapabilities, ResponsesCapabilities};
pub use request::{ApiProtocol, ApiRequest};
