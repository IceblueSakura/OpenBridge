//! Definition, input validation, and side-effect-free execution for the `hello` test tool.
//!
//! This tool may use only its bounded request arguments. It must not read private configuration,
//! inspect the registry, access files, perform network egress, or call Providers. `rmcp` macros
//! derive the JSON Schema from the parameter struct and reject unknown fields at dispatch time.

use rmcp::{ServerHandler, handler::server::wrapper::Parameters, model::ServerCapabilities};
use schemars::JsonSchema;

/// Bounded request arguments for the hello tool.
#[derive(Debug, serde::Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct HelloParams {
    /// Name to greet.
    name: String,
}

/// The local hello tool server. Owns no state and performs no external side effects.
#[derive(Debug, Clone, Default)]
pub(crate) struct HelloServer;

#[rmcp::tool_router]
impl HelloServer {
    /// Returns a greeting for the provided name.
    #[rmcp::tool(description = "Returns a greeting for the provided name.")]
    fn hello(&self, Parameters(params): Parameters<HelloParams>) -> String {
        format!("Hi, {}!", params.name)
    }
}

#[rmcp::tool_handler]
impl ServerHandler for HelloServer {
    fn get_info(&self) -> rmcp::model::ServerInfo {
        rmcp::model::ServerInfo::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_tool_list_changed()
                .build(),
        )
        .with_server_info(rmcp::model::Implementation::new(
            "openbridge",
            env!("CARGO_PKG_VERSION"),
        ))
    }
}
