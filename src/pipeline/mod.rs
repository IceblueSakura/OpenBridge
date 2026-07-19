use bytes::Bytes;
use serde_json::Value;
use thiserror::Error;

use crate::{
    config::RegistrySnapshot,
    core::{CapabilitySet, Protocol, ValidatedRequest},
};

#[derive(Debug, Error)]
pub enum RouteError {
    #[error("request body must be a JSON object")]
    InvalidJson,
    #[error("request body must contain a non-empty model")]
    MissingModel,
    #[error("requested model is not configured")]
    UnknownModel,
    #[error("configured model has no deployment candidate")]
    NoDeployment,
    #[error("selected deployment does not support this protocol")]
    UnsupportedProtocol,
    #[error("selected deployment does not support streaming")]
    StreamingUnsupported,
    #[error("selected deployment does not support requested capabilities")]
    UnsupportedCapabilities,
}

#[derive(Debug)]
pub struct PreparedNativeRequest {
    deployment_id: String,
    request: ValidatedRequest,
}

impl PreparedNativeRequest {
    pub fn deployment_id(&self) -> &str {
        &self.deployment_id
    }

    pub fn request(&self) -> &ValidatedRequest {
        &self.request
    }

    pub fn into_request(self) -> ValidatedRequest {
        self.request
    }
}

pub fn prepare_native_request(
    snapshot: &RegistrySnapshot,
    protocol: Protocol,
    body: Bytes,
) -> Result<PreparedNativeRequest, RouteError> {
    let mut document: Value = serde_json::from_slice(&body).map_err(|_| RouteError::InvalidJson)?;
    let object = document.as_object_mut().ok_or(RouteError::InvalidJson)?;
    let public_model = object
        .get("model")
        .and_then(Value::as_str)
        .filter(|model| !model.is_empty())
        .ok_or(RouteError::MissingModel)?;
    let deployment_id = snapshot
        .alias(public_model)
        .ok_or(RouteError::UnknownModel)?
        .candidates()
        .first()
        .ok_or(RouteError::NoDeployment)?
        .clone();
    let deployment = snapshot
        .deployment(&deployment_id)
        .ok_or(RouteError::NoDeployment)?;
    let capabilities = deployment.capabilities();
    let protocol_supported = match protocol {
        Protocol::ChatCompletions => capabilities.chat,
        Protocol::Responses => capabilities.responses,
    };
    if !protocol_supported {
        return Err(RouteError::UnsupportedProtocol);
    }
    if object.get("stream").and_then(Value::as_bool) == Some(true) && !capabilities.streaming {
        return Err(RouteError::StreamingUnsupported);
    }
    let requested_features = CapabilitySet {
        chat: false,
        responses: false,
        streaming: false,
        function_tools: object
            .get("tools")
            .and_then(Value::as_array)
            .is_some_and(|tools| !tools.is_empty()),
        structured_output: requests_structured_output(object),
        previous_response_id: object
            .get("previous_response_id")
            .is_some_and(|value| !value.is_null()),
        background: object.get("background").and_then(Value::as_bool) == Some(true),
        response_store: object.get("store").and_then(Value::as_bool) == Some(true),
    };
    if !requested_features.is_subset_of(*capabilities) {
        return Err(RouteError::UnsupportedCapabilities);
    }
    object.insert(
        "model".to_owned(),
        Value::String(deployment.upstream_model().to_owned()),
    );
    let body = serde_json::to_vec(&document)
        .map(Bytes::from)
        .map_err(|_| RouteError::InvalidJson)?;

    Ok(PreparedNativeRequest {
        deployment_id,
        request: ValidatedRequest::new(protocol, body),
    })
}

fn requests_structured_output(object: &serde_json::Map<String, Value>) -> bool {
    object
        .get("response_format")
        .is_some_and(is_non_text_format)
        || object
            .get("text")
            .and_then(Value::as_object)
            .and_then(|text| text.get("format"))
            .is_some_and(is_non_text_format)
}

fn is_non_text_format(format: &Value) -> bool {
    format
        .as_object()
        .and_then(|format| format.get("type"))
        .and_then(Value::as_str)
        .is_some_and(|format_type| format_type != "text")
}
