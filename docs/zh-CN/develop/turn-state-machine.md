# 回合状态机开发记录

## 当前实现范围

已经完成：

- 独立的 `ooaw-core` Rust crate；
- 与具体战争无关的阵营 ID；
- 由场景数据定义的阶段顺序；
- `new_game(scenario_id)`；
- `get_game_snapshot()`；
- `submit_game_command(request)`；
- `EndPhase` 命令；
- 自动阶段连续处理；
- revision 检查；
- 阶段、回合和游戏结束事件；
- 十四回合状态机测试。

尚未实现：

- 单位、地图和移动；
- 再补给选择；
- 战斗计划内容；
- 打击和地面战斗；
- 预备队移动；
- `preBattle` 和 `postBattle` 的具体自动结算；
- 正式场景部署与胜利条件。

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

## 内核模块结构

游戏状态机按职责拆分，同时由 crate 根统一导出公共类型：

```text
src/game/
├─ mod.rs       模块组织与公共导出
├─ state.rs     GameState、快照和回合状态
├─ command.rs   游戏命令和命令结果
├─ event.rs     领域事件
├─ error.rs     规则错误
├─ engine.rs    命令执行与阶段推进
└─ tests.rs     状态机测试
```

外部调用继续通过 `ooaw_core::{...}` 使用这些类型，不依赖内部文件布局。

## 当前 IPC

创建游戏：

```text
new_game("nato-1983-standard")
```

当前快照示例：

```json
{
  "protocolVersion": 1,
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
