# 扩展能力合同

本文集中定义 Embeddings、Native 图片/文件/音频和 Images Generations 的固定接口、资源边界与验收约束。

共同能力边界与各域合同的叶子划分如下；每个域有独立验收编号。

| 叶子 | 只回答什么 | 验收 |
|---|---|---|
| [共同能力边界](extended-capabilities/common.md) | 能力分层、编译与预检、Native 保真、资源保护、retry/取消、共同非目标 | — |
| [Embeddings](extended-capabilities/embeddings.md) | input forms、encoding、dimension、响应预算 | EMB-01..04 |
| [Native 图片输入](extended-capabilities/image-input.md) | image part、source、URL/Base64、detail | IMG-01..04 |
| [Native 文件输入](extended-capabilities/file-input.md) | file part、source 规则、resource identity | FILE-01..04 |
| [Native 音频](extended-capabilities/audio.md) | 五任务身份、mimo-v2.5 系列、audio output 预算 | AUD-01..09 |
| [Images Generations](extended-capabilities/images-generations.md) | 下游契约、DashScope wire、响应验证 | GEN-01..05 |
