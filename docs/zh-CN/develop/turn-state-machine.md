# 回合状态机开发记录

## 当前实现范围

已经完成：

- 独立的 `ooaw-core` Rust crate；
- 与具体战争无关的阵营 ID；
- 由场景数据定义的阶段顺序；
- `new_game(scenario_id)`；
- `list_scenarios()`；
- `get_game_snapshot()`；
- `submit_game_command(request)`；
- `EndPhase` 命令；
- 自动阶段连续处理；
- revision 检查；
- 阶段、回合和游戏结束事件；
- 通用单位、战力面和地图位置数据结构；
- 由场景数据定义的逐回合增援；
- 自动联合增援结算与 `reinforcementsArrived` 事件；
- 通过第一回合增援完成的最小开局部署；
- BALTAP 1983 的七回合框架、44 个单位、开局部署和逐回合增援表；
- 前端重绘算子所需的结构化单位和战力数据；
- 作为 `ScenarioDefinition` 一部分的权威地图模型与 NATO 地图数据；
- `new_game` 创建游戏时一次性下发地图，普通快照只保留稳定 `mapId`；
- 前端提交 `EndPhase`、消费返回事件和权威快照的完整命令链路；
- 开局增援结算后的地图单位、战略预备队数量和程序化自绘算子显示；
- 单位 HQ、移动和战斗补给状态；
- 自动 `preBattle` 结算与 `preBattleSupplyChecked` 事件；
- 可自由交错提交的战斗计划命令：选择再补给对象、设置攻击目标、战术/行军移动、命令装车/下车、铁路移动和空运；
- 内核权威寻路与移动校验：地形与格边消耗、移动力、禁行地形、敌方控制区、最低移动、丹麦渡轮、敌占格与堆叠限制，以及只读的移动预览；
- 剧本规则数据中的铁路距离（20 格）、铁路容量（华约 8 步/北约 10 步，只计已装车单位）、各方空运司令部数量（BALTAP：华约 3、北约 1）、每回合一次再补给和 4 个机动步堆叠上限；
- 战斗计划路线与攻击目标的前端地图叠加显示；
- 七回合与十四回合状态机测试。

尚未实现：

- 完整单位序列与尚未进入地图数据的道路、河流通行修正；
- 基于地图控制、国家、城市、敌方控制区、HQ 支援范围和阻断边的动态补给线判定；
- 空域控制、拦截、空运损失与升降机司令部完整编制；
- 打击和地面战斗；
- 预备队移动；
- `postBattle` 的具体自动结算；
- 其余正式剧本的部署与胜利条件；
- BALTAP 的特殊规则和胜利条件；

## 已确认的设计方向

计划中的玩家回合为：

```text
Pre-battle
→ Battle planning
→ Offensive strike
→ Combat
→ Reserve
→ Post-battle
```

- `Pre-battle` 是自动阶段，负责在玩家操作前结算 HQ 补给和移动补给状态，不要求玩家输入。
- 再补给行动、铁路/空海/地面/直升机移动、预备队指定和移动后的恢复处理将并入 `Battle planning`。
- 原恢复阶段中的再补给用途选择也将并入 `Battle planning`。
- `Battle planning` 将根据具体规则集决定计划是否具有强制性。
- `Combat` 保持独立，因为打击负责远程火力，战斗负责地面接敌、撤退、占领和推进。
- `Post-battle` 将承担解除压制等自动清理。

阵营、行动顺序、阶段是否存在以及阶段属于哪一方，都由场景或规则集定义，不能写死在状态机引擎中。

当前原型让双方都经过 `battlePlanning`，以便以后容纳双方的再补给选择。具体规则处理器可以进一步区分强制计划与非强制计划。

`preBattle` 的第一层实现会为当前行动方的单位生成补给检查记录。增援到达时即以完全补给状态进入游戏；战略预备队内的 HQ 或战斗单位在每次检查时恢复为其适用补给类型的有补给状态。地图上的单位暂时保留已有的权威补给状态。完整 LOS 计算要等地图国家归属、城市控制、敌方控制区、HQ 支援范围和补给线阻断数据进入内核后再替换这一保留逻辑。

`battlePlanning` 现在是行动方的统一命令阶段。选择再补给对象、添加或移除攻击目标、移动其他单位、命令装车、下车或空运之间没有人为规定的提交顺序；每条命令只校验其实际规则前置条件。移动会立即更新权威位置并保存内核选择的路线，攻击目标和再补给选择保存在 `battlePlan` 中。结束阶段时，对再补给目标所在格的己方战斗单位统一恢复适用补给状态，计划则保留给后续打击与战斗阶段使用。

地面移动支持战术移动和行军移动；玩家规则见 `docs/zh-CN/rules/movement.md`，实现说明见下文“移动规则”。铁路装车要占用当前整个计划阶段，下一次该方战斗计划开始时才转为 `entrained`；已装车单位每回合最多移动 20 格。基础空运只允许带 `airTransportable` 特性的空降/空中机动单位，从城市或战略预备队移至非海洋、非山地、非敌方控制区的合法格。海运暂不实现。

### 移动规则

所有移动规则位于 `src/movement.rs`。`moveUnit` 与只读查询 `movement_options` 共用同一套搜索和目的地校验，因此预览中的每个目的地都能作为命令被接受。前端不实现任何移动规则：单位可用哪些移动方式、其他方式为何不可用，以及每个合法目的地，都来自 `GameState::movement_modes`。括号中的编号对应 2020 版规则书（`internet/NATO_Rules_2020.pdf`）。

已实现：

- 按主要地形计算地形消耗，城市优先于其下方地形（2.2.1）；城市消耗 1（12.3 示例）；
- 大河格边 +1（12.1 示例）；
- 禁行地形：全海格、没有堤道的全海格边、阻断格边、敌占格（12.1.1、12.1.2）；
- 控制区投射：攻击力 1+ 的单位和 HQ 投射到相邻六格，攻击力 0 只投射到自身格，铁路标记下的单位不投射，控制区可越过阻断格边和全海格边（8.0、13.4.1）；
- 战术移动进入 +1、离开 +1 的控制区消耗，软目标单位停止规则（拆分命令时同样生效），软目标单位只能在目标格已有己方单位时从敌控格直接进入敌控格（12.3、12.3.1、12.3.2）；
- 行军：移动力翻倍；HQ 和缺乏移动补给的单位不可行军；不能从敌控格出发、进入或停在敌控格（12.4、8.1）；
- 尚未移动的单位可进行最低移动，按直接相邻的单格边计算（12.5）；
- 丹麦渡轮 1513/1514：每阶段每个方向一个 NATO 单位，视为最低移动（12.8）；
- 铁路：20 格，不能穿越禁行地形，不能离开或进入敌控格（13.2）；
- 堆叠：每条命令结束时四个机动单位战力面加一个 HQ（9.1.1）；
- 城市（30）：地面移动永远不能进入敌方自由城市；只有战术移动可以进入敌方被征服城市，并取得途经的每个此类城市的控制权（若移动方是原始拥有方则解放该城市）；行军和铁路避开敌方控制的城市；空运不能停在敌方控制的城市；己方自由城市与己方单位一样，为软目标单位进入而抵消敌方控制区（30.1.1、30.2.1–30.2.3、30.3.1、30.3.2）。

未验证或未实现：

- 地形效果表（TEC）印在行动顺序卡背面，`internet/` 中没有该卡：VASSAL 模组（`internet/NATO_PZG_v2_4_1/`，与 `~/Downloads` 中的副本相同）、2020 版规则书以及 BoardGameGeek 上的在线更新规则书和剧本手册（`internet/NATO_Living_Rules_Booklet_1-1-26.pdf`、`internet/NATO_Play_Booklet_Updated_1-1-26.pdf`）中都没有。地图上的地形图例只给出地形优先级（关键/大型城市 1、小城市 2、山地 3、崎岖地 4、森林 5、沼泽 6、开阔地 7）。沼泽消耗 1：规则 25.8.2 的设计者注释说明沼泽与开阔地除阻止扩张外没有区别。崎岖地、山地的消耗（2/3）、大型城市和关键城市消耗 1、小河格边消耗 0（`BattlePlanningRules::minor_river_cost`）仍为占位值；山口格边尚未建模。
- 尚未实现空域（行军和铁路需在己方空域内）、本土防卫单位不得离开本国（12.7）、OMG/预备队标记（12.6）、空中遮断区、混乱/压制标记、山口格边和难民规则。
- 由于未模拟丹麦投降，华约不能使用丹麦渡轮。
- BALTAP 游戏区域（剧本手册 36.4.1.2）：任何单位不得移动到易北河以南，也不得在那里追踪补给线。尚未实现：描绘出的河流格边不构成闭合边界，需要依据地图手工定义被排除的格子。

已应用的在线更新规则（2026 年 1 月 1 日）勘误：铁路运力只计算已装车单位（13.3）；装车中的单位在战斗计划开始时，仅在运力允许时按单位 ID 顺序转为已装车（尚未提供由玩家选择哪些单位转换）；空运不能从敌控格出发（16.1.1）；BALTAP 空运司令部为华约 3 个、NATO 1 个，每个每回合运载一个战力面（3.8、36.2）。其他 1-1-26 变更（12.6 的 OMG/预备队资格、25.1.1.1 的华约战斗投入）涉及尚未实现的系统。
- 堆叠在每条命令结束时检查，而不是只在阶段结束时检查（9.1.1 允许途经超堆叠）。

格边数据：`crates/ooaw-core/data/natoMap.json` 中的 `allSea`、`causeway`、`majorRiver`、`minorRiver` 和 `danishFerry` 由本地未提交的工具 `tmp/map-extract/water_hexsides.py` 生成。当海水颜色覆盖格边两侧带状区域至少 85% 的长度且没有描绘出的河流沿该格边时，判定为全海格边；河流类型取自已描绘的河流折线；渡轮按规则 12.8 设置。目前只描绘出一条堤道（小贝尔特桥），它穿过的并不是全海格边；阿夫鲁戴克大堤和其他堤道尚未描绘。

同一计划内的每条计划命令都可以撤销：`setResupplyTarget` 传入 `unitId: null` 清除再补给选择，`setAttackTarget` 传入 `selected: false` 移除攻击目标，`undoUnitMovement` 撤销单位最近一段移动，`detrainUnit` 取消本计划中下达的装车命令，`undoDetrainUnit` 恢复本计划中下车单位的已装车标记。如果该单位此后进行了非铁路移动，或铁路运力已不足，`undoDetrainUnit` 会被拒绝。

完整游戏回合仍使用同一个扁平顺序表，不引入第二个状态机：

```text
jointStatus
→ jointReinforcement
→ warsawPact.preBattle ... warsawPact.postBattle
→ nato.preBattle ... nato.postBattle
```

联合阶段的 actor 为 `all`，之后的阶段 actor 为对应阵营。

`jointReinforcement` 是自动阶段。进入该阶段时，内核从场景的增援表中取出当前回合的单位，将其加入权威状态，并返回包含完整单位状态的 `reinforcementsArrived` 事件。第一回合的条目使用同一机制表示开局部署，不另设一套初始化逻辑。

## 内核模块结构

游戏状态机按职责拆分，同时由 crate 根统一导出公共类型：

```text
src/model/
├─ mod.rs               模型组织与公共导出
├─ map.rs               地图、地形、城市、网格和六角边模型
├─ planning.rs          战斗计划、攻击目标与移动记录
├─ side.rs              阵营标识与定义
├─ phase.rs             阶段标识、执行方式与行动方
├─ rules.rs             移动方式、装车状态与剧本规则参数
├─ scenario.rs          场景模型、注册表和公共构造逻辑
├─ scenario_baltap.rs   BALTAP 场景内容
└─ unit.rs              单位标识、定义、战力面与位置

src/
├─ state.rs     GameState、快照和回合状态
├─ command.rs   游戏命令和命令结果
├─ event.rs     领域事件
├─ error.rs     规则错误
├─ engine.rs    命令执行与阶段推进
├─ phase.rs     阶段入口处理器
├─ planning.rs  战斗计划命令与计划记录
├─ movement.rs  移动规则、寻路与移动预览
└─ tests.rs     状态机测试
```

`src/model/` 保存游戏内容的数据模型；crate 根目录保存命令执行和状态机行为。`src/model/map.rs` 定义可序列化的地图结构，并从 `crates/ooaw-core/data/natoMap.json` 加载内嵌 NATO 地图。`src/model/unit.rs` 定义稳定单位 ID、国籍、单位类型、编制、战力面、地图位置、运行时单位状态，以及 HQ、移动和战斗补给状态。`src/planning.rs` 实现战斗计划命令，`src/movement.rs` 实现权威移动规则与寻路；其余尚未实现的阶段仍在根目录的 `phase.rs` 中保留空处理器。

`src/model/scenario_baltap.rs` 保存 BALTAP 1983 的场景数据。场景共有 44 个单位：第一回合 27 个，第二至第六回合分别为 11、2、2、1、1 个。第一回合单位中，17 个进入地图格，10 个进入战略预备队。后续增援进入战略预备队。

算子不依赖图片资源。内核通过单位定义和每个战力面的 `attack`、`defense`、`movement` 等结构化字段提供游戏信息，前端按自己的视觉风格绘制算子。

前端通过 `src/gameApi.ts` 调用 `new_game` 和 `submit_game_command`。开局时地图上没有单位；玩家在联合状态阶段选择结束阶段后，内核自动完成联合增援和战前结算。前端使用返回快照重绘单位层和战斗计划叠加层，并将镜头聚焦到新到达的单位。算子由 PixiJS 的矩形、线条、椭圆和文字现场绘制，不加载原作算子图片。

模型类型和场景查询可以通过 `ooaw_core::model::{...}` 导入。crate 根继续重新导出公共项，因此现有的 `ooaw_core::{...}` 调用保持兼容。

## 当前 IPC

创建游戏：

```text
new_game("nato-1983-standard")
```

`new_game` 返回启动数据：

```json
{
  "snapshot": { "protocolVersion": 10, "scenario": { "mapId": "nato-central-europe" }, "battlePlan": null },
  "map": {
    "id": "nato-central-europe",
    "version": 1,
    "grid": {},
    "hexes": [],
    "hexsides": []
  }
}
```

示例省略了地图的完整网格、地形和绘制数据。地图只在创建游戏时随启动响应下发；后续命令返回的快照不重复携带地图。

列出可用场景：

```text
list_scenarios()
```

BALTAP 1983 的场景 ID 为：

```text
new_game("nato-baltap-1983")
```

当前快照示例：

```json
{
  "protocolVersion": 10,
  "gameId": "generated-uuid",
  "revision": 0,
  "scenario": {
    "id": "nato-1983-standard",
    "name": "NATO 1983 Rules Prototype",
    "maxGameTurns": 14,
    "mapId": "nato-central-europe",
    "sides": [
      { "id": "warsawPact", "name": "Warsaw Pact" },
      { "id": "nato", "name": "NATO" }
    ]
  },
  "status": "inProgress",
  "turn": {
    "gameTurn": 1,
    "stepIndex": 0,
    "currentStep": {
      "id": "joint.jointStatus",
      "phaseId": "jointStatus",
      "actor": { "type": "all" },
      "execution": "interactive"
    }
  },
  "units": [],
  "pendingDecision": null
}
```

当前命令请求：

```json
{
  "expectedRevision": 0,
  "command": { "type": "endPhase" }
}
```

过期的 revision 返回 `revisionMismatch`。合法命令返回事件和新的完整快照。

只读计划预览（不会改变状态或 revision）：

```text
movement_options({ "request": { "unitId": "soviet.2gta.21motorRifleDivision" } })
```

```json
{ "revision": 3, "unitId": "soviet.2gta.21motorRifleDivision",
  "modes": [
    { "mode": "tactical", "unavailable": null,
      "options": [{ "hexId": "2411", "cost": 1, "path": ["2411"] }] },
    { "mode": "rail", "options": [],
      "unavailable": { "code": "unitNotEntrained", "message": "A unit must finish entraining before it can move by rail" } }
  ] }
```

```text
attack_target_options()  →  { "revision": 3, "hexIds": ["2214", "2415"] }
```

`attack_target_options` 列出有敌方单位或属于敌方自由城市的格子（25.1.1）。快照的 `cities` 数组为每个城市格给出 `{ hexId, owner, controller, free }`。占领或解放城市的移动及其撤销会附带 `CityControlChanged` 事件；每条 `PlannedMovement` 记录其 `cityControlChanges`。

命令的变体名和字段名都使用 camelCase。战斗计划命令：

```json
{ "type": "setResupplyTarget", "unitId": "soviet.2gta.21motorRifleDivision" }
{ "type": "setAttackTarget", "hexId": "2415", "selected": true }
{ "type": "moveUnit", "unitId": "soviet.2gta.21motorRifleDivision", "destination": "2411", "mode": "tactical" }
{ "type": "undoUnitMovement", "unitId": "soviet.2gta.21motorRifleDivision" }
{ "type": "entrainUnit", "unitId": "soviet.2gta.21motorRifleDivision" }
{ "type": "detrainUnit", "unitId": "soviet.2gta.21motorRifleDivision" }
{ "type": "undoDetrainUnit", "unitId": "soviet.2gta.21motorRifleDivision" }
```

## 桌面界面

右侧控制面板是主视图。未选中单位时，它显示“前往格子”、所选格的地形、指挥区、城市/港口数据、格边特征和格内单位；在战斗计划阶段，还会列出行动方的战略预备队，并提供一个切换按钮，把所选的敌占格加入攻击目标或从中移除。选择单位（从列表中选择，或在地图上点击其算子）后，面板切换到单位详情视图，显示绘制的算子、身份、战力数值、补给、铁路状态、特性，以及计划方自有单位可用的计划行动。任何行动按钮在其命令进入计划后都会变成撤销按钮；该状态由前端从权威的 `battlePlan` 推导，而不是依赖本地记录。左键点击算子会选中该单位，左键点击其他位置会让面板回到格子视图。

移动在地图上下达。选中计划方自有单位时，面板显示移动方式选择器（战术、行军、铁路、空运）；单位移动后，其他方式会被禁用。前端针对所选单位调用一次 `movement_options`，并在每条命令被接受后重新调用，绝不按指针移动逐帧调用。移动方式按钮的启用状态和说明只取自该响应；攻击目标按钮只对 `attack_target_options` 中的格子启用；格子视图显示每个城市的控制方以及它是自由城市还是被征服城市。悬停在列出的目的地上时，会沿内核选择的路线绘制红色箭头；右键点击即提交 `moveUnit`。未列出的格子不显示箭头，也忽略右键点击。移动后所选单位保持选中，便于继续移动或撤销。

显示选项（镜头缩放、适配地图和地图图层开关）位于右上角“设置”按钮打开的模态设置窗口中。底栏可折叠，并分为两个只读区域：左侧“战斗计划”实时渲染 `snapshot.battlePlan`，右侧“战斗日志”在战斗结算实现之前保持为空。

结束第一回合的 `jointStatus` 后，状态机会进入并自动完成 `jointReinforcement`。返回结果中的事件流包含：

```json
{
  "type": "reinforcementsArrived",
  "gameTurn": 1,
  "units": [
    {
      "id": "soviet.6thGuardsMotorRifleDivision",
      "name": "6th Guards Motor Rifle Division",
      "sideId": "warsawPact",
      "nationId": "sovietUnion",
      "unitTypeId": "motorRifleDivision",
      "formationId": null,
      "traits": [],
      "steps": [
        {
          "attack": 8,
          "defense": 6,
          "movement": 5
        },
        {
          "attack": 4,
          "defense": 4,
          "movement": 5
        }
      ],
      "strengthStepIndex": 0,
      "location": { "type": "hex", "hexId": "2806" },
      "supply": {
        "headquarters": null,
        "movement": "supplied",
        "combat": "supplied"
      }
    }
  ]
}
```

实际事件同时包含该回合到达的全部单位；示例只展示一个单位。返回的完整快照也会在 `units` 中包含目前已经进入游戏的所有单位。

联合增援之后，`preBattle` 自动结算并发出：

```json
{
  "type": "preBattleSupplyChecked",
  "gameTurn": 1,
  "sideId": "warsawPact",
  "units": [
    {
      "unitId": "soviet.6thGuardsMotorRifleDivision",
      "supply": {
        "headquarters": null,
        "movement": "supplied",
        "combat": "supplied"
      }
    }
  ]
}
```

随后 `preBattle` 自动结束，当前阶段直接变为该阵营的 `battlePlanning`。
