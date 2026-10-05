# 空军算子系统重写方案

> 状态：第一版后端已实现，前端可按本文协议接入。旧空中点数字段暂时以空值保留，供前端迁移期间兼容。

## 1. 目标和范围

现有空军系统使用每回合刷新的 `AirPoints` 购买空袭或空中遮断任务。重写后，空中力量由有名称、有基地、有两级强度的空军算子组成。

第一版只保留三类空军单位：

- 战斗机（`fighter`）：向一个地图格出动，以该格为中心建立战斗空域，执行制空战斗和截击。
- 战斗轰炸机（`fighterBomber`）：向一个地图格或敌方地图外机场出动，执行现有 NATO 空中打击；被截击时可以用自身空战值自卫。
- 预警机（`aew`）：向一个地图格出动，为覆盖范围内的友军空战提供修正；第一版不能被攻击。

三类飞机均为两步单位。第一步损失后翻面并使用第二步数据，再损失一步即被消灭。

第一版明确不包括 ECM、SEAD、防空火力、空运飞机算子、飞机补充和机场修复。现有 Airlift Command 和空中运输移动仍作为独立的 NATO 地面移动规则保留。现有空中遮断任务先移出新系统，待战斗轰炸机基本流程稳定后再作为新任务类型加入。

## 2. 对现有代码的判断

当前实现有以下特点：

1. `RulesState` 保存 `airPoints`、当前 `strikePlan` 和 `airInterdictionZones`。
2. 每回合 Joint Reinforcement 重置航空点；Offensive Strike 开始时创建空任务计划，玩家在该阶段选择目标并主动结算。
3. 回合顺序已经是华约计划、北约计划、华约 Strike、北约 Strike，因此适合在两方计划完成后插入一次联合自动空战结算。
4. 当前 `Airspace` 由有补给的地面单位和城市向外投射五格，并参与行军、铁路、空运和海运校验。它不能直接替换成战斗机出动空域，否则计划阶段中已经执行的地面移动会被稍后发生的空战追溯性地判为非法。
5. 通用 `Unit` 不适合直接容纳飞机。地面规则普遍遍历全部 `Unit`，并假定它们参与补给、堆叠、ZOC、城市控制和地图移动；通用 `UnitLocation` 也只有地图格和战略预备队。

因此，新飞机使用 NATO 规则插件自己的独立状态类型。内核继续把这些状态当作插件拥有的 JSON 字段保存，无需修改 `ooaw-core` 或通用插件 API。

现有静态 `Airspace` 暂时改名为规则语义更明确的“地面作战空域”，继续服务地面移动和原 NATO 修正。新系统另建“战斗机覆盖区”，只负责空战和截击，两者不混用。

## 3. 建议的数据模型

### 3.1 标识符

在 NATO 插件内新增两个强类型标识符：

```rust
pub struct AirUnitId(pub String);
pub struct AirBaseId(pub String);
```

不要复用 `UnitId`。这样地面损失事件、移动命令和空军事件无法误接彼此的 ID。

### 3.2 空军单位

```rust
pub enum AirUnitKind {
    Fighter,
    FighterBomber,
    Aew,
}

pub struct AirUnitStep {
    pub air_combat: i8,
    pub evasion: i8,
    pub strike_modifier: i8,
    pub combat_radius: u8,
    pub aew_modifier: i8,
    pub aew_radius: u8,
}

pub struct AirUnitDefinition {
    pub id: AirUnitId,
    pub name: String,
    pub side_id: SideId,
    pub nation_id: NationId,
    pub kind: AirUnitKind,
    pub base_id: AirBaseId,
    pub steps: [AirUnitStep; 2],
}

pub enum AirReadiness {
    Ready,
    Aborted,
    Flown,
}

pub struct AirUnitState {
    #[serde(flatten)]
    pub definition: AirUnitDefinition,
    pub strength_step_index: usize,
    pub readiness: AirReadiness,
}
```

不同类型只使用相应字段，场景验证负责拒绝无效组合：战斗机必须有正数战斗空域半径；战斗轰炸机必须有打击能力；AEW 必须有支援半径和 AEW 修正。无关字段填零，使快照结构对 TypeScript 保持简单。

命名采用 1985 算子的风格：国家、部队番号、机型，例如显示名使用 `US 525th TFS — F-15C`，稳定 ID 使用 `us.525.f15c`。最终番号和数值从本地 1985 规则/VASSAL 资料整理进受版本控制的 JSON；游戏运行和 CI 不读取 `internet/`，也不复制原版算子图片。

### 3.3 地图外机场

```rust
pub struct AirBaseDefinition {
    pub id: AirBaseId,
    pub name: String,
    pub side_id: SideId,
    pub anchor_hex_id: HexId,
    pub sortie_capacity: u8,
    pub strike_modifier: i8,
}

pub struct AirBaseState {
    #[serde(flatten)]
    pub definition: AirBaseDefinition,
    pub damage: u8,
    pub suppressed_through_turn: Option<u16>,
}
```

机场本身不放在地图格中。`anchorHexId` 是机场对应的地图边缘接近点，只用于三件事：判断机场打击是否处于敌方战斗机覆盖区、在地图上绘制任务航线、为将来的航程规则保留入口。第一版不限制飞机从基地到目标的航程；战斗机的 `combatRadius` 只是抵达中心格后形成的覆盖半径。

机场有效出动容量为：

- `damage = 0`：完整 `sortieCapacity`；
- `damage = 1`：`ceil(sortieCapacity / 2)`；
- `damage >= 2`：机场关闭，容量为 0；
- 当前回合不晚于 `suppressedThroughTurn`：容量为 0。

第一版机场不会自动修复。已经升空的飞机不因同回合机场稍后被打击而受到影响；机场损伤从下一回合计划阶段起限制出动。

### 3.4 出动计划和结算记录

```rust
pub enum AirMission {
    AirSuperiority { center_hex_id: HexId },
    GroundStrike { hex_id: HexId, unit_ids: Vec<UnitId> },
    AirBaseStrike { air_base_id: AirBaseId },
    EarlyWarning { center_hex_id: HexId },
}

pub struct AirSortie {
    pub id: u32,
    pub air_unit_id: AirUnitId,
    pub mission: AirMission,
    pub status: AirSortieStatus,
}

pub struct AirPlan {
    pub game_turn: u16,
    pub side_id: SideId,
    pub sorties: Vec<AirSortie>,
    pub next_sortie_id: u32,
}
```

`RulesState` 新增：

```text
airUnits
airBases
airPlans
airOperationsReport
eliminatedAirUnitIds
```

`airOperationsReport` 保存本回合每轮制空配对、骰点、修正、原始表结果、步损和中止结果，供战报、动画、存档和重放使用。

## 4. 新回合顺序

建议把标准回合顺序明确写开，而不是继续用当前 `SIDE_PHASES` 双重循环生成：

```text
Joint Status（自动）
Joint Reinforcement（自动；重置飞机 Ready）
WP Pre-Battle（自动）
NATO Pre-Battle（自动）
WP Battle Planning（互动；地面命令和空军出动）
NATO Battle Planning（互动；地面命令和空军出动）
Joint Air Operations（自动；制空战斗和截击）
WP Offensive Strike（进入时自动结算；互动检查点）
NATO Offensive Strike（进入时自动结算；互动检查点）
WP/NATO Combat
WP/NATO Reserve
WP/NATO Post-Battle
```

这把“Strike 开始时立刻结算空战”具体化为：两方完成计划后、第一方 Offensive Strike 之前，统一运行一次 `jointAirOperations`。玩家在 Strike 阶段不再临时购买或改变任务。当前实现会在进入 Offensive Strike 时自动结算任务，但保留互动阶段作为结果检查点，由玩家结束阶段。

华约仍先结算地面/机场打击，北约后结算，与当前顺序一致。若前一个任务已经消灭后一个任务的全部指定目标，后一个任务记录为 `targetGone`，不重选目标也不掷骰。

## 5. 计划阶段规则

新增命令：

```json
{
  "type": "planAirSortie",
  "airUnitId": "us.525.f15c",
  "mission": { "type": "airSuperiority", "centerHexId": "3216" }
}
```

以及：

```json
{ "type": "cancelAirSortie", "sortieId": 3 }
```

后端每次重新验证：

1. 当前是该方 Battle Planning；
2. 飞机属于该方、尚存至少一步且状态为 `Ready`；
3. 一架飞机一回合最多出动一次；
4. 基地未关闭/压制且没有超过有效容量；
5. 任务类型与飞机类型匹配；
6. 中心格或目标存在；
7. 地面打击仍遵守现有规则：一个任务最多指定两个敌方单位步，HQ 单独作为目标；
8. 机场打击只能指定敌方机场。

第一版计划保存在共同快照中，适用于当前本地单机模式。将来若加入多人战争迷雾，应在服务器的按玩家视图层隐藏对方未结算计划，不应把隐藏逻辑写进规则结算。

## 6. 制空战斗

### 6.1 覆盖区和相交

战斗机出动到中心格 `C` 后覆盖所有满足下式的地图格：

```text
hexDistance(C, H) <= combatRadius
```

敌对两架战斗机的空域相交，当且仅当：

```text
hexDistance(centerA, centerB) <= radiusA + radiusB
```

使用现有已经验证过的六角格距离函数，边界格算在覆盖范围内。

### 6.2 多对多配对

不能让一架战斗机在同一轮对所有相交敌机各射击一次。每个空战轮按以下方式结算：

1. 用双方仍在场的战斗机和相交关系建立二分图；
2. 求确定性的最大匹配，使尽可能多的敌对战斗机成对交战；
3. 距离短者优先，同距离按双方 `AirUnitId` 排序，保证同一存档和种子产生同一顺序；
4. 每对同时射击，然后统一应用双方结果；
5. 中止和被消灭的飞机退出本回合空中行动；
6. 重新建立相交图并进入下一轮，直到没有敌对覆盖区相交。

设置 20 个空战轮的安全上限，防止异常表数据导致无限循环；达到上限时剩余飞机均视为脱离，并写入明确事件。正常 1985 表下几乎不会触发。

这种配对已经自然体现数量优势：未配对战斗机可以在下一轮接替或保留去截击，因此第一版不再额外叠加 1985 的数量优势骰点修正。

### 6.3 空战表

采用 1985 的 d20 空战表：

```text
column = clamp(attacker.airCombat - defender.evasion, -4, +4)
modifiedRoll = d20 + AEW modifier
```

双方同时掷骰。第一版只加入 AEW 修正，不加入飞行员、ECM 和额外数量优势修正。表的原始结果保留在战报中；在两步算子尺度上应用为：

| 1985 原始结果 | 第一版状态效果 |
| --- | --- |
| 无效果 | 留在空域 |
| `0-0-a` | 无步损，但中止并退出本回合 |
| `0-1-a` | 损失一步并中止 |
| `1-0-a` | 损失一步并中止 |
| `1-1-a` | 损失两步并中止；完整两步单位被消灭 |

虽然 `0-1-a` 和 `1-0-a` 在第一版都表现为翻面，它们仍作为不同原始结果保存，以便以后加入“受损可修、击毁不可修”的 1985 细分规则。

### 6.4 AEW

AEW 对满足以下条件的友军提供修正：

- 制空战斗：友军战斗机的任务中心在 AEW 支援半径内；
- 截击战斗：战斗机以自身任务中心判断，战斗轰炸机以其打击目标格或机场 `anchorHexId` 判断。

同一架飞机只取覆盖它的最高 AEW 修正，不叠加多个 AEW。初始建议值按 1985 设置为北约 `+2`、华约 `+1`，但数值存放在单位步骤数据中而不是写死阵营。AEW 第一版不参加配对、不拦截，也不能成为攻击目标。

## 7. 战斗机截击战斗轰炸机

所有制空战斗结束后再处理截击：

1. 仅仍未中止、未消灭的战斗机可截击；
2. 地面打击用目标格判断是否在敌方战斗机覆盖区内；机场打击用机场 `anchorHexId` 判断；
3. 每架战斗机本回合最多截击一架战斗轰炸机，每架战斗轰炸机最多被一架战斗机截击；
4. 对所有合法关系做确定性最大匹配，优先距离较短的战斗机，再按 ID 排序；
5. 截击只进行一个空战轮；战斗机和战斗轰炸机用同一空战表同时射击；
6. 战斗轰炸机被中止或消灭时，其打击任务取消；若未中止，即使翻面也继续执行任务；
7. 战斗机在制空战斗后只要仍在场，就可以继续执行一次截击。

让战斗轰炸机还击符合其单位类型，也使不同机型的 `airCombat` 和 `evasion` 有意义。若以后加入纯轰炸机，可把其空战值设为零或单独定义不能还击。

## 8. 打击结算

### 8.1 地面目标

幸存战斗轰炸机沿用当前 NATO d6 Strike Table、地形/城市修正、列车修正、惊袭修正、单位 disruption/step loss 和 Breakthrough Marker 处理。

在现有修正上再加入当前飞机步骤的 `strikeModifier`。第一批数据可以全部设为 0，先验证流程，再根据机型平衡。当前地面作战 `Airspace` 的 `+1/0/-1` 修正第一版保留，以避免同时改动地面移动和原 NATO 平衡；经过试玩后可单独决定是否删除。

### 8.2 地图外机场

机场打击使用同一 d6 Strike Table：

| 结果 | 机场效果 |
| --- | --- |
| `NoEffect` | 无效果 |
| `Disrupted` | 设置 `suppressedThroughTurn = currentTurn + 1`，下一回合不能出动 |
| `StepLoss` | `damage += 1`，并同样压制到下一回合；两点损伤关闭机场 |

机场自己的 `strikeModifier` 参与骰点，用于表达分散、加固或易受攻击程度。机场打击不会直接杀伤驻场飞机。机场修复和转场以后分别增加命令，不混入第一版。

## 9. 命令、查询和事件协议

删除或停用旧命令：

```text
planAirStrike
planAirInterdiction
cancelAirMission
resolveAirStrikes
```

新增命令：

```text
planAirSortie
cancelAirSortie
```

新增查询：

- `airPlanningOptions`：当前计划方每架飞机是否可出动及不可用原因、基地剩余容量；
- `airMissionOptions { airUnitId }`：该飞机允许的任务类型、目标格/机场和可选地面目标；
- `fighterCoverage { sideId }`：已公开计划或结算后的覆盖格，用于地图叠层。

核心新增事件：

```text
airUnitsReadied
airSortiePlanned
airSortieCancelled
airCombatRoundResolved
airUnitStepLost
airUnitAborted
airUnitEliminated
airInterceptionResolved
airStrikeResolved
airStrikeAborted
airBaseSuppressed
airBaseDamaged
```

每个随机事件都必须带自然骰、修正、修正后骰点、表列和原始结果。前端只负责动画和展示，不自行重算结果。

建议前端首先只依赖以下稳定边界：`AirUnitState`、`AirBaseState`、`AirPlan`、四种 `AirMission`、两个命令、三个查询和上述事件名称。后端开始实现前应先把这些 JSON 形状与前端 agent 固定下来。

## 10. 文件拆分

建议新增：

```text
plugins/nato-official/src/model/air.rs
plugins/nato-official/src/air_operations.rs
plugins/nato-official/src/air_strikes.rs
plugins/nato-official/data/natoAirForces.json
plugins/nato-official/tests/air_operations.rs
```

需要修改：

```text
plugins/nato-official/src/model/mod.rs
plugins/nato-official/src/model/scenario.rs
plugins/nato-official/src/model/scenario_campaign.rs
plugins/nato-official/src/model/rules.rs
plugins/nato-official/src/rules.rs
plugins/nato-official/src/phase.rs
plugins/nato-official/src/command.rs
plugins/nato-official/src/event.rs
plugins/nato-official/src/plugin.rs
plugins/nato-official/src/setup.rs
src/gameApi.ts
```

当前 `strikes.rs` 还混有地面单位 disruption、step loss 和阶段恢复等通用逻辑。删除旧空袭代码前，应先把这些通用函数移到 `casualties.rs` 或相应地面战斗模块，避免空军重写意外破坏地面战斗。

## 11. 分阶段实施

### 阶段 A：固定协议和场景骨架

1. 新增空军/机场/计划类型和 `serde(default)` 状态字段；
2. 新增场景空军数据文件和验证，但先只给测试场景少量单位；
3. 与前端 agent 固定 JSON 协议；
4. 保留旧航空点代码，确保每次提交都能编译和运行。

验收：新状态可创建、序列化、同步和重载；旧玩法尚未改变。

### 阶段 B：计划和阶段机

1. 加入出动/取消命令和查询；
2. 校验基地容量、归属、单位状态和任务目标；
3. 在两方 Battle Planning 后插入自动 `jointAirOperations`；
4. 暂时让该阶段只记录计划，不掷骰。

验收：完整回合可以通过，所有出动在计划结束后锁定。

### 阶段 C：制空和 AEW

1. 实现覆盖区、相交图和确定性最大匹配；
2. 为插件骰子增加 `d20()`；
3. 实现同时射击、两步损失、中止、重复轮次和 AEW；
4. 输出完整事件和战报。

验收：相同游戏 ID 和命令序列产生完全相同的配对、骰点和结果。

### 阶段 D：截击和打击

1. 实现战斗机对战斗轰炸机的一次截击匹配；
2. 把现有 NATO 地面打击函数改为接受 `AirUnitId` 任务；
3. 加入机场目标及损伤/压制；
4. 进入两方 Offensive Strike 时自动结算，并保留互动结果检查点。

验收：被中止的战斗轰炸机不打击；未中止任务准确应用地面或机场结果。

### 阶段 E：场景数据和旧系统删除

1. 从 1985 资料整理 1983/1988 所需的命名、机型和初始数值；
2. 为 Strategic Surprise、Extended Buildup、War of Nerves 的 1983/1988 版本分配空军和基地；
3. 为 BALTAP 和 prototype 场景提供明确的最小空军编制或显式声明无空军；
4. 删除 `AirPoints`、旧 `StrikePlan`、旧命令、旧查询和旧 UI 契约；
5. 更新中英文玩家规则和开发文档。

验收：所有内建场景无需 `internet/` 即可启动；快照中不再出现旧航空点状态。

## 12. 必须覆盖的自动测试

至少包含以下规则测试：

1. 两格空域刚好接触时会交战，超出一格时不会；
2. 多对多相交使用稳定最大匹配，一架飞机同一轮不会射击两次；
3. 同时射击允许双方同轮互相消灭；
4. 一步损失翻面，两步损失消灭；中止结果不产生额外步损；
5. AEW 在范围内生效、范围外不生效，多个 AEW 不叠加；
6. 制空幸存战斗机可截击，每架战斗机和战斗轰炸机最多配对一次；
7. 被中止的战斗轰炸机不会进入 Strike；
8. 地面打击继续满足现有两步目标、HQ、地形、惊袭、突破标记规则；
9. 机场满状态、半容量、关闭和压制状态正确限制下一回合出动；
10. 同回合已经出动的飞机不受后来机场损伤影响；
11. 存档同步和失败命令回滚不会改变骰子或留下半个空战结果；
12. 原有地面 Airspace、行军、铁路、空运、海运和地面战斗测试全部继续通过。

## 13. 第一版完成定义

第一版完成时，玩家能够在 Battle Planning 中看到有名称的两步飞机和所属地图外机场，给每架可用飞机下达一个合法任务；两方计划完成后，游戏自动且可重放地解决战斗机交战、AEW 修正和战斗机截击；幸存战斗轰炸机随后自动使用 NATO Strike Table 打击地面单位或机场；所有损失、中止、机场状态和任务结果都进入权威快照与事件流，前端不承担任何规则判断。
