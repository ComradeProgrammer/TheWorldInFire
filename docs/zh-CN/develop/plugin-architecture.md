# 规则插件架构

游戏规则可以通过插件替换和扩展。一个精简的内核负责游戏状态和回合结构；编译为 WebAssembly 的规则插件决定每个阶段和每条命令的行为。官方 NATO 规则本身就是这样一个插件，它随内核打包并始终加载。

## 状态

已实现：

- 带 Wasmtime 宿主的内核（`crates/ooaw-core`）、共享协议 crate（`crates/ooaw-plugin-api`）和 Rust 插件 SDK（`crates/ooaw-plugin-sdk`）。
- 官方 NATO 规则（`plugins/nato-official`，包名 `ooaw-nato`）。它在构建时编译为 WebAssembly 并内嵌进内核；桌面应用、`ooaw-engine` 进程和所有测试都以 WebAssembly 插件的形式运行它。
- 官方战斗与空中打击规则中的过滤器（具名扩展点）。
- 一个示例第三方插件（`plugins/examples/night-fighting`），内核测试会把它与官方规则一起加载。

尚未实现：在桌面应用中加载第三方插件、保存与回放游戏，以及本文末尾列出的其他计划工作。

## 职责划分

```text
┌──────────── ooaw-core（内核，原生代码）────────────┐
│ 权威状态 · 带种子的骰子 · 回合流程                │
│ 命令路由 · 全有或全无的命令执行 · 快照            │
│ 插件注册表 · Wasmtime 沙箱                         │
└───────────▲───────────────────────────┬────────────┘
            │ 导入：roll、host_call     │ 导出：ooaw_dispatch
┌───────────┴───────────────────────────▼────────────┐
│ 规则插件（WebAssembly）                            │
│ ooaw-nato（官方，内置）· 第三方插件                │
└────────────────────────────────────────────────────┘
```

内核不了解任何 NATO 规则。它负责：

- **状态。**当前回合位置、单位、城市控制、待玩家决定的事项，以及规则拥有的状态条目。
- **骰子。**一个以游戏 ID 为种子的 SplitMix64 生成器。插件通过导入函数掷骰，因此每个结果都能由种子和命令历史复现。
- **回合流程。**内核按场景的阶段步骤推进，发出 `phaseStarted`、`phaseEnded`、`gameTurnStarted` 和 `gameCompleted` 事件，并自动执行自动阶段。
- **路由。**`endPhase` 由内核处理；其他所有命令和查询都交给声明它们的插件。
- **事务。**每条命令执行前，内核会复制状态和骰子；如果命令被拒绝或插件出错，两者都会恢复，revision 不变。
- **一致性检查。**插件只能通过经过检查的状态变更来修改状态：单位必须位于已知格子且战力面索引有效，城市必须存在，规则状态条目不能占用内核字段名。内核不检查任何游戏规则。

其余一切都归规则插件负责：场景及其内容、阶段行为、命令校验与执行、预览，以及插件自己的状态（保存在内核中）。

## Crate

| Crate | 作用 |
| --- | --- |
| `ooaw-plugin-api` | 共享词汇：标识、地图、阶段、阵营、通用单位，以及 `protocol` 模块（ABI、调用、状态变更、清单）。可原生编译，也可编译为 `wasm32`。 |
| `ooaw-plugin-sdk` | 用 Rust 编写插件：`RulesPlugin` trait、`export_plugin!` 宏、宿主函数（`host::roll`、`host::filter`、`host::log`），以及计算状态变更的 `mirror` 辅助函数。 |
| `ooaw-core` | 内核：`GameEngine`、Wasmtime 运行时、内置官方插件，以及 `ooaw-engine` 可执行文件。 |
| `ooaw-nato` | 官方 NATO 规则。作为 `cdylib` 是 WebAssembly 插件；作为 `rlib` 是供测试和扩展这些规则的插件使用的类型模型。 |
| `ooaw-example-night-fighting` | 示例第三方插件，与内核一起构建，供内核测试使用。 |

这些 crate 与 `src-tauri` 共同组成以仓库根目录为根的 Cargo 工作区。

## 运行时

插件在 Wasmtime 中运行：每个进程用 Cranelift 编译一次，没有任何 WASI 导入。插件接触外界的唯一途径是内核提供的 `ooaw` 导入函数。

- **确定性。**开启 NaN 规范化，关闭宽松 SIMD 和线程，因此插件在任何机器上都算出相同的结果。随机性只能来自 `roll`。
- **沙箱。**只加载内置插件的游戏运行在*受信任*沙箱中，编译时不插入中断检查。只要加载了任何第三方插件，游戏就运行在*受保护*沙箱中：每次顶层调用都有约十秒的实际时间期限，由 epoch 中断强制执行，用于终止永不返回的插件。这个期限不会改变任何正常结束的调用的结果。燃料计量（fuel）经实测会让规则代码慢约 40%，因此没有采用。
- **内存。**每个实例最多使用 512 MiB 线性内存。
- **故障。**陷阱、超时或格式错误的响应会让调用以 `pluginFault` 失败。内核回滚命令，并在下次调用该插件之前替换实例，把新实例重新挂接到游戏上。
- **启动。**内置插件在第一次使用时编译，release 构建约需 0.2 秒；桌面应用在启动时于后台线程中完成编译。

同一局游戏的所有插件实例共享一个 Wasmtime store。store 的数据保存内核状态，因此在一个插件运行期间，内核可以应用它的变更，并调用参与过滤器的其他插件。

## 二进制接口（ABI 版本 1）

所有消息都是 UTF-8 JSON。插件是一个 WebAssembly 核心模块，导出：

| 导出 | 作用 |
| --- | --- |
| `memory` | 线性内存。 |
| `ooaw_abi_version() -> i32` | 返回 1。 |
| `ooaw_alloc(len) -> ptr` | 供内核写入请求的缓冲区。 |
| `ooaw_free(ptr, len)` | 释放请求缓冲区或响应。 |
| `ooaw_dispatch(ptr, len) -> i64` | 处理一个 `DispatchRequest`，返回 `DispatchResponse` 的 `(response_ptr << 32) \| response_len`。 |

插件可以从 `ooaw` 模块导入：

| 导入 | 作用 |
| --- | --- |
| `roll(sides) -> i32` | 游戏骰子的下一个值，范围 `1..=sides`。在查询和过滤器中调用会触发陷阱。 |
| `host_call(ptr, len) -> i32` | 发送一个 `HostRequest`（`filter` 或 `log`），返回响应长度。 |
| `host_read(ptr)` | 把该响应复制进插件内存。 |

具体消息类型见 `crates/ooaw-plugin-api/src/protocol.rs`。在游戏模型仍在变化时，JSON 让任何语言都能方便地遵循这个协议；模型稳定后可以改用带类型的 WIT 接口。

## 调用与路由

插件通过 `PluginManifest` 描述自己：标识、版本、ABI 版本、所处理的阶段类型、命令类型、查询和过滤器，它提供的场景，以及它是否为 `stateless`（无状态）。创建游戏时传入一个有序的插件列表（官方插件在最前）：

- 场景来自最后一个列出该场景 ID 的插件；
- 阶段、命令或查询由最后一个声明它的插件处理；
- 列出某个过滤器的每个插件都会按加载顺序参与该过滤器。

| 调用 | 作用 | 能否修改状态 |
| --- | --- | --- |
| `manifest` | 描述插件。 | 否 |
| `createGame` | 构建场景：摘要、地图、回合流程和开局状态。 | 否 |
| `attach` | 为已有游戏服务（其他插件，或替换后的实例）。 | 否 |
| `applySetup` | 按插件自己的格式铺设自定义起始局面。 | 是 |
| `phaseStarted`、`phaseEnding` | 阶段处理；拒绝 `phaseEnding` 会让阶段保持不变。 | 是 |
| `command` | 执行玩家命令。 | 是 |
| `query` | 回答只读预览。 | 否 |
| `filter` | 在扩展点调整一个值。 | 否 |

只读调用若上报状态变更或掷骰，会以 `readOnlyViolation` 失败或触发陷阱。

## 状态镜像与变更

每个插件都保存自己需要的可变状态的镜像，因此内核不必每次调用都重新发送完整状态。内核维护一个代号（generation），并记录每个插件镜像对应的代号。只有镜像过期时，请求才会携带完整的 `GameState`（单位、城市控制、待决事项和规则状态条目）：首次调用、回滚之后、直接编辑之后，或其他插件修改状态之后。回合位置随每次调用一起发送。

插件直接修改镜像，然后把差异作为 `Change` 上报：

| 变更 | 效果 |
| --- | --- |
| `putUnit` | 新增单位，或整体替换单位。 |
| `updateUnit` | 替换指定的顶层字段；其他插件拥有的标记保持不变。 |
| `removeUnit` | 把单位移出游戏。 |
| `setCityControl` | 更改城市的控制方。 |
| `setRules` | 替换一个规则状态条目。 |
| `setPendingDecision` | 设置或清除玩家必须做出的决定。 |

内核中的单位是通用的：身份、阵营、国籍、类型、编制、特性、由规则定义印刷数值的战力面列表、战力面索引、位置，以及*标记*，即平铺在单位对象中的规则字段。NATO 规则的带类型单位序列化后正好是这个结构，`supply`、`trainStatus` 和 `disruption` 都是标记。规则拥有的状态条目也以同样方式平铺在快照中，因此面向客户端的 JSON 保持了拆分之前的结构。

NATO 插件保存一份基线，记录上次上报的状态。每次可修改状态的调用之后，SDK 的 `mirror` 辅助函数逐字段比较基线与当前镜像。

## 过滤器

过滤器是具名的扩展点。需要某个值的插件先自己算出它，再依次交给参与该过滤器的其他插件；每个插件可以返回调整后的值。若没有其他插件参与，就直接跳过调用。调用前，发起方会先发送尚未上报的变更，使其他插件看到当前状态。过滤器是只读的。

官方规则提供以下过滤器（`plugins/nato-official/src/filters.rs`）：

| 过滤器 | 输入 | 值 |
| --- | --- | --- |
| `nato.combat.attackStrength` | `{ sideId, objective, unitId }` | 一个进攻单位应用印刷修正后的 `UnitStrength` |
| `nato.combat.defenseStrength` | `{ sideId, objective, unitId }` | 一个防守单位应用印刷修正后的 `UnitStrength` |
| `nato.combat.columnShifts` | `{ sideId, objective, attackingUnitIds, offensiveSupport }` | 应用两列上限之前的战斗比列移 |
| `nato.combat.result` | `{ sideId, objective, finalColumn, dieRoll }` | 从战斗结果表读出的 `CombatResult` |
| `nato.air.strikeModifier` | `{ sideId, hexId, unitIds }` | 一个目标在打击表上的掷骰修正 |

战斗预览与实际结算使用同样的过滤器，因此预览总与战斗一致。其他插件添加的强度修正或列移带有该插件自己的名称，例如 `example.nightFighting`；战斗计划界面按原样显示这个名称。

示例插件 `plugins/examples/night-fighting` 为每场战斗追加一个 −1 列移。它只参与 `nato.combat.columnShifts`，声明自己为 `stateless`，并且像其他语言编写的插件一样直接使用 JSON，而不依赖 `ooaw-nato` 的类型。`crates/ooaw-core/tests/plugins.rs` 检查它同时影响预览和实际结算。

## 用 Rust 编写插件

下面是省略函数体的提纲；`plugins/examples/night-fighting/src/lib.rs` 是一个完整的插件。

```rust
use ooaw_plugin_sdk::{export_plugin, Attachment, RulesPlugin};

#[derive(Default)]
struct MyRules { /* 镜像与基线 */ }

impl RulesPlugin for MyRules {
    fn manifest(&self) -> PluginManifest { /* 阶段、命令、查询、过滤器 */ }
    fn attach(&mut self, attachment: Attachment) -> Result<(), RuleError> { Ok(()) }
    fn set_turn(&mut self, turn: TurnPosition) {}
    fn sync(&mut self, state: Value) -> Result<(), RuleError> { /* 载入镜像 */ }
    fn take_changes(&self) -> Result<Vec<Change>, RuleError> { /* mirror 辅助函数 */ }
    fn filter(&self, name: &str, input: Value, value: Value) -> Result<Value, RuleError> { Ok(value) }
}

export_plugin!(MyRules);
```

以 `crate-type = ["cdylib", "rlib"]` 为 `wasm32-unknown-unknown` 构建，然后用 `PluginModule::from_wasm` 载入字节，并把模块放在官方插件之后传给 `GameEngine::with_plugins`。

## 构建

`crates/ooaw-core/build.rs` 调用 Cargo，以 release 模式为 `wasm32-unknown-unknown` 构建 `ooaw-nato` 和示例插件，使用独立的目标目录 `target/wasm-plugins`，再把模块复制到 `OUT_DIR` 供内核内嵌。因此每次构建内核（包括 `npm run tauri dev` 和 `cargo check`）都需要这个编译目标：

```sh
rustup target add wasm32-unknown-unknown
```

官方模块约 2 MB，其中包括内嵌的地图和战役数据。即使在开发构建中，Cranelift 和 Wasmtime 的编译器包也会优化编译（见工作区 `Cargo.toml`），因为未优化的 Cranelift 编译插件要慢很多倍。

## 测试

- `plugins/nato-official/tests/rules.rs` 和 `campaign_scenarios.rs`：NATO 规则测试集，全部经过内核与 WebAssembly 插件运行。`tests/common/mod.rs` 中的测试辅助模块通过带类型的 NATO 模型读取状态，并用 `GameEngine::edit_state` 修改状态；由内核控制开关的 `debug.checkSupply` 命令可按需执行补给检查。
- `crates/ooaw-core/tests/kernel.rs`：路由、事务、只读查询、调试命令开关和直接编辑。
- `crates/ooaw-core/tests/plugins.rs`：受保护沙箱中的第三方过滤器插件。
- `crates/ooaw-core/tests/engine_black_box.rs` 和 `campaign_scenarios.rs`：`ooaw-engine` JSON 进程。
- 变更比较（`ooaw-plugin-sdk`）和变更检查（`ooaw-core`）的单元测试。

## 性能

下表是在开发机上测得的 release 构建单次调用耗时，并与拆分前的原生引擎对比。

| 操作 | 原生引擎 | 插件，受信任沙箱 | 插件，受保护沙箱 |
| --- | --- | --- | --- |
| BALTAP `movementModes`（一个单位，所有移动方式） | 4.4 ms | 4.4 ms | 6.6 ms |
| 341 个单位的战役 `movementModes` | 10.0 ms | 9.9 ms | 14.5 ms |
| BALTAP `endPhase` | 0.9 ms | 1.0 ms | 1.3 ms |
| 341 个单位的战役 `endPhase` | 9.2 ms | 11.7 ms | 15.5 ms |

在受信任沙箱中，规则代码以原生速度运行。大场景中 `endPhase` 剩余的额外开销来自变更的上报与检查。

## 计划工作

- 在桌面应用中加载第三方插件（发现、按局选择、显示清单）。目前只有 `GameEngine::with_plugins` 能接收它们。
- 存档和回放必须记录插件列表（含版本和模块哈希）以及骰子状态。
- 插件不能调用正在处理调用的插件（`reentrantPluginCall`）；过滤器参与者暂时不能向其他插件发起查询。
- 每个阶段、命令和查询只有一个处理者，目前还没有观察者钩子。
- 地图模型中的地形、城市和六角边类型是所有插件共享的封闭枚举。
- 过期插件会收到完整状态；如果只发送其上次同步之后的变更，加载多个有状态插件的游戏会更省开销。
- 预编译模块（`.cwasm`）可以省去启动时的编译。
- 内核目前无法显示插件自带的界面；前端理解官方规则的状态和预览，对其他插件的名称按原样显示。
- 故障恢复路径（陷阱后替换实例）还没有自动化测试。
