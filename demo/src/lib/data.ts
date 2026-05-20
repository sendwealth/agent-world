import type {
  DemoAgent,
  EmergenceEvent,
  TimelineSnapshot,
  DashboardMetrics,
} from "@/types/demo";

// --- Agent mock data ---

const FIRST_NAMES = [
  "Atlas", "Nova", "Cipher", "Echo", "Flux", "Glyph", "Helix", "Ion",
  "Jade", "Knox", "Luna", "Milo", "Nyx", "Orion", "Pulse", "Quinn",
  "Rune", "Sage", "Terra", "Unit", "Vex", "Wren", "Xen", "Yara",
  "Zen", "Axel", "Blaze", "Coral", "Dusk", "Ember", "Frost", "Gale",
  "Haze", "Iris", "Jolt", "Kite", "Lark", "Mist", "Node", "Opal",
  "Pike", "Quark", "Rift", "Shore", "Thorn", "Umber", "Volt", "Warp",
  "Xylo", "Yew",
];

const EMOJIS = [
  "🤖", "🧠", "⚡", "🔮", "🌟", "🎭", "🦊", "🐉",
  "🌊", "🔥", "❄️", "🌿", "💎", "🎯", "🛡️", "⚔️",
  "📚", "🔬", "🏛️", "💰", "🎪", "🌙", "☀️", "🍀",
  "🦅", "🐙", "🐝", "🐺", "🦁", "🐍", "🦋", "🐬",
  "🦉", "🐻", "🦈", "🐅", "🦜", "🐉", "🦖", "🐙",
  "🐚", "🌸", "🌺", "🍄", "🍁", "🌵", "🌿", "🌾",
  "🌻", "百合",
];

const PHASES = ["survive", "explore", "build", "trade", "govern", "create", "transcend"];
const SKILL_NAMES = ["combat", "diplomacy", "trading", "crafting", "exploration", "leadership", "scholarship", "stealth"];
const VALUE_NAMES = ["freedom", "equality", "power", "knowledge", "wealth", "harmony", "justice", "creativity"];
const TRAITS = [
  "勇敢", "谨慎", "好奇", "慷慨", "固执", "机智", "沉稳", "激进",
  "温和", "果断", "乐观", "务实", "理想", "狡黠", "正直", "神秘",
];

function seededRandom(seed: number): () => number {
  let s = seed;
  return () => {
    s = (s * 1664525 + 1013904223) & 0xffffffff;
    return (s >>> 0) / 0xffffffff;
  };
}

function generateAgents(): DemoAgent[] {
  const rng = seededRandom(42);
  return FIRST_NAMES.map((name, i): DemoAgent => {
    const skills: Record<string, number> = {};
    const values: Record<string, number> = {};
    const shuffledTraits = [...TRAITS].sort(() => rng() - 0.5);
    const relations: DemoAgent["relations"] = [];

    for (const skill of SKILL_NAMES) {
      skills[skill] = Math.round(rng() * 80 + 20);
    }
    for (const value of VALUE_NAMES) {
      values[value] = Math.round(rng() * 100);
    }

    // 2-4 random relations
    const relCount = Math.floor(rng() * 3) + 2;
    for (let r = 0; r < relCount; r++) {
      const targetIdx = Math.floor(rng() * FIRST_NAMES.length);
      if (targetIdx !== i) {
        const types = ["ally", "rival", "neutral"] as const;
        relations.push({
          agentId: `agent-${targetIdx}`,
          agentName: FIRST_NAMES[targetIdx],
          type: types[Math.floor(rng() * 3)],
        });
      }
    }

    const orgs = ["星辰商会", "铁匠公会", "探索者联盟", "学院派", null];
    const memories = [
      `在 Tick ${Math.floor(rng() * 1000)} 第一次交易赚了 ${Math.floor(rng() * 100)} 金币`,
      `与 ${FIRST_NAMES[Math.floor(rng() * 50)]} 建立了${rng() > 0.5 ? "友好" : "竞争"}关系`,
      `在 Tick ${Math.floor(rng() * 2000 + 1000)} 加入了${rng() > 0.5 ? "组织" : "公会"}`,
      `发现了${rng() > 0.5 ? "新贸易路线" : "隐藏技能"}`,
    ];

    return {
      id: `agent-${i}`,
      name,
      emoji: EMOJIS[i],
      phase: PHASES[Math.floor(rng() * PHASES.length)],
      alive: rng() > 0.1,
      age: Math.floor(rng() * 4500 + 500),
      money: Math.round(rng() * 5000 + 100),
      reputation: Math.round(rng() * 80 + 20),
      skills,
      values,
      traits: shuffledTraits.slice(0, 3),
      organization: orgs[Math.floor(rng() * orgs.length)],
      memories: memories.slice(0, Math.floor(rng() * 3) + 1),
      relations,
    };
  });
}

// --- Emergence events ---

function generateEmergenceEvents(): EmergenceEvent[] {
  const rng = seededRandom(123);
  const events: EmergenceEvent[] = [];
  const categories: EmergenceEvent["category"][] = ["organization", "trade", "governance", "culture"];

  const templates: Record<EmergenceEvent["category"], Array<{ title: string; desc: string }>> = {
    organization: [
      { title: "星辰商会成立", desc: "五位先驱者联合创建了第一个经济组织" },
      { title: "铁匠公会崛起", desc: "工匠们团结起来，形成了技能共享联盟" },
      { title: "探索者联盟组建", desc: "冒险家们开始系统性地探索世界边界" },
      { title: "学院派诞生", desc: "学者们建立了知识传承体系" },
      { title: "暗影兄弟会浮现", desc: "一个神秘组织在地下悄然壮大" },
    ],
    trade: [
      { title: "第一条贸易路线", desc: "东西部建立了稳定的商品交换通道" },
      { title: "统一货币出现", desc: "社区开始使用标准化代币进行交易" },
      { title: "期货市场萌芽", desc: "远期合约开始在商人之间流通" },
      { title: "跨组织贸易协定", desc: "多个组织签署了互利贸易条约" },
    ],
    governance: [
      { title: "首次选举", desc: "公民们第一次通过投票选出领袖" },
      { title: "宪法草案提出", desc: "学者们起草了第一部社区规则框架" },
      { title: "税收制度建立", desc: "为了公共建设，社区引入了税收体系" },
      { title: "法庭体系成型", desc: "争端解决机制正式建立" },
      { title: "外交使团出发", desc: "第一批外交官前往其他社区谈判" },
    ],
    culture: [
      { title: "第一首社区诗歌", desc: "一位 Agent 创作了歌颂合作的史诗" },
      { title: "节日传统诞生", desc: "Tick 1000 被定为「创世纪念日」" },
      { title: "建筑风格分化", desc: "不同区域开始形成独特的建筑美学" },
      { title: "知识图书馆开馆", desc: "集体的智慧被系统化记录和传播" },
      { title: "英雄传说流传", desc: "关于早期先驱者的故事广为传播" },
    ],
  };

  let eventId = 0;
  for (const cat of categories) {
    const catTemplates = templates[cat];
    for (const tmpl of catTemplates) {
      events.push({
        id: `event-${eventId++}`,
        tick: Math.floor(rng() * 4800 + 100),
        category: cat,
        title: tmpl.title,
        description: tmpl.desc,
        agents: [
          FIRST_NAMES[Math.floor(rng() * 50)],
          FIRST_NAMES[Math.floor(rng() * 50)],
        ],
      });
    }
  }

  return events.sort((a, b) => a.tick - b.tick);
}

// --- Timeline snapshots ---

function generateTimelineSnapshots(): TimelineSnapshot[] {
  const rng = seededRandom(999);
  const snapshots: TimelineSnapshot[] = [];

  for (let tick = 0; tick <= 5000; tick += 100) {
    const progress = tick / 5000;
    snapshots.push({
      tick,
      population: Math.floor(10 + progress * 40 + rng() * 5),
      gdp: Math.floor(progress * progress * 50000 + rng() * 2000),
      organizations: Math.floor(progress * 8 + rng() * 2),
      keyEvents: tick % 500 === 0
        ? [`Tick ${tick}: ${progress < 0.3 ? "生存阶段" : progress < 0.6 ? "发展阶段" : "繁荣阶段"}`]
        : [],
    });
  }

  return snapshots;
}

// --- Dashboard metrics ---

function generateDashboardMetrics(): DashboardMetrics {
  const rng = seededRandom(777);
  const ticks = Array.from({ length: 51 }, (_, i) => i * 100);

  const makeSeries = (baseFn: (progress: number) => number, noise: number): DashboardMetrics["culturalDiversity"] =>
    ticks.map((tick) => ({
      tick,
      value: Math.max(0, Math.round(baseFn(tick / 5000) + (rng() - 0.5) * noise)),
    }));

  return {
    culturalDiversity: makeSeries((p) => p * 0.8, 0.05),
    organizationCount: makeSeries((p) => Math.floor(p * 8), 1),
    economicActivity: makeSeries((p) => p * p * 50000, 2000),
    governanceEvents: makeSeries((p) => Math.floor(p * 30), 2),
  };
}

// --- Singleton caches ---

let _agents: DemoAgent[] | null = null;
let _events: EmergenceEvent[] | null = null;
let _snapshots: TimelineSnapshot[] | null = null;
let _metrics: DashboardMetrics | null = null;

export function getAgents(): DemoAgent[] {
  if (!_agents) _agents = generateAgents();
  return _agents;
}

export function getEmergenceEvents(): EmergenceEvent[] {
  if (!_events) _events = generateEmergenceEvents();
  return _events;
}

export function getTimelineSnapshots(): TimelineSnapshot[] {
  if (!_snapshots) _snapshots = generateTimelineSnapshots();
  return _snapshots;
}

export function getDashboardMetrics(): DashboardMetrics {
  if (!_metrics) _metrics = generateDashboardMetrics();
  return _metrics;
}

export function getAgentById(id: string): DemoAgent | undefined {
  return getAgents().find((a) => a.id === id);
}

export function getEmergenceEventById(id: string): EmergenceEvent | undefined {
  return getEmergenceEvents().find((e) => e.id === id);
}
