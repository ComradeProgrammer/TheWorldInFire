# 三个战役剧本的年份版本

后端注册了以下六个选项，`list_scenarios()` 与 `new_game(scenario_id)` 共用注册表。前端直接读取列表并传回 ID，不需要自行维护剧本名称或部署表。既有 BALTAP 1983 和规则原型仍保留，共八个注册条目。

| ID | 第一战争回合单位数 | 十四回合内单位总数 |
| --- | ---: | ---: |
| `nato-strategic-surprise-1983` | 199 | 287 |
| `nato-strategic-surprise-1988` | 202 | 286 |
| `nato-extended-buildup-1983` | 341 | 362 |
| `nato-extended-buildup-1988` | 339 | 360 |
| `nato-war-of-nerves-1983` | 199 | 287 |
| `nato-war-of-nerves-1988` | 202 | 286 |

## 数据与已实现行为

`crates/plugins/nato-official/data/natoCampaigns.json` 是构建时内嵌的结构化数据。单位的攻击、防御、移动、战力面、兵种、国籍和隶属取自 2026 年 1 月 1 日在线更新剧本手册 §44 的战斗序列；开局位置和增援取自 VASSAL 2.4.1 对应的六个标准预设，不包括 Alternate NATO 版本。西德 9/3Pz 的位置按手册 §37.3/40.3 勘误改为 2716。

`tools/import_nato_campaigns.py` 可从本地 `internet/` 参考资料重新生成数据，需要 Python 与 pypdf。它只读输入；运行时、构建和测试均不依赖 `internet/`。1988 年的苏军 6G 与 90GT 按 §44.2 交换所属前线；英军升级、加拿大增援和美国 81 旅的年份变化使用各年份的实际数据。剧本文本和模组中的 1988 年苏军 83 空中突击旅也已纳入。

六个选项使用现有十四战争回合流程和逐回合增援机制。Strategic Surprise、War of Nerves 的空运司令部为华约三个、北约两个；Extended Buildup 为华约四个、北约三个。Strategic Surprise 第一回合有奇袭；War of Nerves 因警戒级别尚未建模，不默认给予奇袭。可用前线 HQ 和美国 V、VII 军 HQ 提供进攻支援；波罗的海前线和美国 III 军 HQ 仅在 Extended Buildup 中提供支援。

**筑垒格（38.4.3.4）。** `natoCampaigns.json` 中列出 `fortifiedHexes`，只有 Extended Buildup 会把它复制到 `ScenarioDefinition::fortified_hexes`。该列表共 44 格：与铁幕格边相邻的 43 个西德格，加上西柏林（3007）。地图数据没有铁幕格边，描摹的 `ironCurtain` 折线也只覆盖了部分边境。因此 85 条铁幕格边是从印刷地图图像中读取的：依据铁幕线沿格边的橙色描边判定；易北河一段则取橙色段之间的 `majorRiver` 格边。已校验这些格边构成一条从吕贝克湾到地图南缘、没有分叉的连续线，并取每条格边位于线西侧的格为西德格。当前列表作为 `fortifiedHexIds` 保存在规则状态中：`Rules::new` 从剧本初始化，联合状态阶段移除有华约单位的格，并发出 `fortifiedHexesConquered` 事件。效果只适用于华约针对北约的进攻和打击：
- 目标格的地形移栏改为 `ColumnShiftReason::Fortified`（−2）；
- 北约软目标防守单位以 `StrengthModifier::FortifiedHex` 加倍，取代 `SoftUnitCover`；
- 撤退减少 `max(1, 地形减少量)`；若北约单位撤退进入的第一格是筑垒格，可以在一格后停下；
- 打击的地形修正改为 −2，但目标处于铁路标记下时除外。

战果扩张尚未实现，因此“不得扩张”无需代码。War of Nerves 警戒等级 6 的筑垒（39.4.6、40.4.3.5）依赖尚未实现的警戒等级系统。核打击取消筑垒效果（21.1.3）同样尚未实现；征服标记除取消筑垒状态外没有其他效果。

## 与完整桌游剧本的差异

这些选项是现有内核上的战争阶段接入版本，不能视为完整桌游规则已实现：

- Strategic Surprise 的特殊战前 GT0、向 GDP 移动和逐步激活尚未实现；直接从第一战争回合开始。
- War of Nerves 的和平回合、紧张度、警戒级别、准备措施和动员时钟尚未实现。目前使用固定战争回合增援；因此，同年份的可用单位表与 Strategic Surprise 相同。超过十四回合的动员增援保留在数据的 `deferredUnits` 中，不提前加入现有游戏。
- Reforger 地点、接收 Reforger 战力面及转换尚未实现。按自定规则，Reforger 单位视为地面增援：除 Extended Buildup 外，开局即位于其 Reforger 集结点格。
- 增援框改为地图边缘的入场格（自定规则）。导入工具按战斗序列中每个后续增援的入场代码映射：`G#` 对应第 # 区段的入场格；`RS`/`RF` 对应印刷的集结点格；`RR` 按编制选择区段（捷克斯洛伐克和喀尔巴阡方面军为 3；白俄罗斯方面军、近卫坦克第 5 集团军和驻德集群各方面军为 4；波兰和波罗的海方面军为 5；英军为 1；法军为 2），并带 `"entrained": true`；`A`、`S`、`EB` 进入战略预备队（`hex: null`），之后通过空运或海运离开。
- Extended Buildup 的初始位置保留模组的集结预设，其中部分编制集中在同一集结格；原版的自由部署区域和部署阶段尚未实现。这些初始堆叠可能超过正常移动结束时的限制，目标格移动校验仍然生效。
- Extended Buildup 的可选 XVIII 军干预单位保留在 `deferredUnits`，不擅自选择某回合到达。
- 空中战役表仍使用现有原型空中点数。最大努力、核化学资源、完整自动补给、海运、直升机和各剧本胜利条件尚未实现。十四战争回合后仅结束游戏，不据原版胜利表判定胜负。

`campaign_scenarios.rs` 测试通过 JSON 进程接口列出并创建六个版本，检查年份数值与隶属差异、勘误、空运和支援配置，并运行全部十四回合验证开局、后续增援和结束行为。
