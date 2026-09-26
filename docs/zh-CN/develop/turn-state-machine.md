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
- 七回合与十四回合状态机测试。

尚未实现：

- 完整单位序列、地图规则和移动；
- 再补给选择；
- 战斗计划内容；
- 打击和地面战斗；
- 预备队移动；
- `preBattle` 和 `postBattle` 的具体自动结算；
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

- `Pre-battle` 将承载移动以及不需要玩家选择的恢复处理。
- 原恢复阶段中的再补给选择将并入 `Battle planning`。
- `Battle planning` 将根据具体规则集决定计划是否具有强制性。
- `Combat` 保持独立，因为打击负责远程火力，战斗负责地面接敌、撤退、占领和推进。
- `Post-battle` 将承担解除压制等自动清理。

阵营、行动顺序、阶段是否存在以及阶段属于哪一方，都由场景或规则集定义，不能写死在状态机引擎中。

当前原型让双方都经过 `battlePlanning`，以便以后容纳双方的再补给选择。具体规则处理器可以进一步区分强制计划与非强制计划。

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
├─ side.rs              阵营标识与定义
├─ phase.rs             阶段标识、执行方式与行动方
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
└─ tests.rs     状态机测试
```

`src/model/` 保存游戏内容的数据模型；crate 根目录保存命令执行和状态机行为。`src/model/unit.rs` 定义稳定单位 ID、国籍、单位类型、编制、战力面、地图位置和运行时单位状态。当前未实现的阶段在根目录的 `phase.rs` 中保留空处理器，之后可以逐项填充而不改变外层状态机。

`src/model/scenario_baltap.rs` 保存 BALTAP 1983 的场景数据。场景共有 44 个单位：第一回合 27 个，第二至第六回合分别为 11、2、2、1、1 个。第一回合单位中，17 个进入地图格，10 个进入战略预备队。后续增援进入战略预备队。

算子不依赖图片资源。内核通过单位定义和每个战力面的 `attack`、`defense`、`movement` 等结构化字段提供游戏信息，前端按自己的视觉风格绘制算子。

模型类型和场景查询可以通过 `ooaw_core::model::{...}` 导入。crate 根继续重新导出公共项，因此现有的 `ooaw_core::{...}` 调用保持兼容。

## 当前 IPC

创建游戏：

```text
new_game("nato-1983-standard")
```

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
  "protocolVersion": 4,
  "gameId": "generated-uuid",
  "revision": 0,
  "scenario": {
    "id": "nato-1983-standard",
    "name": "NATO 1983 Rules Prototype",
    "maxGameTurns": 14,
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
      "location": { "type": "hex", "hexId": "2806" }
    }
  ]
}
```

实际事件同时包含该回合到达的全部单位；示例只展示一个单位。返回的完整快照也会在 `units` 中包含目前已经进入游戏的所有单位。
