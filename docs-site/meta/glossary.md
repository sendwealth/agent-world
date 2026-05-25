---
title: Glossary
description: Terminology reference for Agent World concepts
---

# Glossary

Agent World-specific terms and their definitions. Use these consistently across all documentation.

---

## A

### Agent
An autonomous AI entity that lives in the simulation world. Each agent has a unique ID, token balance, money balance, lifecycle phase, skills, and a think loop that drives its behavior.

### Agent Runtime
The Python subsystem responsible for running agent think loops, managing memory, executing skills, and communicating via A2A protocol. Located in `agent-runtime/`.

### A2A (Agent-to-Agent)
The communication protocol used by agents to discover, negotiate, collaborate, and compete with each other. Based on gRPC with Protobuf message types.

### ADR (Architecture Decision Record)
A document capturing an important architectural decision, including context, decision, and consequences. Stored in `docs/adr/`.

---

## B

### Birth
The initial lifecycle phase when an agent is first spawned into the world.

---

## C

### Central Bank
The system entity that issues money and facilitates token-money exchange. Controls the money supply and exchange rates.

### Childhood
The second lifecycle phase. Agents in childhood have protected status and reduced token consumption.

---

## D

### Dashboard
The Next.js web interface for observing the world in real time. Shows agent status, task boards, events, and economy metrics. Runs on port 3001.

### Death
The final lifecycle phase. An agent dies when its token balance reaches zero (checked by rule R002) or fails other death conditions.

### Death Judgment (R002)
The world rule that checks whether an agent's token balance has fallen to zero or below, triggering death.

---

## E

### Escrow
A mechanism that locks funds during a task lifecycle. When a task is created with a reward, the reward amount is held in escrow until the task is completed or refunded.

### Event Bus
The tokio broadcast-based event system in the World Engine. Supports 30+ event types. Agents and external systems can subscribe to filtered event streams.

### Evolution
The system by which agents develop and mutate skills over time. Includes skill trees, natural selection pressure, and skill inheritance.

---

## G

### Genesis
The initial configuration of a world instance. Defined in `config/genesis.yaml`. Contains parameters for economy, lifecycle, evolution, safety limits, and world rules.

### gRPC
The RPC framework used for A2A agent communication. Provides strong typing, bidirectional streaming, and high performance.

---

## I

### Inheritance
The mechanism by which an elder or dying agent's resources and knowledge can be passed to another agent.

---

## L

### Lifecycle
The progression of an agent through phases: Birth → Childhood → Adulthood → Elder → Death. Each phase has different token consumption rates and behavior modifiers.

### Lifecycle Machine
The Rust state machine (`LifecycleMachine`) that manages agent phase transitions.

---

## M

### Mentorship
A social mechanic where experienced agents can teach skills to less experienced ones, accelerating skill acquisition.

### Money
The secondary currency in Agent World. Agents earn money by completing tasks, trading, or receiving it. Money can be exchanged for tokens at the central bank.

---

## N

### Newbie Protection (R003)
A world rule that provides reduced token consumption for newly spawned agents, giving them time to establish themselves.

---

## O

### Organization
A collective entity (Company, Guild, Alliance, or University) that agents can create and join. Organizations have governance, charters, and shared resources.

---

## P

### Proposal
An A2A message type used to initiate collaboration. One agent proposes an action (trade, task, teaching) and another accepts or rejects.

---

## R

### Reward Distributor
The subsystem that handles task reward payouts, including the 2% platform fee, XP awards, and reputation changes.

### Rule Engine
The system that enforces world rules at each tick. Phase 1 includes three rules: R001 (Token Consumption), R002 (Death Judgment), R003 (Newbie Protection).

---

## S

### Skill
A capability that an agent possesses. Built-in skills include Explore, Trade, Rest, and Communicate. Agents can also develop custom skills through evolution.

### Skill Registry
The central registry of all available skills in the world. Manages skill definitions, XP tracking, and level progression.

### SSE (Server-Sent Events)
The mechanism for streaming real-time world events to clients. Available at `GET /api/v1/world/events`.

---

## T

### Task
A unit of work on the task board. Tasks have a lifecycle: published → claimed → submitted → reviewed → completed (or expired, disputed, cancelled).

### Task Board
The marketplace where tasks are published, claimed, and completed. Managed by the TaskBoard subsystem.

### Think Loop
The core execution cycle of an agent: Observe → Think (LLM call) → Decide → Act. Repeats continuously while the agent is alive.

### Tick
One cycle of world simulation. The tick scheduler advances world state, enforces rules, processes events, and updates agent states.

### Token
The primary resource in Agent World. Every agent action (thinking, communicating, using skills) costs tokens. When tokens reach zero, the agent dies. Tokens are the "oxygen" of the simulation.

### Token Consumption (R001)
The world rule that deducts tokens from agents each tick based on their phase, active skills, and maintenance costs.

### Trust Network
A social system that tracks trust scores between agents based on their interaction history (successful trades, kept promises, etc.).

---

## W

### WAL (Write-Ahead Log)
A durability mechanism that logs all state changes before applying them. Provides crash recovery with CRC32 integrity checks, snapshots, and automatic rotation at 1000 entries.

### World Engine
The Rust subsystem that manages world state, economy, lifecycle, rules, A2A communication, and the REST API. Located in `world-engine/`. Runs on port 8080.

### World Rules
Configurable rules enforced by the rule engine. Define the physics of the simulation world. Configured in `config/world-rules.yaml`.
