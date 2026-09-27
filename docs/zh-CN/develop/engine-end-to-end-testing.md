# 游戏引擎端到端测试

游戏引擎提供名为 `ooaw-engine` 的逐行 JSON 进程适配器。自动化客户端、回放工具和黑盒测试可以通过它使用引擎，而不需要链接 Rust 引擎内部代码。

标准输入的每一行表示一个请求，进程会在标准输出写回且只写回一行 JSON 响应。进程会保留一个游戏会话，直到后续的 `newGame` 请求替换它或进程退出。诊断信息写入标准错误，保证标准输出始终是稳定的协议数据流。

支持以下请求：

- `listScenarios`
- `newGame`，包含 `scenarioId` 和用于确定性随机结果的 `gameId`
- `getSnapshot`
- `submitCommand`，包含 `expectedRevision` 和正常序列化的游戏命令

响应类型包括 `gameStarted`、`snapshot`、`commandAccepted`、`scenarios` 和 `error`。命令被接受时，响应包含新的版本号、领域事件和权威快照。命令被拒绝时，版本号不会增加。客户端提交下一条命令时必须使用最新版本号。

集成测试 `crates/ooaw-core/tests/engine_black_box.rs` 会启动已编译的可执行文件，并且只通过 JSON 与它通信。测试不导入任何 `ooaw_core` 类型，同时验证会话错误、规则拒绝、自动阶段处理、增援事件、状态变更和乐观并发拒绝。

运行方式：

```sh
cargo test --manifest-path crates/ooaw-core/Cargo.toml --test engine_black_box
```

会话示例：

```json
{"type":"newGame","scenarioId":"nato-1983-standard","gameId":"example"}
{"type":"submitCommand","expectedRevision":0,"command":{"type":"endPhase"}}
{"type":"getSnapshot"}
```
