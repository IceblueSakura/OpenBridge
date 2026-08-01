//! 请求分析与 Route 规划的包入口。
//!
//! 子模块分别拥有稳定错误、计划数据类型、请求事实分析和 registry 路由规划；本文件
//! 仅声明模块并保持既有公共 API 路径。

mod analysis;
mod error;
mod planning;
mod types;

pub use analysis::analyze_request;
pub use error::RequestPlanningError;
pub use planning::plan_request;
pub use types::{RequestRequirements, RouteCandidate, RoutePlan};
