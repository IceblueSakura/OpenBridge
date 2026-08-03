//! 下游 Public Model 的固定能力契约与安全序列化模型。
//!
//! 本模块只编译客户端可依赖的静态模型事实和 Chat/Responses 接口能力。能力取所有可执行
//! Route 的保守交集，但响应中不保留或暴露 Provider、Target、Route、上游模型及凭据边界。

use std::collections::BTreeSet;

use serde::Serialize;

use crate::core::{ApiProtocol, ReasoningOutput};

use super::{
    InputModality, ModelContextLength, ModelLifecycle, ModelLifecycleStatus, OutputModality,
    PublicModelConfig, ReasoningLevel, ReasoningSupport, Route, RouteMode, UpstreamApi,
    UpstreamApiCapabilities,
};

/// 扩展模型信息对象的稳定 schema 版本。
pub const MODEL_INFO_SCHEMA_VERSION: &str = "1";

/// 能力证据状态；`unknown` 不能在请求预检中当作支持。
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SupportState {
    /// 所有可执行 Route 都明确支持该能力。
    Supported,
    /// 至少一条可执行 Route 明确不支持该能力。
    Unsupported,
    /// 当前静态事实不足以安全判断。
    Unknown,
}

impl SupportState {
    /// 将明确的布尔能力转换为公开状态。
    const fn from_bool(supported: bool) -> Self {
        if supported {
            Self::Supported
        } else {
            Self::Unsupported
        }
    }

    /// 判断能力是否能被请求路径当作已保证支持。
    pub(crate) const fn is_supported(self) -> bool {
        matches!(self, Self::Supported)
    }

    /// 计算多个完整 Route profile 的保守交集。
    fn intersection(values: impl Iterator<Item = Self>) -> Self {
        let mut saw_value = false;
        let mut saw_unknown = false;
        for value in values {
            saw_value = true;
            match value {
                Self::Unsupported => return Self::Unsupported,
                Self::Unknown => saw_unknown = true,
                Self::Supported => {}
            }
        }
        if !saw_value || saw_unknown {
            Self::Unknown
        } else {
            Self::Supported
        }
    }
}

impl From<ReasoningSupport> for SupportState {
    fn from(value: ReasoningSupport) -> Self {
        match value {
            ReasoningSupport::Supported => Self::Supported,
            ReasoningSupport::Unsupported => Self::Unsupported,
            ReasoningSupport::Unknown => Self::Unknown,
        }
    }
}

/// Public Model 可承担的任务类别。
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelTask {
    /// 对话生成任务。
    Chat,
    /// 通用文本生成任务。
    TextGeneration,
}

/// Public Model 的上下文窗口限制。
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct ContextWindow {
    max_context_tokens: Option<u32>,
    max_input_tokens: Option<u32>,
    max_output_tokens: Option<u32>,
}

impl ContextWindow {
    /// 从 registry 内部的三段限制构造公开对象。
    const fn from_model(value: ModelContextLength) -> Self {
        Self {
            max_context_tokens: value.context_tokens(),
            max_input_tokens: value.input_tokens(),
            max_output_tokens: value.output_tokens(),
        }
    }

    /// 返回公开契约保证的最大输出 token 数。
    pub(crate) const fn max_output_tokens(self) -> Option<u32> {
        self.max_output_tokens
    }

    /// 对所有 Route 的已知限制取最小值；任一值未知时保持未知。
    fn intersection<'a>(values: impl Iterator<Item = &'a Self> + Clone) -> Self {
        Self {
            max_context_tokens: intersect_optional_limit(
                values.clone().map(|value| value.max_context_tokens),
            ),
            max_input_tokens: intersect_optional_limit(
                values.clone().map(|value| value.max_input_tokens),
            ),
            max_output_tokens: intersect_optional_limit(
                values.map(|value| value.max_output_tokens),
            ),
        }
    }
}

/// Public Model 已确认的输入与输出模态。
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ModelModalities {
    input: Vec<InputModality>,
    output: Vec<OutputModality>,
}

impl ModelModalities {
    /// 计算多个 Route profile 的稳定集合交集。
    fn intersection<'a>(values: impl Iterator<Item = &'a Self> + Clone) -> Self {
        Self {
            input: intersect_sets(values.clone().map(|value| value.input.as_slice())),
            output: intersect_sets(values.map(|value| value.output.as_slice())),
        }
    }
}

/// 模型本体的 reasoning 能力。
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ModelReasoningCapabilities {
    support: SupportState,
    levels: Vec<ReasoningLevel>,
}

/// 接口可观察的 reasoning 输出形态。
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReasoningOutputMode {
    /// 上游明确不返回 reasoning 输出。
    Unsupported,
    /// 返回可读的完整 reasoning 文本。
    PlainText,
    /// 只返回可读 reasoning summary。
    Summary,
    /// 返回不可读 opaque continuation。
    Opaque,
    /// 当前证据不足以判断输出形态。
    Unknown,
}

impl From<ReasoningOutput> for ReasoningOutputMode {
    fn from(value: ReasoningOutput) -> Self {
        match value {
            ReasoningOutput::Unsupported => Self::Unsupported,
            ReasoningOutput::PlainText => Self::PlainText,
            ReasoningOutput::Summary => Self::Summary,
            ReasoningOutput::Opaque => Self::Opaque,
            ReasoningOutput::Unknown => Self::Unknown,
        }
    }
}

/// 模型本体的公开能力摘要。
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ModelCapabilities {
    tasks: Vec<ModelTask>,
    context_window: ContextWindow,
    modalities: ModelModalities,
    supported_parameters: Vec<String>,
    tokenizer: Option<String>,
    knowledge_cutoff: Option<String>,
    reasoning: ModelReasoningCapabilities,
}

/// Public Model 的 function tool 能力。
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ToolCapabilities {
    support: SupportState,
    types: Vec<ToolType>,
    tool_choice_modes: Vec<ToolChoiceMode>,
    parallel_calls: SupportState,
    strict_schema: SupportState,
}

/// 下游可声明的工具种类。
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolType {
    /// JSON-schema function tool。
    Function,
}

/// function tool 的选择模式。
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolChoiceMode {
    /// 禁止模型调用工具。
    None,
    /// 由模型自动选择是否调用工具。
    Auto,
    /// 要求模型至少调用一个工具。
    Required,
    /// 指定一个命名 function。
    Named,
}

/// 结构化输出能力。
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct StructuredOutputCapabilities {
    support: SupportState,
    modes: Vec<StructuredOutputMode>,
    strict_schema: SupportState,
}

/// OpenBridge 当前建模的结构化输出模式。
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StructuredOutputMode {
    /// JSON object 输出约束。
    JsonObject,
    /// JSON schema 输出约束。
    JsonSchema,
}

/// 单个下游接口的 reasoning 能力。
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct InterfaceReasoningCapabilities {
    support: SupportState,
    levels: Vec<ReasoningLevel>,
    output: ReasoningOutputMode,
}

/// 单个下游接口的持久状态能力。
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct StateCapabilities {
    store: SupportState,
    previous_response_id: SupportState,
    background: SupportState,
}

/// 一个协议接口唯一、固定且可直接用于请求预检的能力契约。
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ModelInterfaceCapabilities {
    context_window: ContextWindow,
    modalities: ModelModalities,
    supported_parameters: Vec<String>,
    streaming: SupportState,
    system_messages: SupportState,
    tools: ToolCapabilities,
    structured_outputs: StructuredOutputCapabilities,
    reasoning: InterfaceReasoningCapabilities,
    prompt_caching: SupportState,
    state: StateCapabilities,
}

impl ModelInterfaceCapabilities {
    /// 判断接口是否保证支持 streaming。
    pub(crate) const fn supports_streaming(&self) -> bool {
        self.streaming.is_supported()
    }

    /// 判断接口是否保证支持 function tools。
    pub(crate) const fn supports_function_calling(&self) -> bool {
        self.tools.support.is_supported()
    }

    /// 判断接口是否保证支持并行 function calls。
    pub(crate) const fn supports_parallel_tool_calls(&self) -> bool {
        self.tools.parallel_calls.is_supported()
    }

    /// 判断接口是否保证支持图像输入。
    pub(crate) fn supports_image_input(&self) -> bool {
        self.modalities.input.contains(&InputModality::Image)
    }

    /// 判断接口是否保证支持结构化输出。
    pub(crate) const fn supports_structured_outputs(&self) -> bool {
        self.structured_outputs.support.is_supported()
    }

    /// 判断接口是否保证支持 `store: true`。
    pub(crate) const fn supports_store(&self) -> bool {
        self.state.store.is_supported()
    }

    /// 判断接口是否保证支持 `previous_response_id`。
    pub(crate) const fn supports_previous_response_id(&self) -> bool {
        self.state.previous_response_id.is_supported()
    }

    /// 判断接口是否保证支持后台响应。
    pub(crate) const fn supports_background(&self) -> bool {
        self.state.background.is_supported()
    }

    /// 返回接口保证的最大输出 token 数。
    pub(crate) const fn max_output_tokens(&self) -> Option<u32> {
        self.context_window.max_output_tokens()
    }

    /// 返回接口 reasoning 的证据状态。
    pub(crate) const fn reasoning_support(&self) -> SupportState {
        self.reasoning.support
    }

    /// 返回接口保证支持的 reasoning level 集合。
    pub(crate) fn reasoning_levels(&self) -> &[ReasoningLevel] {
        &self.reasoning.levels
    }
}

/// Public Model 的两个 OpenAI-compatible 接口契约。
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ModelInterfaces {
    chat_completions: Option<ModelInterfaceCapabilities>,
    responses: Option<ModelInterfaceCapabilities>,
}

impl ModelInterfaces {
    /// 按下游协议返回固定接口契约。
    pub(crate) const fn for_protocol(
        &self,
        protocol: ApiProtocol,
    ) -> Option<&ModelInterfaceCapabilities> {
        match protocol {
            ApiProtocol::ChatCompletions => self.chat_completions.as_ref(),
            ApiProtocol::Responses => self.responses.as_ref(),
        }
    }

    /// 判断至少存在一个可执行接口。
    const fn is_available(&self) -> bool {
        self.chat_completions.is_some() || self.responses.is_some()
    }
}

/// OpenAI 标准 Models resource 的严格四字段投影。
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct StandardModel {
    id: String,
    object: &'static str,
    created: u64,
    owned_by: &'static str,
}

impl StandardModel {
    /// 返回下游稳定 Public Model id。
    pub fn id(&self) -> &str {
        &self.id
    }
}

/// OpenBridge 扩展接口返回的完整 Public Model 信息。
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PublicModelInfo {
    schema_version: &'static str,
    #[serde(flatten)]
    standard: StandardModel,
    name: String,
    description: Option<String>,
    lifecycle: ModelLifecycle,
    capabilities: ModelCapabilities,
    interfaces: ModelInterfaces,
}

impl PublicModelInfo {
    /// 返回 OpenAI 标准四字段投影。
    pub fn standard(&self) -> &StandardModel {
        &self.standard
    }

    /// 按协议返回与请求预检共用的固定能力契约。
    pub(crate) const fn interface(
        &self,
        protocol: ApiProtocol,
    ) -> Option<&ModelInterfaceCapabilities> {
        self.interfaces.for_protocol(protocol)
    }
}

/// 已解析的下游 Public Model、固定信息对象和有序 Route 列表。
#[derive(Debug)]
pub struct PublicModel {
    pub(super) routes: Vec<String>,
    pub(super) info: PublicModelInfo,
}

impl PublicModel {
    /// 返回按优先级排列的 Route id；能力不会改变该顺序。
    pub fn routes(&self) -> &[String] {
        &self.routes
    }

    /// 返回扩展接口使用的完整安全模型信息。
    pub fn info(&self) -> &PublicModelInfo {
        &self.info
    }

    /// 返回 OpenAI 标准 Models resource 投影。
    pub fn standard(&self) -> &StandardModel {
        self.info.standard()
    }

    /// 按下游协议返回请求预检使用的唯一能力契约。
    pub(crate) const fn interface(
        &self,
        protocol: ApiProtocol,
    ) -> Option<&ModelInterfaceCapabilities> {
        self.info.interface(protocol)
    }

    /// 判断模型是否仍对客户端可见且至少存在一个可执行接口。
    pub(crate) fn is_available(&self) -> bool {
        self.info.lifecycle.status != ModelLifecycleStatus::Retired
            && self.info.interfaces.is_available()
    }
}

/// 编译 Public Model 时使用的单条可执行 Route 视图。
pub(super) struct PublicRouteBinding<'a> {
    pub(super) route: &'a Route,
    pub(super) upstream_api: &'a UpstreamApi,
    pub(super) target_enabled: bool,
}

/// 从完整 Route 集合编译不含部署细节的固定 Public Model。
pub(super) fn compile_public_model(
    config: PublicModelConfig,
    bindings: &[PublicRouteBinding<'_>],
) -> PublicModel {
    // 只把静态启用且 endpoint capability 已启用的 Route 纳入可执行契约。
    let profiles = bindings
        .iter()
        .filter(|binding| binding.target_enabled)
        .filter_map(RouteCapabilityProfile::from_binding)
        .collect::<Vec<_>>();

    // 分协议计算唯一保守交集，并从所有可执行 Route 汇总模型本体事实。
    let chat_completions = aggregate_interface(
        profiles
            .iter()
            .filter(|profile| profile.protocol == ApiProtocol::ChatCompletions),
    );
    let responses = aggregate_interface(
        profiles
            .iter()
            .filter(|profile| profile.protocol == ApiProtocol::Responses),
    );
    let capabilities = aggregate_model_capabilities(&profiles);

    // 固化标准投影与扩展对象；Route id 仅保留在私有执行对象中。
    let info = PublicModelInfo {
        schema_version: MODEL_INFO_SCHEMA_VERSION,
        standard: StandardModel {
            id: config.id,
            object: "model",
            created: config.created,
            owned_by: "openbridge",
        },
        name: config.display_name,
        description: config.description,
        lifecycle: config.lifecycle,
        capabilities,
        interfaces: ModelInterfaces {
            chat_completions,
            responses,
        },
    };
    PublicModel {
        routes: config.routes,
        info,
    }
}

#[derive(Clone)]
struct RouteCapabilityProfile {
    protocol: ApiProtocol,
    context_window: ContextWindow,
    modalities: ModelModalities,
    model_modalities: Option<ModelModalities>,
    model_parameters: Vec<String>,
    model_reasoning: SupportState,
    model_reasoning_levels: Vec<ReasoningLevel>,
    interface_parameters: Vec<String>,
    streaming: SupportState,
    system_messages: SupportState,
    function_calling: SupportState,
    parallel_tool_calls: SupportState,
    structured_outputs: SupportState,
    reasoning: SupportState,
    reasoning_levels: Vec<ReasoningLevel>,
    reasoning_output: ReasoningOutputMode,
    prompt_caching: SupportState,
    store: SupportState,
    previous_response_id: SupportState,
    background: SupportState,
}

impl RouteCapabilityProfile {
    /// 将 Native 或 Bridged Route 转换为面向下游协议的完整能力 profile。
    fn from_binding(binding: &PublicRouteBinding<'_>) -> Option<Self> {
        let route = binding.route;
        let upstream_api = binding.upstream_api;
        let generation = upstream_api.capabilities().generation_capabilities();
        if !generation.enabled {
            return None;
        }

        // Bridge 只公开当前转换器完整支持的公共子集。
        let bridged = route.mode() == RouteMode::Bridged;
        let structured_outputs = generation.structured_outputs && !bridged;
        let image_input = generation.image_input && !bridged;
        let store = generation.store && !bridged;
        let reasoning = route_reasoning_support(upstream_api, bridged);
        let reasoning_levels = if reasoning == SupportState::Supported {
            upstream_api.model().reasoning_levels().to_vec()
        } else {
            Vec::new()
        };
        let (
            previous_response_id,
            background,
            prompt_caching,
            audio_input,
            file_input,
            audio_output,
        ) = protocol_specific_capabilities(route, upstream_api, bridged);

        // 将模型参数和协议控制字段收窄为该 Route 完整接受的集合。
        let model_parameters =
            sorted_unique(upstream_api.model().supported_parameters().iter().cloned());
        let interface_parameters = interface_parameters(
            route.downstream_protocol(),
            route.mode(),
            &model_parameters,
            generation.streaming,
            generation.function_calling,
            generation.parallel_tool_calls,
            structured_outputs,
            reasoning,
            store,
            previous_response_id,
            background,
        );
        let mut input = vec![InputModality::Text];
        if image_input {
            input.push(InputModality::Image);
        }
        if audio_input {
            input.push(InputModality::Audio);
        }
        if file_input {
            input.push(InputModality::File);
        }
        let mut output = vec![OutputModality::Text];
        if audio_output {
            output.push(OutputModality::Audio);
        }
        if let Some(model_input) = upstream_api.model().input_modalities() {
            input.retain(|modality| model_input.contains(modality));
        }
        if let Some(model_output) = upstream_api.model().output_modalities() {
            output.retain(|modality| model_output.contains(modality));
        }
        let model_modalities = upstream_api
            .model()
            .input_modalities()
            .zip(upstream_api.model().output_modalities())
            .map(|(input, output)| ModelModalities {
                input: sorted_values(input),
                output: sorted_values(output),
            });
        let model_reasoning = SupportState::from(upstream_api.model().reasoning());
        let model_reasoning_levels = if model_reasoning.is_supported() {
            upstream_api.model().reasoning_levels().to_vec()
        } else {
            Vec::new()
        };

        Some(Self {
            protocol: route.downstream_protocol(),
            context_window: ContextWindow::from_model(upstream_api.model().context_length()),
            modalities: ModelModalities { input, output },
            model_modalities,
            model_parameters,
            model_reasoning,
            model_reasoning_levels,
            interface_parameters,
            streaming: SupportState::from_bool(generation.streaming),
            system_messages: SupportState::Unknown,
            function_calling: SupportState::from_bool(generation.function_calling),
            parallel_tool_calls: SupportState::from_bool(generation.parallel_tool_calls),
            structured_outputs: SupportState::from_bool(structured_outputs),
            reasoning,
            reasoning_levels,
            reasoning_output: route_reasoning_output(upstream_api, bridged, reasoning),
            prompt_caching: SupportState::from_bool(prompt_caching),
            store: SupportState::from_bool(store),
            previous_response_id: SupportState::from_bool(previous_response_id),
            background: SupportState::from_bool(background),
        })
    }
}

/// 返回下游接口实际可观察的 reasoning 输出形态。
fn route_reasoning_output(
    upstream_api: &UpstreamApi,
    bridged: bool,
    reasoning: SupportState,
) -> ReasoningOutputMode {
    if !bridged {
        return upstream_api.reasoning_output().into();
    }
    match reasoning {
        SupportState::Supported => upstream_api.reasoning_output().into(),
        SupportState::Unsupported => ReasoningOutputMode::Unsupported,
        SupportState::Unknown => ReasoningOutputMode::Unknown,
    }
}

/// 按协议读取 Native endpoint 的专有能力，Bridge 一律收窄状态与额外模态。
fn protocol_specific_capabilities(
    route: &Route,
    upstream_api: &UpstreamApi,
    bridged: bool,
) -> (bool, bool, bool, bool, bool, bool) {
    if bridged {
        return (false, false, false, false, false, false);
    }
    match upstream_api.capabilities() {
        UpstreamApiCapabilities::ChatCompletions(capabilities) => (
            false,
            false,
            capabilities.prompt_caching,
            capabilities.audio_input,
            capabilities.file_input,
            capabilities.audio_output,
        ),
        UpstreamApiCapabilities::Responses(capabilities) => (
            route.downstream_protocol() == ApiProtocol::Responses
                && capabilities.previous_response_id,
            route.downstream_protocol() == ApiProtocol::Responses && capabilities.background,
            capabilities.prompt_caching,
            false,
            capabilities.file_input,
            false,
        ),
    }
}

/// 计算模型 reasoning 经当前 Route 后是否仍能作为下游请求能力公开。
fn route_reasoning_support(upstream_api: &UpstreamApi, bridged: bool) -> SupportState {
    let model_support = SupportState::from(upstream_api.model().reasoning());
    if !bridged || model_support != SupportState::Supported {
        return model_support;
    }
    match (upstream_api.protocol(), upstream_api.reasoning_output()) {
        (ApiProtocol::ChatCompletions, ReasoningOutput::PlainText)
        | (ApiProtocol::Responses, ReasoningOutput::PlainText | ReasoningOutput::Summary) => {
            SupportState::Supported
        }
        (_, ReasoningOutput::Unknown) => SupportState::Unknown,
        _ => SupportState::Unsupported,
    }
}

/// 生成单条 Route 对下游保证接受的参数名集合。
#[allow(clippy::too_many_arguments)]
fn interface_parameters(
    protocol: ApiProtocol,
    mode: RouteMode,
    model_parameters: &[String],
    streaming: bool,
    function_calling: bool,
    parallel_tool_calls: bool,
    structured_outputs: bool,
    reasoning: SupportState,
    store: bool,
    previous_response_id: bool,
    background: bool,
) -> Vec<String> {
    // Native 保留已确认模型参数，Bridge 只保留转换器的显式 allowlist。
    let mut parameters = model_parameters
        .iter()
        .filter(|parameter| {
            mode == RouteMode::Native || bridge_parameter_allowed(protocol, parameter)
        })
        .cloned()
        .collect::<BTreeSet<_>>();

    // 加入并收窄 OpenBridge 已实际门控的协议控制字段。
    if streaming {
        parameters.insert("stream".to_owned());
    }
    if function_calling {
        parameters.insert("tools".to_owned());
        parameters.insert("tool_choice".to_owned());
    } else {
        parameters.remove("tools");
        parameters.remove("tool_choice");
    }
    if parallel_tool_calls {
        parameters.insert("parallel_tool_calls".to_owned());
    } else {
        parameters.remove("parallel_tool_calls");
    }
    if structured_outputs {
        match protocol {
            ApiProtocol::ChatCompletions => {
                parameters.insert("response_format".to_owned());
            }
            ApiProtocol::Responses => {
                parameters.insert("text".to_owned());
            }
        }
    } else {
        parameters.remove("response_format");
        parameters.remove("structured_outputs");
        parameters.remove("text");
    }
    if reasoning.is_supported() {
        parameters.insert(match protocol {
            ApiProtocol::ChatCompletions => "reasoning_effort".to_owned(),
            ApiProtocol::Responses => "reasoning".to_owned(),
        });
    } else {
        parameters.remove("reasoning");
        parameters.remove("reasoning_effort");
    }
    if store {
        parameters.insert("store".to_owned());
    } else {
        parameters.remove("store");
    }
    if previous_response_id {
        parameters.insert("previous_response_id".to_owned());
    }
    if background {
        parameters.insert("background".to_owned());
    }
    parameters.into_iter().collect()
}

/// 判断参数能否由当前 Bridge 请求转换器完整表示。
fn bridge_parameter_allowed(protocol: ApiProtocol, parameter: &str) -> bool {
    match protocol {
        ApiProtocol::ChatCompletions => matches!(
            parameter,
            "max_completion_tokens"
                | "max_tokens"
                | "parallel_tool_calls"
                | "reasoning_effort"
                | "stream"
                | "temperature"
                | "tool_choice"
                | "tools"
                | "top_p"
        ),
        ApiProtocol::Responses => matches!(
            parameter,
            "max_output_tokens"
                | "parallel_tool_calls"
                | "reasoning"
                | "stream"
                | "temperature"
                | "tool_choice"
                | "tools"
                | "top_p"
        ),
    }
}

/// 把同一协议的全部完整 Route profile 收敛为唯一接口契约。
fn aggregate_interface<'a>(
    profiles: impl Iterator<Item = &'a RouteCapabilityProfile> + Clone,
) -> Option<ModelInterfaceCapabilities> {
    let profiles = profiles.collect::<Vec<_>>();
    if profiles.is_empty() {
        return None;
    }

    // 分别计算标量、集合与 reasoning 输出的保守交集。
    let context_window =
        ContextWindow::intersection(profiles.iter().map(|profile| &profile.context_window));
    let modalities =
        ModelModalities::intersection(profiles.iter().map(|profile| &profile.modalities));
    let supported_parameters = intersect_sets(
        profiles
            .iter()
            .map(|profile| profile.interface_parameters.as_slice()),
    );
    let streaming = SupportState::intersection(profiles.iter().map(|profile| profile.streaming));
    let function_calling =
        SupportState::intersection(profiles.iter().map(|profile| profile.function_calling));
    let parallel_tool_calls =
        SupportState::intersection(profiles.iter().map(|profile| profile.parallel_tool_calls));
    let structured_outputs =
        SupportState::intersection(profiles.iter().map(|profile| profile.structured_outputs));
    let reasoning = SupportState::intersection(profiles.iter().map(|profile| profile.reasoning));
    let reasoning_levels = if reasoning.is_supported() {
        intersect_sets(
            profiles
                .iter()
                .map(|profile| profile.reasoning_levels.as_slice()),
        )
    } else {
        Vec::new()
    };
    let reasoning_output =
        intersect_reasoning_output(profiles.iter().map(|profile| profile.reasoning_output));

    // 根据聚合状态构造稳定的工具、结构化输出和 state 子对象。
    Some(ModelInterfaceCapabilities {
        context_window,
        modalities,
        supported_parameters,
        streaming,
        system_messages: SupportState::intersection(
            profiles.iter().map(|profile| profile.system_messages),
        ),
        tools: ToolCapabilities {
            support: function_calling,
            types: function_calling
                .is_supported()
                .then_some(ToolType::Function)
                .into_iter()
                .collect(),
            tool_choice_modes: if function_calling.is_supported() {
                vec![
                    ToolChoiceMode::None,
                    ToolChoiceMode::Auto,
                    ToolChoiceMode::Required,
                    ToolChoiceMode::Named,
                ]
            } else {
                Vec::new()
            },
            parallel_calls: parallel_tool_calls,
            strict_schema: if function_calling.is_supported() && structured_outputs.is_supported() {
                SupportState::Supported
            } else {
                SupportState::Unsupported
            },
        },
        structured_outputs: StructuredOutputCapabilities {
            support: structured_outputs,
            modes: if structured_outputs.is_supported() {
                vec![
                    StructuredOutputMode::JsonObject,
                    StructuredOutputMode::JsonSchema,
                ]
            } else {
                Vec::new()
            },
            strict_schema: if structured_outputs.is_supported() {
                SupportState::Supported
            } else {
                SupportState::Unsupported
            },
        },
        reasoning: InterfaceReasoningCapabilities {
            support: reasoning,
            levels: reasoning_levels,
            output: reasoning_output,
        },
        prompt_caching: SupportState::intersection(
            profiles.iter().map(|profile| profile.prompt_caching),
        ),
        state: StateCapabilities {
            store: SupportState::intersection(profiles.iter().map(|profile| profile.store)),
            previous_response_id: SupportState::intersection(
                profiles.iter().map(|profile| profile.previous_response_id),
            ),
            background: SupportState::intersection(
                profiles.iter().map(|profile| profile.background),
            ),
        },
    })
}

/// 聚合 Public Model 的模型本体能力，不混入 Provider 或 Route 身份。
fn aggregate_model_capabilities(profiles: &[RouteCapabilityProfile]) -> ModelCapabilities {
    if profiles.is_empty() {
        return ModelCapabilities {
            tasks: Vec::new(),
            context_window: ContextWindow::from_model(ModelContextLength::default()),
            modalities: ModelModalities {
                input: Vec::new(),
                output: Vec::new(),
            },
            supported_parameters: Vec::new(),
            tokenizer: None,
            knowledge_cutoff: None,
            reasoning: ModelReasoningCapabilities {
                support: SupportState::Unknown,
                levels: Vec::new(),
            },
        };
    }

    // 模型本体事实同样按所有可执行 Route 取交集，避免 fallback 扩大公开能力。
    let reasoning =
        SupportState::intersection(profiles.iter().map(|profile| profile.model_reasoning));
    let declared_modalities = profiles
        .iter()
        .map(|profile| profile.model_modalities.as_ref())
        .collect::<Option<Vec<_>>>();
    ModelCapabilities {
        tasks: vec![ModelTask::Chat, ModelTask::TextGeneration],
        context_window: ContextWindow::intersection(
            profiles.iter().map(|profile| &profile.context_window),
        ),
        modalities: declared_modalities.map_or_else(
            || ModelModalities::intersection(profiles.iter().map(|profile| &profile.modalities)),
            |modalities| ModelModalities::intersection(modalities.into_iter()),
        ),
        supported_parameters: intersect_sets(
            profiles
                .iter()
                .map(|profile| profile.model_parameters.as_slice()),
        ),
        tokenizer: None,
        knowledge_cutoff: None,
        reasoning: ModelReasoningCapabilities {
            support: reasoning,
            levels: if reasoning.is_supported() {
                intersect_sets(
                    profiles
                        .iter()
                        .map(|profile| profile.model_reasoning_levels.as_slice()),
                )
            } else {
                Vec::new()
            },
        },
    }
}

/// 只有全部 Route 都提供数值时才返回安全最小值。
fn intersect_optional_limit(values: impl Iterator<Item = Option<u32>>) -> Option<u32> {
    let values = values.collect::<Vec<_>>();
    if values.is_empty() || values.iter().any(Option::is_none) {
        None
    } else {
        values.into_iter().flatten().min()
    }
}

/// 对有序可比较集合计算稳定交集。
fn intersect_sets<'a, T>(values: impl Iterator<Item = &'a [T]>) -> Vec<T>
where
    T: Clone + Ord + 'a,
{
    let mut values = values.map(|value| value.iter().cloned().collect::<BTreeSet<_>>());
    let Some(mut intersection) = values.next() else {
        return Vec::new();
    };
    for value in values {
        intersection.retain(|item| value.contains(item));
    }
    intersection.into_iter().collect()
}

/// 将任意参数迭代器去重并按 wire 名称排序。
fn sorted_unique(values: impl Iterator<Item = String>) -> Vec<String> {
    values.collect::<BTreeSet<_>>().into_iter().collect()
}

/// 复制并稳定排序一个已校验无重复的枚举集合。
fn sorted_values<T: Clone + Ord>(values: &[T]) -> Vec<T> {
    values
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

/// 仅在所有 Route 返回相同 reasoning 输出形态时公开该形态。
fn intersect_reasoning_output(
    mut values: impl Iterator<Item = ReasoningOutputMode>,
) -> ReasoningOutputMode {
    let Some(first) = values.next() else {
        return ReasoningOutputMode::Unknown;
    };
    if values.all(|value| value == first) {
        first
    } else {
        ReasoningOutputMode::Unknown
    }
}
