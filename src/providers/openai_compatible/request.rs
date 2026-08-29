//! Request-body preparation for OpenAI-compatible operations and probes.

use bytes::Bytes;
use http::{Method, Uri};

use crate::{
    core::{ApiProtocol, ApiRequest, EmbeddingRequest, ImagesRequest, OperationKind},
    provider::{AdapterError, PreparedUpstreamRequest},
    registry::{ReasoningLevel, ReasoningLevelMapping, UpstreamApi},
};

use super::{OpenAiCompatibleAdapter, embeddings};

impl OpenAiCompatibleAdapter {
    /// Builds the fixed model-list request used by the administrative probe.
    pub(crate) fn prepare_model_list_request(
        self,
    ) -> Result<PreparedUpstreamRequest, AdapterError> {
        // Bind the static OpenAI-compatible path without accepting a URL or query override.
        let relative_uri = Uri::from_static(self.model_list_path);
        Ok(PreparedUpstreamRequest::new(
            Method::GET,
            relative_uri,
            Bytes::new(),
        ))
    }

    /// Extracts model identifiers through the selected static response-envelope profile.
    pub(crate) fn model_list_ids(self, response: &serde_json::Value) -> Option<Vec<String>> {
        (self.model_list_parser)(response)
    }

    /// Returns the relative path attached to one declared Generation operation.
    pub(crate) const fn generation_path(self, protocol: ApiProtocol) -> Option<&'static str> {
        match protocol {
            ApiProtocol::ChatCompletions => self.chat_path,
            ApiProtocol::Responses => self.responses_path,
        }
    }

    /// Returns the relative path attached to the declared Embeddings operation.
    pub(crate) const fn embeddings_path(self) -> Option<&'static str> {
        self.embeddings_path
    }

    /// Replaces target-specific wire values and binds the selected Upstream API endpoint.
    pub(crate) fn prepare_routed_request(
        self,
        protocol: ApiProtocol,
        path: &'static str,
        request: &ApiRequest,
        upstream_api: &UpstreamApi,
    ) -> Result<PreparedUpstreamRequest, AdapterError> {
        // Reject any mismatch before parsing a body or selecting Provider wire behavior.
        if request.protocol() != protocol || upstream_api.operation() != protocol.operation() {
            return Err(AdapterError::UnsupportedProtocol);
        }
        let relative_uri = Uri::from_static(path);

        // Parse and replace the upstream model field controlled only by the selected API.
        let mut document: serde_json::Value =
            serde_json::from_slice(request.body()).map_err(|_| AdapterError::InvalidRequestBody)?;
        document
            .as_object_mut()
            .ok_or(AdapterError::InvalidRequestBody)?
            .insert(
                "model".to_owned(),
                serde_json::Value::String(upstream_api.upstream_model().to_owned()),
            );

        // Apply only the selected operation's trusted Provider wire transformation.
        (self.request_body_hook)(
            protocol,
            document
                .as_object_mut()
                .ok_or(AdapterError::InvalidRequestBody)?,
        )?;
        discard_ignored_generation_parameters(
            document
                .as_object_mut()
                .ok_or(AdapterError::InvalidRequestBody)?,
            upstream_api,
        );
        let reasoning_level_mapping = apply_reasoning_level_mapping(
            protocol,
            document
                .as_object_mut()
                .ok_or(AdapterError::InvalidRequestBody)?,
            upstream_api,
        );

        // Re-serialize once after all trusted Provider wire transformations.
        let body = serde_json::to_vec(&document)
            .map(Bytes::from)
            .map_err(|_| AdapterError::InvalidRequestBody)?;
        Ok(
            PreparedUpstreamRequest::new(Method::POST, relative_uri, body)
                .with_reasoning_level_mapping(reasoning_level_mapping),
        )
    }

    /// Binds one fixed administrative Generation probe to a declared Provider path and model.
    ///
    /// Unlike routed preparation, this deliberately applies no registered model's ignored-parameter
    /// or reasoning mapping rules. The caller may probe an unregistered model, while the Provider
    /// path and bounded body transformation remain compile-time trusted.
    pub(crate) fn prepare_probe_request(
        self,
        protocol: ApiProtocol,
        path: &'static str,
        request: &ApiRequest,
        upstream_model: &str,
        streaming: bool,
    ) -> Result<PreparedUpstreamRequest, AdapterError> {
        // Reject protocol/delivery disagreement before selecting Provider wire behavior.
        if request.protocol() != protocol {
            return Err(AdapterError::UnsupportedProtocol);
        }
        let mut document: serde_json::Value =
            serde_json::from_slice(request.body()).map_err(|_| AdapterError::InvalidRequestBody)?;
        let object = document
            .as_object_mut()
            .ok_or(AdapterError::InvalidRequestBody)?;
        if object.get("stream").and_then(serde_json::Value::as_bool) != Some(streaming) {
            return Err(AdapterError::InvalidRequestBody);
        }
        let output_limit_field = match protocol {
            ApiProtocol::ChatCompletions => "max_completion_tokens",
            ApiProtocol::Responses => "max_output_tokens",
        };
        let expected_output_limit = object
            .get(output_limit_field)
            .and_then(serde_json::Value::as_u64);

        // Override only the model in the built-in synthetic request, then apply Provider-wide wire rules.
        object.insert(
            "model".to_owned(),
            serde_json::Value::String(upstream_model.to_owned()),
        );
        (self.request_body_hook)(protocol, object)?;
        if expected_output_limit.is_some()
            && object
                .get(output_limit_field)
                .and_then(serde_json::Value::as_u64)
                != expected_output_limit
        {
            return Err(AdapterError::InvalidRequestBody);
        }

        // Bind only the static operation path and the caller-selected response lifecycle.
        let body = serde_json::to_vec(&document)
            .map(Bytes::from)
            .map_err(|_| AdapterError::InvalidRequestBody)?;
        Ok(
            PreparedUpstreamRequest::new(Method::POST, Uri::from_static(path), body)
                .with_streaming_response(streaming),
        )
    }

    /// Replaces the Public Model and binds the fixed Native Embeddings endpoint.
    pub(crate) fn prepare_embedding_routed_request(
        self,
        path: &'static str,
        request: &EmbeddingRequest,
        upstream_api: &UpstreamApi,
    ) -> Result<PreparedUpstreamRequest, AdapterError> {
        // Require an Embeddings API before parsing a body or binding the selected operation path.
        if upstream_api.operation() != OperationKind::EmbeddingsCreate {
            return Err(AdapterError::UnsupportedProtocol);
        }

        // Parse the preflighted object and replace only the registry-owned model field.
        let mut document: serde_json::Value =
            serde_json::from_slice(request.body()).map_err(|_| AdapterError::InvalidRequestBody)?;
        document
            .as_object_mut()
            .ok_or(AdapterError::InvalidRequestBody)?
            .insert(
                "model".to_owned(),
                serde_json::Value::String(upstream_api.upstream_model().to_owned()),
            );
        embeddings::prepare_request_body(
            document
                .as_object_mut()
                .ok_or(AdapterError::InvalidRequestBody)?,
            upstream_api.embedding_encoding_policy(),
        )?;

        // Re-serialize once after the Provider-scoped encoding translation.
        let body = serde_json::to_vec(&document)
            .map(Bytes::from)
            .map_err(|_| AdapterError::InvalidRequestBody)?;
        Ok(PreparedUpstreamRequest::new(
            Method::POST,
            Uri::from_static(path),
            body,
        ))
    }

    /// Replaces the Public Model, applies the Provider wire conversion, and binds the Images endpoint.
    pub(crate) fn prepare_images_routed_request(
        self,
        path: &'static str,
        request: &ImagesRequest,
        upstream_api: &UpstreamApi,
    ) -> Result<PreparedUpstreamRequest, AdapterError> {
        // Require an Images Generations API before parsing a body or binding the operation path.
        if upstream_api.operation() != OperationKind::ImagesGenerations {
            return Err(AdapterError::UnsupportedProtocol);
        }

        // Parse the preflighted object and replace only the registry-owned model field.
        let mut document: serde_json::Value =
            serde_json::from_slice(request.body()).map_err(|_| AdapterError::InvalidRequestBody)?;
        document
            .as_object_mut()
            .ok_or(AdapterError::InvalidRequestBody)?
            .insert(
                "model".to_owned(),
                serde_json::Value::String(upstream_api.upstream_model().to_owned()),
            );

        // Apply only the selected operation's trusted Provider wire transformation.
        (self.images_request_body_hook)(
            document
                .as_object_mut()
                .ok_or(AdapterError::InvalidRequestBody)?,
        )?;

        // Re-serialize once without converting prompt, n, size, or user fields.
        let body = serde_json::to_vec(&document)
            .map(Bytes::from)
            .map_err(|_| AdapterError::InvalidRequestBody)?;
        Ok(PreparedUpstreamRequest::new(
            Method::POST,
            Uri::from_static(path),
            body,
        ))
    }
}

/// Extracts model identifiers from the common OpenAI-compatible `data[].id` envelope.
pub(super) fn parse_openai_model_list_ids(response: &serde_json::Value) -> Option<Vec<String>> {
    Some(
        response
            .get("data")?
            .as_array()?
            .iter()
            .filter_map(|entry| entry.get("id").and_then(serde_json::Value::as_str))
            .map(str::to_owned)
            .collect(),
    )
}

/// Removes only the closed ordinary-generation fields configured for the selected Upstream API.
fn discard_ignored_generation_parameters(
    document: &mut serde_json::Map<String, serde_json::Value>,
    upstream_api: &UpstreamApi,
) {
    for parameter in upstream_api.ignored_generation_parameters() {
        document.remove(parameter.as_wire_name());
    }
}

/// Keeps ordinary OpenAI-compatible request bodies unchanged after model replacement.
pub(super) fn preserve_request_body(
    _protocol: ApiProtocol,
    _document: &mut serde_json::Map<String, serde_json::Value>,
) -> Result<(), AdapterError> {
    Ok(())
}

/// Keeps already OpenAI-compatible Images request bodies unchanged after model replacement.
pub(super) fn convert_images_to_openai_compatible_shape(
    _document: &mut serde_json::Map<String, serde_json::Value>,
) -> Result<(), AdapterError> {
    Ok(())
}

/// Removes a standard Chat reasoning level and returns its equivalent thinking-switch state.
pub(crate) fn take_chat_reasoning_switch(
    protocol: ApiProtocol,
    document: &mut serde_json::Map<String, serde_json::Value>,
) -> Result<Option<bool>, AdapterError> {
    // Leave non-Chat requests and requests without an explicit standard level unchanged.
    if protocol != ApiProtocol::ChatCompletions {
        return Ok(None);
    }
    let Some(level) = document.remove("reasoning_effort") else {
        return Ok(None);
    };

    // Treat `none` as disabled and every other recognized model level as enabled.
    let level = level
        .as_str()
        .and_then(ReasoningLevel::from_wire)
        .ok_or(AdapterError::InvalidRequestBody)?;
    Ok(Some(level != ReasoningLevel::None))
}

/// Applies one canonical reasoning level mapping at the protocol-defined wire location.
fn apply_reasoning_level_mapping(
    protocol: ApiProtocol,
    document: &mut serde_json::Map<String, serde_json::Value>,
    upstream_api: &UpstreamApi,
) -> Option<ReasoningLevelMapping> {
    // Locate only the standard reasoning field for the prepared request protocol.
    let value = match protocol {
        ApiProtocol::ChatCompletions => document.get_mut("reasoning_effort"),
        ApiProtocol::Responses => document
            .get_mut("reasoning")
            .and_then(serde_json::Value::as_object_mut)
            .and_then(|reasoning| reasoning.get_mut("effort")),
    }?;

    // Resolve and write only an explicitly configured canonical-to-Provider mapping.
    let downstream = value.as_str().and_then(ReasoningLevel::from_wire)?;
    let upstream = upstream_api.reasoning_level_mapping(downstream)?.to_owned();
    *value = serde_json::Value::String(upstream.clone());
    Some(ReasoningLevelMapping {
        downstream,
        upstream,
    })
}
