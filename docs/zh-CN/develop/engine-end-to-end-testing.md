# 游戏引擎端到端测试

内核提供名为 `ooaw-engine` 的逐行 JSON 进程适配器（`crates/ooaw-core/src/bin/ooaw-engine.rs`）。它使用内置的官方规则插件运行游戏；自动化客户端、回放工具和黑盒测试可以通过它使用引擎，而不需要链接 Rust 引擎内部代码。

标准输入的每一行表示一个请求，进程会在标准输出写回且只写回一行 JSON 响应。进程会保留一个游戏会话，直到后续的 `newGame` 请求替换它或进程退出。诊断信息写入标准错误，保证标准输出始终是稳定的协议数据流。

支持以下请求：

- `listScenarios`
- `newGame`，包含 `scenarioId`、用于确定性随机结果的 `gameId`，以及可选的 `setup`
- `getSnapshot`
- `submitCommand`，包含 `expectedRevision` 和正常序列化的游戏命令

响应类型包括 `gameStarted`、`snapshot`、`commandAccepted`、`scenarios` 和 `error`。命令被接受时，响应包含新的版本号、领域事件和权威快照。命令被拒绝时，版本号不会增加。客户端提交下一条命令时必须使用最新版本号。

集成测试 `crates/ooaw-core/tests/engine_black_box.rs` 会启动已编译的可执行文件，并且只通过 JSON 与它通信。测试不导入任何 `ooaw_core` 类型，同时验证会话错误、创建游戏时结算的自动开局、规则拒绝、事件与版本号、状态变更和乐观并发拒绝。第二个测试进行一个 BALTAP 回合：把一个华约师标记为 OMG，检查华约预备队阶段只有该单位可以移动且只能战术移动，结束双方的预备队阶段（移除标记并进行战后阶段），并检查下一回合战前阶段的补给报告。

## 从自定义局面开始

`newGame` 可以带一个可选的 `setup`，让测试从任意局面开始。内核先创建剧本，然后不断结束阶段直到到达 `start`（途中所有阶段照常结算，包括增援和补给），再把 setup 交给拥有该剧本的插件铺设其余内容。NATO 规则的格式是 `plugins/nato-official/src/setup.rs` 中的 `GameSetup`。所有字段都是可选的：

| 字段 | 作用 |
| --- | --- |
| `start` | `{ gameTurn, phaseId, sideId? }`：开始的步骤；联合阶段省略 `sideId`。默认是第一个步骤。 |
| `units` | 替换所有在场单位。每项为 `{ id, hex?, step?, disruption?, supply?, trainStatus? }`。`id` 可以是剧本中的任意单位，包括尚未到达的增援；省略 `hex` 表示放入战略预备队；`step` 是战力面序号（0 为满编）；`supply`（`supplied` 或 `outOfSupply`）作用于该单位具有的所有补给类型，默认有补给。 |
| `cityControl` | `[{ hexId, controller }]`，覆盖城市控制方。 |
| `breakthroughMarkers` | 替换突破标记：`[{ sideId, hexId }]`。 |
| `airInterdictionZones` | 替换空中遮断区：`[{ sideId, hexId }]`。 |
| `airUnits` | 覆盖空军算子的战力面与状态：`[{ id, step?, readiness? }]`。 |
| `airBases` | 覆盖地图外机场状态：`[{ id, damage?, suppressedThroughTurn? }]`。 |
| `attackTargets`、`reserveUnitIds` | 设置当前战斗计划的攻击目标和预备队/OMG 单位；需要已有战斗计划，因此 `start` 必须在该方战斗计划阶段或之后。 |

局面直接替换状态而不重新检查规则，所以测试可以构造正常游戏要很多回合才能形成的局面。飞机 `step` 只允许 0 或 1，机场 `damage` 只允许 0–2。开始步骤本身的自动结算已经执行：若要让引擎为铺设的局面检查补给，应从上一回合北约的预备队阶段开始，再提交 `endPhase`；双方的战前阶段都在每个回合开始时进行。未知的单位、格子、阵营或字段，或无法到达的 `start`，都会以 `invalidSetup` 拒绝。被拒绝的请求不会开始游戏。

黑盒测试用它让三个苏军师包围一个西德旅，并检查北约下一个战前步骤会使其缺乏补给，同时检查各种拒绝情况。

运行方式：

```sh
cargo test -p ooaw-core --test engine_black_box
```

会话示例：

```json
{"type":"newGame","scenarioId":"nato-1983-standard","gameId":"example"}
{"type":"newGame","scenarioId":"nato-baltap-1983","gameId":"pocket","setup":{"start":{"gameTurn":1,"sideId":"nato","phaseId":"reserve"},"units":[{"id":"westGermany.6panzergrenadierDivision.16panzergrenadierBrigade","hex":"2617"},{"id":"soviet.2gta.21motorRifleDivision","hex":"2516"}]}}
{"type":"submitCommand","expectedRevision":0,"command":{"type":"endPhase"}}
{"type":"getSnapshot"}
```
