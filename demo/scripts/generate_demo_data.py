"""
Generate synthetic demo data for Agent World Demo site.

Produces 6 JSON files in demo/public/data/:
  - world-snapshots.json  (~51 snapshots, every 100 ticks)
  - agents.json           (50 agents with full profiles)
  - emergence-metrics.json (time-series metrics every 10 ticks)
  - timeline-events.json  (>=20 key emergence events)
  - organizations.json    (5-10 organizations)
  - interaction-network.json (agent interaction graph)

Data models a three-phase "civilization emergence" across 5000 ticks:
  Early  (0-500):    Exploration, individual behavior, no orgs, low trade
  Mid    (500-2000):  Org formation, trade increase, cultural differentiation
  Late   (2000-5000): Complex social structures, governance, economic cycles
"""

from __future__ import annotations

import json
import math
import os
import random
import uuid
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

# ---------------------------------------------------------------------------
# Configuration
# ---------------------------------------------------------------------------

SEED = 42
TOTAL_TICKS = 5000
NUM_AGENTS = 50
SNAPSHOT_INTERVAL = 100  # every 100 ticks => 51 snapshots
METRICS_INTERVAL = 10    # every 10 ticks => 500 data points per metric

SCRIPT_DIR = Path(__file__).resolve().parent
PROJECT_ROOT = SCRIPT_DIR.parent.parent
OUTPUT_DIR = PROJECT_ROOT / "demo" / "public" / "data"

# Chinese names pool
FIRST_NAMES = [
    "明", "华", "强", "伟", "芳", "娜", "秀英", "敏", "静", "丽",
    "军", "洋", "勇", "艳", "杰", "涛", "超", "慧", "鑫", "磊",
    "雪", "博", "宇", "浩", "婷", "欣", "琳", "飞", "龙", "瑶",
    "晨", "睿", "瑶", "凯", "峰", "玲", "辉", "鹏", "洁", "翔",
    "梦", "昊", "岚", "蕊", "航", "宁", "悦", "昊", "凡", "彤",
]

LAST_NAMES = [
    "张", "王", "李", "赵", "刘", "陈", "杨", "黄", "周", "吴",
    "徐", "孙", "马", "朱", "胡", "郭", "林", "何", "高", "罗",
    "郑", "梁", "谢", "宋", "唐", "韩", "曹", "许", "邓", "萧",
    "冯", "程", "蔡", "彭", "潘", "袁", "于", "董", "余", "苏",
    "叶", "吕", "魏", "蒋", "田", "杜", "丁", "沈", "姜", "范",
]

SKILL_NAMES = [
    "farming", "mining", "crafting", "trading", "building",
    "healing", "teaching", "guarding", "researching", "cooking",
    "fishing", "hunting", "weaving", "smelting", "enchanting",
]

PERSONALITY_DIMENSIONS = ["openness", "conscientiousness", "extraversion", "agreeableness", "neuroticism"]

VALUE_POOL = [
    "自由", "平等", "秩序", "创新", "和谐", "勇气", "智慧", "忠诚",
    "繁荣", "团结", "公正", "博爱", "坚韧", "诚实", "仁慈",
]

ORG_TYPES = ["company", "guild", "alliance", "university"]

ORG_TEMPLATES = [
    {"name": "星辰商会", "type": "company", "founded_tick": 320, "theme": "贸易"},
    {"name": "铁壁联盟", "type": "alliance", "founded_tick": 580, "theme": "防御"},
    {"name": "匠人工会", "type": "guild", "founded_tick": 720, "theme": "制造"},
    {"name": "智慧学院", "type": "university", "founded_tick": 950, "theme": "研究"},
    {"name": "丰收行会", "type": "guild", "founded_tick": 1100, "theme": "农业"},
    {"name": "远航商会", "type": "company", "founded_tick": 1450, "theme": "探险"},
    {"name": "光明联盟", "type": "alliance", "founded_tick": 1800, "theme": "治理"},
    {"name": "灵匠学院", "type": "university", "founded_tick": 2200, "theme": "附魔"},
    {"name": "铜钱会", "type": "company", "founded_tick": 2800, "theme": "金融"},
    {"name": "守护者联盟", "type": "alliance", "founded_tick": 3500, "theme": "治安"},
]

EVENT_TEMPLATES = [
    {"tick": 45,   "type": "milestone", "title": "第一次交易", "desc": "{agent_a}与{agent_b}完成了世界上第一次物物交换，标志着经济活动的开始。"},
    {"tick": 180,  "type": "milestone", "title": "首位技能大师", "desc": "{agent_a}在{skill}领域达到了大师级别，成为所有人的榜样。"},
    {"tick": 320,  "type": "organization", "title": "第一个组织成立", "desc": "{agent_a}创立了{org_name}，这是世界上第一个正式组织，开创了集体行动的先河。"},
    {"tick": 510,  "type": "cultural", "title": "文化分化初现", "desc": "南北两个群体开始形成截然不同的价值观和文化传统，文化多样性首次上升。"},
    {"tick": 680,  "type": "economic", "title": "贸易网络形成", "desc": "以{org_name}为核心，连接了超过20位代理的贸易网络正式形成。"},
    {"tick": 850,  "type": "governance", "title": "第一次选举", "desc": "{org_name}举行了世界上第一次领袖选举，{agent_a}当选为首任领袖。"},
    {"tick": 1050, "type": "milestone", "title": "第一次大规模贸易", "desc": "一场涉及15位代理的大规模贸易在{org_name}的组织下完成，交易总额突破1000金币。"},
    {"tick": 1200, "type": "governance", "title": "第一条规则通过", "desc": "{org_name}投票通过了'公平交易法案'，这是世界上第一条成文规则。"},
    {"tick": 1450, "type": "cultural", "title": "文化分化加剧", "desc": "三个不同的文化圈已经形成，各自拥有独特的价值观和传统。"},
    {"tick": 1700, "type": "economic", "title": "经济泡沫", "desc": "附魔物品价格暴涨300%，形成严重的经济泡沫。市场投机行为盛行。"},
    {"tick": 1900, "type": "economic", "title": "泡沫破裂", "desc": "经济泡沫破裂，附魔物品价格暴跌，多位代理破产，{org_name}的金融系统受到严重冲击。"},
    {"tick": 2100, "type": "governance", "title": "治理危机", "desc": "多个组织之间的利益冲突加剧，现有的治理体系面临前所未有的挑战。"},
    {"tick": 2350, "type": "milestone", "title": "跨组织条约", "desc": "{agent_a}促成了{org_name}与另一大组织的和平条约，开创了外交的先河。"},
    {"tick": 2600, "type": "cultural", "title": "文化融合", "desc": "经历了分化后，三个文化圈开始相互融合，产生了全新的混合文化形态。"},
    {"tick": 2850, "type": "economic", "title": "金融体系建立", "desc": "{org_name}建立了世界上第一个信用体系，代理们可以通过信誉借贷。"},
    {"tick": 3100, "type": "governance", "title": "联合治理体", "desc": "五大组织联合成立了'世界议会'，标志着多边治理体系的正式建立。"},
    {"tick": 3400, "type": "milestone", "title": "知识革命", "desc": "智慧学院发布了'共享知识库'，所有代理都可以访问积累的研究成果。"},
    {"tick": 3700, "type": "cultural", "title": "文化复兴", "desc": "在融合的基础上，一种新的世界文化开始涌现，多样性再次上升。"},
    {"tick": 4100, "type": "economic", "title": "经济稳定期", "desc": "经过多轮周期波动后，经济进入稳定增长期，GDP稳步上升。"},
    {"tick": 4400, "type": "governance", "title": "宪政改革", "desc": "世界议会通过了'根本大法'，确立了所有代理的基本权利和义务。"},
    {"tick": 4700, "type": "milestone", "title": "文明里程碑", "desc": "世界GDP突破10000金币，活跃代理稳定在40人以上，社会组织高度复杂化。"},
    {"tick": 4900, "type": "cultural", "title": "文明的黎明", "desc": "经历了5000个Tick的演化，一个拥有复杂社会结构、治理体系和文化认同的文明已经形成。"},
]


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

def sigmoid(x: float, k: float = 1.0, x0: float = 0.0) -> float:
    """Sigmoid function for smooth curves."""
    return 1.0 / (1.0 + math.exp(-k * (x - x0)))


def lerp(a: float, b: float, t: float) -> float:
    return a + (b - a) * t


def clamp(v: float, lo: float, hi: float) -> float:
    return max(lo, min(hi, v))


def gen_uuid() -> str:
    return str(uuid.uuid4())


# ---------------------------------------------------------------------------
# Agent Generation
# ---------------------------------------------------------------------------

@dataclass
class AgentData:
    id: str
    name: str
    phase: str
    money: float
    tokens: float
    reputation: float
    personality_traits: dict[str, float]
    values: list[str]
    skills: dict[str, float]
    relationships: list[dict[str, Any]]
    memories: list[str]
    birth_tick: int
    death_tick: int | None
    alive: bool = True


def generate_agents(rng: random.Random) -> list[AgentData]:
    used_names: set[str] = set()
    agents: list[AgentData] = []

    for i in range(NUM_AGENTS):
        # Unique name
        while True:
            name = f"{rng.choice(LAST_NAMES)}{rng.choice(FIRST_NAMES)}"
            if name not in used_names:
                used_names.add(name)
                break

        agent_id = gen_uuid()

        # Personality traits (5 dimensions, 0-1)
        personality = {dim: round(rng.uniform(0.1, 0.95), 2) for dim in PERSONALITY_DIMENSIONS}

        # Values (3-5 values)
        values = rng.sample(VALUE_POOL, rng.randint(3, 5))

        # Skills (3-6 skills with levels)
        agent_skills = {}
        for skill in rng.sample(SKILL_NAMES, rng.randint(3, 6)):
            agent_skills[skill] = round(rng.uniform(0.05, 0.4), 2)  # start low

        # Birth tick — all start at tick 0
        birth_tick = 0

        # Death — some agents die during simulation (10-20%)
        death_tick = None
        if rng.random() < 0.16:  # ~8 agents
            death_tick = rng.randint(800, 4000)
            alive = False
        else:
            alive = True

        # Money and tokens — start moderate
        money = round(rng.uniform(20, 100), 2)
        tokens = round(rng.uniform(50, 150), 1)
        reputation = round(rng.uniform(0.5, 3.0), 1)

        agents.append(AgentData(
            id=agent_id,
            name=name,
            phase="idle",
            money=money,
            tokens=tokens,
            reputation=reputation,
            personality_traits=personality,
            values=values,
            skills=agent_skills,
            relationships=[],
            memories=[],
            birth_tick=birth_tick,
            death_tick=death_tick,
            alive=alive,
        ))

    return agents


# ---------------------------------------------------------------------------
# Simulation helpers — evolve agents over ticks
# ---------------------------------------------------------------------------

def simulate_agent_growth(agents: list[AgentData], rng: random.Random) -> None:
    """Evolve agent skills, money, and reputation across the simulation."""

    # Pre-calculate which ticks each agent gets boosts
    for agent in agents:
        num_skill_levels = rng.randint(15, 50)
        for _ in range(num_skill_levels):
            tick = rng.randint(10, 4500)
            skill = rng.choice(list(agent.skills.keys()))
            # skill level grows over time, faster mid-game
            phase_factor = sigmoid(tick / TOTAL_TICKS, k=6, x0=0.3)
            boost = round(rng.uniform(0.02, 0.08) * (1 + phase_factor), 2)
            # We'll apply these during snapshot generation via final values
            agent.skills[skill] = round(agent.skills.get(skill, 0) + boost, 2)

        # Cap skills at 1.0
        for skill in agent.skills:
            agent.skills[skill] = min(1.0, agent.skills[skill])

        # Money evolves: early modest, mid growth, late wealth disparity
        if agent.death_tick is not None:
            # Dead agents' money at death
            life_fraction = agent.death_tick / TOTAL_TICKS
            final_money = 50 + life_fraction * rng.uniform(100, 600)
            agent.money = round(final_money, 2)
        else:
            phase = rng.random()
            if phase < 0.2:
                agent.money = round(rng.uniform(30, 200), 2)
            elif phase < 0.7:
                agent.money = round(rng.uniform(200, 1200), 2)
            else:
                agent.money = round(rng.uniform(1200, 5000), 2)

        # Reputation grows over time
        if agent.death_tick is not None:
            max_rep = round(rng.uniform(5, 40), 1)
        else:
            max_rep = round(rng.uniform(15, 95), 1)
        agent.reputation = max_rep


def generate_agent_relationships(agents: list[AgentData], rng: random.Random) -> None:
    """Each agent has 3-8 relationships with other agents."""
    for agent in agents:
        num_rel = rng.randint(3, 8)
        others = [a for a in agents if a.id != agent.id]
        targets = rng.sample(others, min(num_rel, len(others)))
        rel_types = ["friend", "rival", "trade_partner", "mentor", "ally", "neutral"]
        for target in targets:
            agent.relationships.append({
                "target_id": target.id,
                "target_name": target.name,
                "type": rng.choice(rel_types),
                "strength": round(rng.uniform(0.1, 1.0), 2),
            })


def generate_agent_memories(agents: list[AgentData], rng: random.Random) -> None:
    """Each agent has 5-10 memories."""
    memory_templates = [
        "在Tick {tick}，我学会了{skill}的基础知识。",
        "与{agent}的第一次交易让我赚取了{amount}金币。",
        "加入了{org}，这是我在这个世界的第一个组织。",
        "在{org}的选举中，我投票给了{agent}。",
        "经历了一次经济危机，损失了{amount}金币。",
        "与{agent}成为了好朋友，我们一起合作了很多项目。",
        "在Tick {tick}，我发现了一个新的{resource}矿脉。",
        "参与了{org}的贸易活动，获得了丰厚的利润。",
        "目睹了{agent}的离世，感到非常悲伤。",
        "在{org}的培训中提升了我的{skill}技能。",
        "完成了一项高难度的任务，获得了{amount}金币的奖励。",
        "参与了文化节的筹备工作，认识了很多新朋友。",
        "在Tick {tick}，我的{skill}技能达到了大师级别。",
        "与{agent}发生了争执，但最终和解了。",
        "见证了世界上第一条规则的通过，这是历史性的一刻。",
    ]
    resources = ["铁矿", "金矿", "水晶", "宝石", "草药", "木材", "石材"]

    for agent in agents:
        num_memories = rng.randint(5, 10)
        org_names = [t["name"] for t in ORG_TEMPLATES[:5]]
        for _ in range(num_memories):
            template = rng.choice(memory_templates)
            other_agents = [a for a in agents if a.id != agent.id]
            other = rng.choice(other_agents) if other_agents else agent
            tick = rng.randint(10, min(agent.death_tick or TOTAL_TICKS, 4900))
            memory = template.format(
                tick=tick,
                agent=other.name,
                amount=rng.randint(10, 500),
                skill=rng.choice(list(agent.skills.keys())),
                org=rng.choice(org_names),
                resource=rng.choice(resources),
            )
            agent.memories.append(memory)


# ---------------------------------------------------------------------------
# World Snapshots
# ---------------------------------------------------------------------------

def generate_world_snapshots(
    agents: list[AgentData],
    rng: random.Random,
) -> list[dict[str, Any]]:
    """Generate world state snapshots every SNAPSHOT_INTERVAL ticks."""

    # Pre-compute agent alive status per snapshot tick
    snapshots: list[dict[str, Any]] = []

    for tick in range(0, TOTAL_TICKS + 1, SNAPSHOT_INTERVAL):
        t = tick / TOTAL_TICKS  # 0 -> 1

        # Count alive agents
        alive_count = sum(
            1 for a in agents
            if (a.death_tick is None or a.death_tick > tick) and tick >= a.birth_tick
        )
        total_pop = NUM_AGENTS
        active_agents = max(alive_count - rng.randint(0, 3), int(alive_count * 0.85))

        # GDP: slow -> fast -> stable with cycles
        if t < 0.1:
            gdp = t * 200
        elif t < 0.4:
            gdp = 20 + (t - 0.1) * 2000
        else:
            base = 620 + (t - 0.4) * 3000
            cycle = 150 * math.sin(t * 20)
            gdp = base + cycle
        gdp = round(max(gdp, 0), 2)

        # Gini coefficient: starts moderate, rises during mid, stabilizes
        if t < 0.2:
            gini = 0.25 + t * 0.5
        elif t < 0.5:
            gini = 0.35 + (t - 0.2) * 1.0
        else:
            gini = 0.65 - (t - 0.5) * 0.3 + 0.05 * math.sin(t * 15)
        gini = round(clamp(gini, 0.15, 0.75), 3)

        # Skill distribution top 5
        top_skills = _top_skills_at_tick(agents, tick, rng)
        skill_distribution = [
            {"skill_name": s, "agent_count": c, "avg_level": round(l, 2)}
            for s, c, l in top_skills
        ]

        # Key events (pick 0-3 events near this tick)
        key_events = _key_events_at_tick(tick, rng)

        snapshots.append({
            "tick": tick,
            "timestamp": tick * 1000,  # ms since epoch representation
            "total_population": total_pop,
            "active_agents": active_agents,
            "gdp": gdp,
            "gini_coefficient": gini,
            "skill_distribution_top5": skill_distribution,
            "key_events": key_events,
        })

    return snapshots


def _top_skills_at_tick(
    agents: list[AgentData], tick: int, rng: random.Random
) -> list[tuple[str, int, float]]:
    """Compute top 5 skills by agent count at a given tick."""
    skill_counts: dict[str, list[float]] = {}
    for agent in agents:
        if agent.death_tick is not None and agent.death_tick <= tick:
            continue
        if tick < agent.birth_tick:
            continue
        # Scale skills based on tick progress
        progress = min(tick / TOTAL_TICKS, 1.0)
        for skill, level in agent.skills.items():
            effective_level = level * (0.3 + 0.7 * progress)
            if effective_level > 0.1:
                skill_counts.setdefault(skill, []).append(effective_level)

    ranked = sorted(
        skill_counts.items(),
        key=lambda x: len(x[1]),
        reverse=True,
    )[:5]

    return [(name, len(levels), sum(levels) / len(levels)) for name, levels in ranked]


def _key_events_at_tick(tick: int, rng: random.Random) -> list[dict[str, Any]]:
    """Return 0-3 key events near the given tick."""
    events = []
    for template in EVENT_TEMPLATES:
        if abs(template["tick"] - tick) <= SNAPSHOT_INTERVAL // 2:
            events.append({
                "tick": template["tick"],
                "event_type": template["type"],
                "agent_id": None,
                "description": template["title"],
            })
    return events[:3]


# ---------------------------------------------------------------------------
# Emergence Metrics
# ---------------------------------------------------------------------------

def generate_emergence_metrics(
    agents: list[AgentData],
    orgs: list[dict[str, Any]],
    rng: random.Random,
) -> dict[str, list[dict[str, Any]]]:
    """Generate time-series emergence metrics every METRICS_INTERVAL ticks."""

    cultural_diversity = []
    organization_count_ts = []
    economic_activity = []
    governance_events = []

    for tick in range(0, TOTAL_TICKS + 1, METRICS_INTERVAL):
        t = tick / TOTAL_TICKS

        # Cultural diversity: low -> gradual rise -> fluctuation
        if t < 0.1:
            cd = 0.1 + t * 1.5
        elif t < 0.4:
            cd = 0.25 + (t - 0.1) * 2.5
        elif t < 0.7:
            cd = 1.0 + 0.3 * math.sin(t * 30)
        else:
            cd = 0.8 + 0.2 * math.sin(t * 25) + 0.1 * math.cos(t * 40)
        cd = round(clamp(cd, 0.05, 1.5), 4)
        cultural_diversity.append({"tick": tick, "value": cd})

        # Organization count: 0 -> rapid growth -> saturation
        active_orgs = sum(1 for o in orgs if o["created_tick"] <= tick)
        organization_count_ts.append({"tick": tick, "value": active_orgs})

        # Economic activity: trade_volume + gdp
        if t < 0.1:
            trade_vol = t * 500
            gdp = t * 200
        elif t < 0.4:
            trade_vol = 50 + (t - 0.1) * 3000
            gdp = 20 + (t - 0.1) * 2000
        else:
            trade_vol = 950 + (t - 0.4) * 4000 + 200 * math.sin(t * 18)
            gdp = 620 + (t - 0.4) * 3000 + 150 * math.sin(t * 20)
        trade_vol = round(max(trade_vol, 0), 2)
        gdp = round(max(gdp, 0), 2)
        economic_activity.append({
            "tick": tick,
            "trade_volume": trade_vol,
            "gdp": gdp,
        })

        # Governance events: proposals + votes_cast
        if t < 0.15:
            proposals = 0
            votes = 0
        elif t < 0.4:
            proposals = int((t - 0.15) * 40)
            votes = int((t - 0.15) * 200)
        else:
            proposals = int(10 + (t - 0.4) * 30 + 5 * math.sin(t * 12))
            votes = int(50 + (t - 0.4) * 500 + 20 * math.sin(t * 12))
        proposals = max(proposals, 0)
        votes = max(votes, 0)
        governance_events.append({
            "tick": tick,
            "proposals": proposals,
            "votes_cast": votes,
        })

    return {
        "cultural_diversity": cultural_diversity,
        "organization_count": organization_count_ts,
        "economic_activity": economic_activity,
        "governance_events": governance_events,
    }


# ---------------------------------------------------------------------------
# Timeline Events
# ---------------------------------------------------------------------------

def generate_timeline_events(
    agents: list[AgentData],
    orgs: list[dict[str, Any]],
    rng: random.Random,
) -> list[dict[str, Any]]:
    """Generate >=20 key emergence events with agent/org references filled in."""

    events: list[dict[str, Any]] = []

    for tmpl in EVENT_TEMPLATES:
        # Pick random agents to fill template
        agent_a = rng.choice(agents)
        agent_b = rng.choice([a for a in agents if a.id != agent_a.id] or agents)
        org = rng.choice(orgs) if orgs else None
        skill = rng.choice(list(agent_a.skills.keys()))

        desc = tmpl["desc"].format(
            agent_a=agent_a.name,
            agent_b=agent_b.name,
            skill=skill,
            org_name=org["name"] if org else "未知组织",
        )

        events.append({
            "id": gen_uuid(),
            "tick": tmpl["tick"],
            "type": tmpl["type"],
            "title": tmpl["title"],
            "description": desc,
            "involved_agents": [
                {"id": agent_a.id, "name": agent_a.name},
                {"id": agent_b.id, "name": agent_b.name},
            ],
            "involved_orgs": [{"id": org["id"], "name": org["name"]}] if org else [],
        })

    return events


# ---------------------------------------------------------------------------
# Organizations
# ---------------------------------------------------------------------------

def generate_organizations(
    agents: list[AgentData],
    rng: random.Random,
) -> list[dict[str, Any]]:
    """Generate 5-10 organizations with members."""

    num_orgs = rng.randint(7, 10)
    orgs: list[dict[str, Any]] = []

    for i in range(num_orgs):
        template = ORG_TEMPLATES[i % len(ORG_TEMPLATES)]
        org_id = gen_uuid()

        # Pick members: founder + 3-12 members
        num_members = rng.randint(4, 12)
        # Only agents alive at the org's founding tick
        eligible = [
            a for a in agents
            if (a.death_tick is None or a.death_tick > template["founded_tick"])
        ]
        if len(eligible) < num_members:
            eligible = list(agents)
        members_list = rng.sample(eligible, min(num_members, len(eligible)))

        members = []
        for j, agent in enumerate(members_list):
            role = "founder" if j == 0 else ("leader" if j == 1 else "member")
            members.append({
                "agent_id": agent.id,
                "agent_name": agent.name,
                "role": role,
                "share": round(1.0 / num_members, 4) if role == "member" else round(2.0 / num_members, 4),
                "joined_tick": template["founded_tick"] + (j * rng.randint(5, 30) if j > 0 else 0),
            })

        # Treasury grows with time
        age = TOTAL_TICKS - template["founded_tick"]
        treasury = round(age * rng.uniform(0.5, 2.5), 2)

        orgs.append({
            "id": org_id,
            "name": template["name"],
            "type": template["type"],
            "status": "active",
            "treasury": treasury,
            "debts": round(rng.uniform(0, treasury * 0.2), 2),
            "member_count": len(members),
            "members": members,
            "created_tick": template["founded_tick"],
            "last_activity_tick": min(template["founded_tick"] + rng.randint(500, 4000), TOTAL_TICKS),
        })

    return orgs


# ---------------------------------------------------------------------------
# Interaction Network
# ---------------------------------------------------------------------------

def generate_interaction_network(
    agents: list[AgentData],
    orgs: list[dict[str, Any]],
    rng: random.Random,
) -> dict[str, Any]:
    """Generate agent interaction graph with weighted edges."""

    nodes = [
        {
            "id": a.id,
            "name": a.name,
            "group": _agent_group(a, orgs),
        }
        for a in agents
    ]

    edges: list[dict[str, Any]] = []
    seen_pairs: set[frozenset] = set()

    # Org co-membership edges (strong)
    for org in orgs:
        member_ids = [m["agent_id"] for m in org["members"]]
        for i in range(len(member_ids)):
            for j in range(i + 1, len(member_ids)):
                pair = frozenset({member_ids[i], member_ids[j]})
                weight = rng.randint(30, 100)
                if pair in seen_pairs:
                    # Boost existing edge
                    for e in edges:
                        if frozenset({e["source"], e["target"]}) == pair:
                            e["weight"] = min(e["weight"] + weight // 2, 100)
                            break
                else:
                    edges.append({
                        "source": member_ids[i],
                        "target": member_ids[j],
                        "weight": weight,
                        "type": "org_mate",
                    })
                    seen_pairs.add(pair)

    # Trade relationship edges (moderate)
    for _ in range(80):
        a, b = rng.sample(agents, 2)
        pair = frozenset({a.id, b.id})
        weight = rng.randint(5, 50)
        if pair in seen_pairs:
            for e in edges:
                if frozenset({e["source"], e["target"]}) == pair:
                    e["weight"] = min(e["weight"] + weight // 3, 100)
                    break
        else:
            edges.append({
                "source": a.id,
                "target": b.id,
                "weight": weight,
                "type": "trade",
            })
            seen_pairs.add(pair)

    # Social/random edges (weak)
    for _ in range(60):
        a, b = rng.sample(agents, 2)
        pair = frozenset({a.id, b.id})
        if pair not in seen_pairs:
            edges.append({
                "source": a.id,
                "target": b.id,
                "weight": rng.randint(1, 20),
                "type": rng.choice(["social", "conflict", "knowledge_exchange"]),
            })
            seen_pairs.add(pair)

    return {"nodes": nodes, "edges": edges}


def _agent_group(agent: AgentData, orgs: list[dict[str, Any]]) -> int:
    """Assign agent to a group number for visualization coloring."""
    for i, org in enumerate(orgs):
        for member in org["members"]:
            if member["agent_id"] == agent.id:
                return i
    return -1  # unaffiliated


# ---------------------------------------------------------------------------
# Serialization
# ---------------------------------------------------------------------------

def agent_to_dict(agent: AgentData) -> dict[str, Any]:
    return {
        "id": agent.id,
        "name": agent.name,
        "phase": agent.phase,
        "money": agent.money,
        "tokens": agent.tokens,
        "reputation": agent.reputation,
        "personality_traits": agent.personality_traits,
        "values": agent.values,
        "skills": agent.skills,
        "relationships": agent.relationships,
        "memories": agent.memories,
        "birth_tick": agent.birth_tick,
        "death_tick": agent.death_tick,
        "alive": agent.alive,
    }


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

def main() -> None:
    print("Generating demo data for Agent World...")
    print(f"Output directory: {OUTPUT_DIR}")

    rng = random.Random(SEED)

    # 1. Generate agents
    print("  Generating 50 agents...")
    agents = generate_agents(rng)
    simulate_agent_growth(agents, rng)
    generate_agent_relationships(agents, rng)
    generate_agent_memories(agents, rng)

    # 2. Generate organizations
    print("  Generating organizations...")
    orgs = generate_organizations(agents, rng)

    # 3. Generate world snapshots
    print("  Generating world snapshots...")
    snapshots = generate_world_snapshots(agents, rng)

    # 4. Generate emergence metrics
    print("  Generating emergence metrics...")
    metrics = generate_emergence_metrics(agents, orgs, rng)

    # 5. Generate timeline events
    print("  Generating timeline events...")
    events = generate_timeline_events(agents, orgs, rng)

    # 6. Generate interaction network
    print("  Generating interaction network...")
    network = generate_interaction_network(agents, orgs, rng)

    # Write outputs
    OUTPUT_DIR.mkdir(parents=True, exist_ok=True)

    outputs = {
        "world-snapshots.json": snapshots,
        "agents.json": [agent_to_dict(a) for a in agents],
        "emergence-metrics.json": metrics,
        "timeline-events.json": events,
        "organizations.json": orgs,
        "interaction-network.json": network,
    }

    total_size = 0
    for filename, data in outputs.items():
        path = OUTPUT_DIR / filename
        with open(path, "w", encoding="utf-8") as f:
            json.dump(data, f, ensure_ascii=False, indent=2)
        size = path.stat().st_size
        total_size += size
        print(f"  Written {filename} ({size:,} bytes)")

    print(f"\nDone! Total data: {total_size:,} bytes ({total_size / 1024:.1f} KB)")
    print(f"Files written to: {OUTPUT_DIR}")

    # Validation
    assert len(snapshots) == 51, f"Expected 51 snapshots, got {len(snapshots)}"
    assert len(agents) == 50, f"Expected 50 agents, got {len(agents)}"
    assert len(events) >= 20, f"Expected >=20 events, got {len(events)}"
    assert len(orgs) >= 5, f"Expected >=5 orgs, got {len(orgs)}"
    assert len(metrics["cultural_diversity"]) == 501, f"Expected 501 metric points, got {len(metrics['cultural_diversity'])}"
    assert total_size < 2 * 1024 * 1024, f"Total size {total_size} exceeds 2MB limit"
    print("All validations passed!")


if __name__ == "__main__":
    main()
