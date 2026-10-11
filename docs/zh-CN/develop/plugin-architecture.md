# 规则插件架构

游戏规则可以通过插件替换和扩展。一个精简的内核负责游戏状态和回合结构；编译为 WebAssembly 的规则插件决定每个阶段和每条命令的行为。官方 NATO 规则本身就是这样一个插件，它随内核打包并始终加载。

## 状态

已实现：

- 带 Wasmtime 宿主的内核（`crates/ooaw-core`）、以及共享插件接口 crate（`crates/ooaw-plugin-api`），其中也包含 Rust 插件 SDK。
- 官方 NATO 规则（`crates/plugins/nato-official`，包名 `ooaw-nato`）。它在构建时编译为 WebAssembly 并内嵌进内核；桌面应用、`ooaw-engine` 进程和所有测试都以 WebAssembly 插件的形式运行它。
- 一个示例第三方插件（`crates/plugins/examples/night-fighting`），用于测试受保护加载和清单校验。

尚未实现：在桌面应用中加载第三方插件、保存与回放游戏，以及本文末尾列出的其他计划工作。

## 职责划分

```text
┌──────────── ooaw-core（内核，原生代码）────────────┐
│ 权威状态 · 带种子的骰子 · 回合流程                │
│ 固定命令 · 全有或全无的命令执行 · 快照            │
│ 插件注册表 · Wasmtime 沙箱                         │
└───────────▲───────────────────────────┬────────────┘
            │ 导入：roll、filter、log   │ 导出：每个 RulesPlugin 方法一个
┌───────────┴───────────────────────────▼────────────┐
│ 单个规则插件（WebAssembly）                        │
│ ooaw-nato（官方，内置）或一个第三方插件             │
└────────────────────────────────────────────────────┘
```

内核不了解任何 NATO 规则。它负责：

- **状态。**当前回合位置、单位、城市控制、待玩家决定的事项，以及规则拥有的状态条目。
- **骰子。**一个以游戏 ID 为种子的 SplitMix64 生成器。插件通过导入函数掷骰，因此每个结果都能由种子和命令历史复现。
- **回合流程。**内核按场景的阶段步骤推进，发出 `phaseStarted`、`phaseEnded`、`gameTurnStarted` 和 `gameCompleted` 事件，并自动执行自动阶段。
- **命令。**`GameCommand` 是内核与所有插件共享的封闭 enum。`endPhase` 由内核处理；其他命令全部交给本局唯一的插件，由它决定如何校验和执行。查询仍通过清单中的名称注册。
- **事务。**每条命令执行前，内核会复制状态和骰子；如果命令被拒绝或插件出错，两者都会恢复，revision 不变。
- **一致性检查。**插件只能通过经过检查的状态变更来修改状态：单位必须位于已知格子且战力面索引有效，城市必须存在，规则状态条目不能占用内核字段名。内核不检查任何游戏规则。

其余一切都归规则插件负责：场景及其内容、阶段行为、命令校验与执行、预览，以及插件自己的状态（保存在内核中）。

## Crate

| Crate | 作用 |
| --- | --- |
| `ooaw-plugin-api` | 共享模型（命令、标识、地图、阶段、阵营、通用单位、游戏状态）与插件契约。`sdk` 包含 `RulesPlugin` trait、`export_plugin!` 宏、宿主函数（`sdk::host::roll`、`filter`、`log`），以及计算状态变更的 `sdk::mirror` 辅助函数；`protocol` 包含 trait 签名中内核也会用到的类型（清单、挂接信息、场景设置、状态变更）。`abi` 声明内核与插件之间的 `extern "C"` 函数。可原生编译，也可编译为 `wasm32`。 |
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

每局游戏只有一个插件实例和一个 Wasmtime store。store 的数据保存权威内核状态与插件运行信息。

## 插件契约（ABI 版本 4）

插件只能用 Rust 编写。内核与插件之间的契约是 `ooaw-plugin-api` 中的 `RulesPlugin` trait 及其签名中的类型。插件实现该 trait 并调用一次 `export_plugin!`。不支持其他语言。

二进制接口是一组 `extern "C"` 函数，声明在 `crates/ooaw-plugin-api/src/abi.rs` 中，每个函数都有一个名称常量和一个函数指针签名。插件的导出与 trait 方法一一对应；`export_plugin!` 根据插件的实现生成这些导出，并在编译期逐一按签名检查。插件的导入是内核提供的服务，由 `sdk::host` 封装，也以同样方式检查。内核按名称查找每个函数。

WebAssembly 函数只能接收和返回数字，因此其他值存放在插件的线性内存中，以指针和字节长度传递。文本（过滤器名、日志消息）使用 UTF-8。其他所有值使用 MessagePack，两侧都由 `abi::encode` 写入、由 `abi::decode` 读取；多个参数作为一个元组传递。返回值的函数返回 `(ptr << 32) | len`，内核用 `ooaw_free` 释放该缓冲区。可能失败的方法返回 `Result<T, RuleError>`，因此规则拒绝作为数据而不是陷阱跨越边界。`GameCommand` 是共享的内部标记 enum；事件、标记与规则状态仍是开放值。

| 导出 | trait 方法 | 参数 → 结果 |
| --- | --- | --- |
| `memory` | — | 线性内存。 |
| `ooaw_abi_version() -> i32` | — | 返回 `ABI_VERSION`，当前为 4。 |
| `ooaw_alloc(len) -> ptr` | — | 供内核写入参数的缓冲区。 |
| `ooaw_free(ptr, len)` | — | 释放结果缓冲区。 |
| `ooaw_manifest() -> i64` | `manifest` | → `PluginManifest` |
| `ooaw_set_turn(game_turn, step_index, completed)` | `set_turn` | 只传数字；无结果。 |
| `ooaw_sync(ptr, len) -> i64` | `sync` | `GameState` → `Result<()>` |
| `ooaw_take_changes() -> i64` | `take_changes` | → `Result<Vec<Change>>` |
| `ooaw_create_game(ptr, len) -> i64` | `create_game` | `(GameId, ScenarioRequest)` → `Result<ScenarioSetup>` |
| `ooaw_attach(ptr, len) -> i64` | `attach` | `Attachment` → `Result<()>` |
| `ooaw_apply_setup(ptr, len) -> i64` | `apply_setup` | setup → `Result<Vec<event>>` |
| `ooaw_phase_started(ptr, len) -> i64` | `phase_started` | `PhaseDefinition` → `Result<Vec<event>>` |
| `ooaw_phase_ending(ptr, len) -> i64` | `phase_ending` | `PhaseDefinition` → `Result<Vec<event>>` |
| `ooaw_command(ptr, len) -> i64` | `command` | `GameCommand` → `Result<Vec<event>>` |
| `ooaw_query(ptr, len) -> i64` | `query` | `(name, input)` → `Result<value>` |
| `ooaw_filter(ptr, len) -> i64` | `filter` | `(name, input, value)` → `Result<value>` |

| 导入（模块 `ooaw`） | SDK 函数 | 作用 |
| --- | --- | --- |
| `roll(sides) -> i32` | `host::roll` | 游戏骰子的下一个值，范围 `1..=sides`。在查询和过滤器中调用会触发陷阱。 |
| `filter_active(ptr, len) -> i32` | `host::filter_active` | 单插件内核中始终返回 0；仅为 ABI 兼容保留。 |
| `filter(ptr, len) -> i32` | `host::filter` | 兼容导入：应用待上报变更并原样返回 value。 |
| `host_read(ptr)` | — | 把上一次 `filter` 的结果复制进插件内存。 |
| `log(ptr, len)` | `host::log` | 把一条消息写入内核日志。 |

内核对一个插件方法的一次调用由一小串调用组成：`ooaw_set_turn`；镜像过期时调用 `ooaw_sync`；方法本身；可能修改状态的方法成功后再调用 `ooaw_take_changes`。`create_game` 和 `attach` 会重新开始镜像，因此不需要同步，也不上报变更。无状态插件既不会被同步，也不会被要求上报变更。

这些函数或其传值方式一旦变化，`ABI_VERSION` 就随之改变。内核会拒绝按其他版本构建的插件，因此插件必须使用匹配版本的 `ooaw-plugin-api` 构建。版本 1 通过单一的 `ooaw_dispatch` 导出传递 JSON 封装消息；版本 2 以 MessagePack 传递同样的封装消息；版本 4 用共享 `GameCommand` enum 取代插件自行声明的命令字符串。

## 调用与路由

插件通过 `PluginManifest` 描述自己：标识、版本、ABI 版本、所处理的阶段类型和查询、它提供的场景，以及它是否为 `stateless`（无状态）。每局游戏必须恰好加载一个插件，并由它提供选中的场景。命令不再通过清单注册：内核只接受共享 `GameCommand` enum 中的变体，并把所有非内核命令交给这个唯一插件。

| 方法 | 作用 | 能否修改状态 |
| --- | --- | --- |
| `manifest` | 描述插件。 | 否 |
| `create_game` | 构建场景：摘要、地图、回合流程和开局状态。 | 否 |
| `attach` | 插件故障后重新挂接替换实例。 | 否 |
| `apply_setup` | 按插件自己的格式铺设自定义起始局面。 | 是 |
| `phase_started`、`phase_ending` | 阶段处理；拒绝 `phase_ending` 会让阶段保持不变。 | 是 |
| `command` | 执行玩家命令。 | 是 |
| `query` | 回答只读预览。 | 否 |
| `filter` | 在扩展点调整一个值。 | 否 |

只读调用若上报状态变更或掷骰，会以 `readOnlyViolation` 失败或触发陷阱。

## 状态镜像与变更

插件保存自己需要的可变状态镜像，因此内核不必每次调用都重新发送完整状态。内核维护一个代号（generation）；只有镜像过期时，才通过 `sync` 发送完整的 `GameState`（单位、城市控制、待决事项和规则状态条目）：首次调用、回滚之后或直接编辑之后。每次调用前，内核都通过 `set_turn` 发送回合位置。

插件直接修改镜像，然后把差异作为 `Change` 上报：

| 变更 | 效果 |
| --- | --- |
| `putUnit` | 新增单位，或整体替换单位。 |
| `updateUnit` | 替换指定的顶层字段；未列出的字段保持不变。 |
| `removeUnit` | 把单位移出游戏。 |
| `setCityControl` | 更改城市的控制方。 |
| `setRules` | 替换一个规则状态条目。 |
| `setPendingDecision` | 设置或清除玩家必须做出的决定。 |

内核中的单位是通用的：身份、阵营、国籍、类型、编制、特性、由规则定义印刷数值的战力面列表、战力面索引、位置，以及*标记*，即平铺在单位对象中的规则字段。NATO 规则的带类型单位序列化后正好是这个结构，`supply`、`trainStatus` 和 `disruption` 都是标记。规则拥有的状态条目也以同样方式平铺在快照中，因此面向客户端的 JSON 保持了拆分之前的结构。

NATO 插件保存一份基线，记录上次上报的状态。每次可修改状态的调用之后，SDK 的 `mirror` 辅助函数逐字段比较基线与当前镜像。

## 过滤器

过滤器函数和清单字段仍保留在 ABI 版本 4 中，但内核规定每局游戏只能有一个插件。因此 `filter_active` 始终返回 false，filter 调用会原样返回输入值，不再启用跨插件规则扩展。

## 用 Rust 编写插件

下面是省略函数体的提纲；`crates/plugins/examples/night-fighting/src/lib.rs` 是一个完整的插件。

```rust
use ooaw_plugin_api::export_plugin;
use ooaw_plugin_api::sdk::{Attachment, RulesPlugin};

#[derive(Default)]
struct MyRules { /* 镜像与基线 */ }

impl RulesPlugin for MyRules {
    fn manifest(&self) -> PluginManifest { /* 阶段、查询、过滤器、场景 */ }
    fn attach(&mut self, attachment: Attachment) -> Result<(), RuleError> { Ok(()) }
    fn set_turn(&mut self, turn: TurnPosition) {}
    fn sync(&mut self, state: Value) -> Result<(), RuleError> { /* 载入镜像 */ }
    fn take_changes(&self) -> Result<Vec<Change>, RuleError> { /* mirror 辅助函数 */ }
    fn filter(&self, name: &str, input: Value, value: Value) -> Result<Value, RuleError> { Ok(value) }
}

export_plugin!(MyRules);
```

以 `crate-type = ["cdylib", "rlib"]` 为 `wasm32-unknown-unknown` 构建，然后用 `PluginModule::from_wasm` 载入字节，并把它作为本局唯一插件传给游戏。

## 构建

`crates/ooaw-core/build.rs` 调用 Cargo，以 release 模式为 `wasm32-unknown-unknown` 构建 `ooaw-nato` 和示例插件，使用独立的目标目录 `target/wasm-plugins`，再把模块复制到 `OUT_DIR` 供内核内嵌。因此每次构建内核（包括 `npm run tauri dev` 和 `cargo check`）都需要这个编译目标：

```sh
rustup target add wasm32-unknown-unknown
```

官方模块约 2 MB，其中包括内嵌的地图和战役数据。即使在开发构建中，Cranelift 和 Wasmtime 的编译器包也会优化编译（见工作区 `Cargo.toml`），因为未优化的 Cranelift 编译插件要慢很多倍。

## 测试

- `crates/plugins/nato-official/tests/rules.rs` 和 `campaign_scenarios.rs`：NATO 规则测试集，全部经过内核与 WebAssembly 插件运行。`tests/common/mod.rs` 中的测试辅助模块通过带类型的 NATO 模型读取状态，并用 `GameEngine::edit_state` 修改状态；由内核控制开关的 `debug.checkSupply` 命令可按需执行补给检查。
- `crates/ooaw-core/tests/kernel.rs`：路由、事务、只读查询、调试命令开关和直接编辑。
- `crates/ooaw-core/tests/plugins.rs`：受保护的第三方插件加载和单插件约束。
- `crates/ooaw-core/tests/engine_black_box.rs` 和 `campaign_scenarios.rs`：`ooaw-engine` JSON 进程。
- 变更比较（`ooaw-plugin-api::sdk::mirror`）和变更检查（`ooaw-core`）的单元测试。

## 性能

下表是在开发机上测得的 release 构建单次调用耗时，并与拆分前的原生引擎对比；测量时使用的是 ABI 版本 1 的 JSON 消息，尚未用 MessagePack 重新测量。

| 操作 | 原生引擎 | 插件，受信任沙箱 | 插件，受保护沙箱 |
| --- | --- | --- | --- |
| BALTAP `movementModes`（一个单位，所有移动方式） | 4.4 ms | 4.4 ms | 6.6 ms |
| 341 个单位的战役 `movementModes` | 10.0 ms | 9.9 ms | 14.5 ms |
| BALTAP `endPhase` | 0.9 ms | 1.0 ms | 1.3 ms |
| 341 个单位的战役 `endPhase` | 9.2 ms | 11.7 ms | 15.5 ms |

在受信任沙箱中，规则代码以原生速度运行。大场景中 `endPhase` 剩余的额外开销来自变更的上报与检查。

## 计划工作

- 在桌面应用中加载第三方替代插件（发现、选择、显示清单）。目前只有 `GameEngine::with_plugins` 能接收一个插件。
- 存档和回放必须记录插件版本、模块哈希以及骰子状态。
- 如果将来重新支持多插件，需要恢复跨插件过滤器路由和嵌套调用跟踪。
- 本局唯一插件处理所有非内核命令；阶段和查询仍使用具名处理器，目前还没有观察者钩子。
- 地图模型中的地形、城市和六角边类型是所有插件共享的封闭枚举。
- 预编译模块（`.cwasm`）可以省去启动时的编译。
- 内核目前无法显示插件自带的界面；前端当前只理解官方规则的状态和预览。
- 故障恢复路径（陷阱后替换实例）还没有自动化测试。
