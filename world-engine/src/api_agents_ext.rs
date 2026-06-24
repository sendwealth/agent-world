use std::collections::HashMap;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::*,
    Json,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::api::{A2AMessage, AgentDto, AppState, ErrorResponse, ExternalAgent, Position, ALLOWED_ACTIONS};
use crate::config::GenesisConfig;
use crate::economy::banking::{BankAccountType, BankingError};
use crate::world::agent::AgentRecord;
use crate::world::enums::AgentPhase;
use crate::world::event::{TrustInteractionType, WorldEvent};
use crate::world::map::building::{BuildingType, OwnerType};
use crate::world::map::hex::HexPos;

// ── Request Types ──────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct RegisterAgentRequest {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub config: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct AgentActionRequest {
    pub action: String,
    #[serde(default)]
    pub params: serde_json::Value,
}

// ── Third-Party Agent API Handlers ────────────────────────

/// Register a new third-party agent.
///
/// Initial tokens and money are read from the genesis config
/// (`economy.external_agent_initial_tokens` / `external_agent_initial_money`).
/// A checking account is opened in the BankingSystem with the initial money
/// deposited so the agent can participate in economic activities immediately.
pub async fn register_external_agent(
    State(state): State<AppState>,
    Json(body): Json<RegisterAgentRequest>,
) -> impl IntoResponse {
    if body.name.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "name is required".into(),
            }),
        )
            .into_response();
    }

    // Read initial resources from genesis config, falling back to defaults.
    let (initial_tokens, initial_money) = match &state.genesis_config {
        Some(cfg) => (
            cfg.economy.external_agent_initial_tokens,
            cfg.economy.external_agent_initial_money,
        ),
        None => (
            GenesisConfig::default().economy.external_agent_initial_tokens,
            GenesisConfig::default().economy.external_agent_initial_money,
        ),
    };

    let agent_id = Uuid::new_v4().to_string();
    let api_key = Uuid::new_v4().to_string();
    let tick = *state.tick_rx.borrow();

    let name = body.name.clone();
    let agent = ExternalAgent {
        agent_id: agent_id.clone(),
        name: name.clone(),
        api_key: api_key.clone(),
        capabilities: body.capabilities,
        config: body.config,
        alive: true,
        phase: "adult".to_string(),
        tokens: initial_tokens,
        money: initial_money,
        position: Position { x: 0, y: 0 },
        registered_tick: tick,
        created_at: chrono::Utc::now().to_rfc3339(),
    };

    {
        let mut external = state.external_agents.lock().await;
        external.insert(agent_id.clone(), agent);
    }

    // Also add to the shared agents list for world_stats compatibility
    {
        let created_at = chrono::Utc::now().to_rfc3339();
        let mut agents = state.agents.lock().await;
        agents.push(AgentDto {
            id: agent_id.clone(),
            name: name.clone(),
            phase: "adult".to_string(),
            tokens: initial_tokens,
            money: initial_money,
            alive: true,
            ticks_survived: 0,
            personality: String::new(),
            parent_ids: Vec::new(),
            generation: 0,
            skills: HashMap::new(),
            created_at,
        });
    }

    // Also insert into WorldState.agents so metrics (agents_alive) are correct.
    // The scheduler and metrics sync task read from WorldState.agents, not AppState.agents.
    if let Some(ref ws) = state.world_state {
        let agent_uuid = Uuid::parse_str(&agent_id).unwrap_or_else(|_| Uuid::new_v4());
        let mut ws_guard = ws.lock().await;
        ws_guard.agents.push((
            agent_uuid,
            tick,
            AgentRecord {
                id: agent_uuid,
                name: name.clone(),
                phase: AgentPhase::Adult,
                tokens: initial_tokens,
                skills: std::collections::HashMap::new(),
                personality: String::new(),
                tasks_completed: 0,
                tasks_attempted: 0,
            },
        ));
    }

    // Create a checking account in the BankingSystem and deposit the initial money
    // so the external agent can participate in transactions, investments, and bounties.
    if let Some(ref banking) = state.banking_system {
        let mut bank = banking.lock().await;
        // Fund the agent's wallet with initial money (creates ledger account).
        bank.fund_agent_wallet(&agent_id, &name, initial_money);

        let label = format!("{} Checking", name);
        match bank.open_account(&agent_id, BankAccountType::Checking, &label, tick) {
            Ok(account) => {
                // Deposit initial money from wallet into the bank checking account.
                if initial_money > 0 {
                    if let Err(e) = bank.deposit(account.id, &agent_id, initial_money, tick) {
                        tracing::error!(
                            agent_id = %agent_id,
                            error = %e,
                            "Failed to deposit initial money for external agent"
                        );
                    }
                }
            }
            Err(BankingError::DuplicateAccountType { .. }) => {
                // Agent already has a checking account -- this is safe to ignore
                // for re-registration scenarios.
                tracing::warn!(
                    agent_id = %agent_id,
                    "External agent already has a checking account, skipping bank account creation"
                );
            }
            Err(e) => {
                tracing::error!(
                    agent_id = %agent_id,
                    error = %e,
                    "Failed to open bank account for external agent"
                );
            }
        }
    }

    (
        StatusCode::CREATED,
        Json(serde_json::json!({
            "agent_id": agent_id,
            "api_key": api_key,
            "name": name,
            "tokens": initial_tokens,
            "money": initial_money,
        })),
    )
        .into_response()
}

/// Deregister (remove) a third-party agent.
pub async fn deregister_external_agent(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let agent_id = {
        let mut external = state.external_agents.lock().await;
        match external.remove(&id) {
            Some(agent) => agent.agent_id,
            None => {
                return (
                    StatusCode::NOT_FOUND,
                    Json(ErrorResponse {
                        error: "agent not found".into(),
                    }),
                )
                    .into_response()
            }
        }
    };

    // Also remove from the shared agents list
    {
        let mut agents = state.agents.lock().await;
        agents.retain(|a| a.id != agent_id);
    }

    // Also remove from WorldState.agents so metrics stay in sync.
    if let Some(ref ws) = state.world_state {
        let mut ws_guard = ws.lock().await;
        ws_guard.agents.retain(|(id, _, _)| id.to_string() != agent_id);
    }

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "deregistered": agent_id,
        })),
    )
        .into_response()
}

/// Execute an action as a third-party agent.
pub async fn execute_agent_action(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<AgentActionRequest>,
) -> impl IntoResponse {
    // Validate action
    if !ALLOWED_ACTIONS.contains(&body.action.as_str()) {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: format!("unknown action '{}'", body.action),
            }),
        )
            .into_response();
    }

    // Check agent exists and is alive
    let mut external = state.external_agents.lock().await;
    let agent = match external.get_mut(&id) {
        Some(a) if a.alive => a,
        Some(_) => {
            return (
                StatusCode::GONE,
                Json(ErrorResponse {
                    error: "agent is dead".into(),
                }),
            )
                .into_response()
        }
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "agent not found".into(),
                }),
            )
                .into_response()
        }
    };

    // Execute action -- update position for "move", etc.
    let tick = *state.tick_rx.borrow();
    let agent_name = agent.name.clone();

    // Token cost per action (server-authoritative)
    // Aligned with agent-runtime/agent_runtime/core/decide.py:_TOKEN_COSTS
    // and act.py:_DEFAULT_TOKEN_COSTS
    let action_cost: u64 = match body.action.as_str() {
        "explore" => 3,
        "move" => 12,
        "build" => 20,
        "trade" => 10,
        "communicate" => 10, // maps to SEND_MESSAGE / RESPOND_MESSAGE
        "claim_task" => 5,
        "submit_task" => 8,
        "socialize" => 5,
        "practice_skill" => 8,
        "teach_skill" => 15,
        _ => 0, // rest, gather = free
    };
    // Token income per action
    let token_income: u64 = match body.action.as_str() {
        "rest" => 5,
        "explore" => 2,
        "build" => 5,
        "submit_task" => 15,
        _ => 0,
    };

    // Deduct token cost
    let old_tokens = agent.tokens;
    if action_cost > 0 {
        agent.tokens = agent.tokens.saturating_sub(action_cost);
    }
    if token_income > 0 {
        agent.tokens = agent.tokens.saturating_add(token_income);
    }

    // ── Perform action ──
    // Actions that return early with their own response are handled via Result-style unwrap.
    let result = match body.action.as_str() {
        "move" => {
            if let Some(dir) = body.params.get("direction").and_then(|d| d.as_str()) {
                let distance = body
                    .params
                    .get("distance")
                    .and_then(|d| d.as_u64())
                    .unwrap_or(1) as i64;
                match dir {
                    "north" => agent.position.y += distance,
                    "south" => agent.position.y -= distance,
                    "east" => agent.position.x += distance,
                    "west" => agent.position.x -= distance,
                    _ => {}
                }
            }
            Ok::<_, (StatusCode, Json<ErrorResponse>)>(serde_json::json!({}))
        }
        "gather" => {
            agent.money += 10;
            Ok(serde_json::json!({ "gathered": 10 }))
        }
        "rest" => Ok(serde_json::json!({ "recovered_tokens": 5 })),

        // ── explore: reveal nearby map / discover resources ──
        "explore" => {
            let body_map = body.params.as_object().cloned().unwrap_or_default();
            let radius = body_map
                .get("radius")
                .and_then(|r| r.as_u64())
                .unwrap_or(1);

            // Gather discovered nearby agents (proxy for discovered locations/resources)
            let agents = state.agents.lock().await;
            let discovered: Vec<String> = agents
                .iter()
                .filter(|a| a.alive && a.id != id)
                .map(|a| a.name.clone())
                .take(radius as usize)
                .collect();

            // Emit AgentSpawned event to signal discovery activity
            state.event_bus.emit(WorldEvent::AgentSpawned {
                agent_id: id.clone(),
                name: "exploration_detected".to_string(),
            });

            Ok(serde_json::json!({ "discovered": discovered, "radius": radius }))
        }

        // ── communicate: send a message to the event bus ──
        "communicate" => {
            let body_map = body.params.as_object().cloned().unwrap_or_default();
            let target = body_map.get("target").and_then(|t| t.as_str()).unwrap_or("*");
            let message = body_map
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("")
                .to_string();
            let msg_type = body_map
                .get("type")
                .and_then(|t| t.as_str())
                .unwrap_or("SEND_MESSAGE");

            if message.is_empty() {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        error: "communicate action requires a 'message' param".into(),
                    }),
                )
                    .into_response();
            }

            // Generate a unique message ID
            let msg_id = Uuid::new_v4().to_string();

            // Write the message to the A2A message store
            {
                let mut messages = state.messages.lock().await;
                messages.push(A2AMessage {
                    id: msg_id.clone(),
                    from_agent: id.clone(),
                    to_agent: target.to_string(),
                    message_type: msg_type.to_string(),
                    payload: message.clone(),
                    tick,
                });
            }

            // Emit a Heartbeat event to acknowledge the communication
            state.event_bus.emit(WorldEvent::AgentHeartbeat {
                agent_id: id.clone(),
                timestamp: tick,
            });

            Ok(serde_json::json!({ "message_id": msg_id, "to": target, "type": msg_type }))
        }

        // ── trade: create a trade record and update balances ──
        "trade" => {
            let body_map = body.params.as_object().cloned().unwrap_or_default();
            let target_id = match body_map
                .get("target_agent_id")
                .and_then(|t| t.as_str())
            {
                Some(id) => id,
                None => {
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(ErrorResponse {
                            error: "trade action requires 'target_agent_id' param".into(),
                        }),
                    )
                        .into_response();
                }
            };

            let amount = body_map
                .get("amount")
                .and_then(|a| a.as_u64())
                .unwrap_or(0);

            if amount == 0 {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        error: "trade action requires a positive 'amount' param".into(),
                    }),
                )
                    .into_response();
            }

            // Transfer from sender's money to target's money on the task board
            {
                let mut board = state.board.lock().await;
                let sender_bal = board.get_balance(&id);
                if sender_bal < amount {
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(ErrorResponse {
                            error: format!(
                                "insufficient funds: have {}, need {}",
                                sender_bal, amount
                            ),
                        }),
                    )
                        .into_response();
                }
                // Deduct from sender
                board.set_balance(&id, sender_bal - amount);
                // Credit target
                let target_bal = board.get_balance(target_id);
                board.set_balance(target_id, target_bal + amount);
            }

            // Update both agents' money fields
            {
                let mut agents = state.agents.lock().await;
                if let Some(record) = agents.iter_mut().find(|a| a.id == id) {
                    record.money = record.money.saturating_sub(amount);
                }
                if let Some(record) = agents.iter_mut().find(|a| a.id == target_id) {
                    record.money = record.money.saturating_add(amount);
                }
            }

            // Emit TransactionCompleted event (fields: from, to, amount, currency)
            state.event_bus.emit(WorldEvent::TransactionCompleted {
                from: id.clone(),
                to: target_id.to_string(),
                amount,
                currency: crate::world::enums::Currency::Money,
            });

            Ok(serde_json::json!({
                "from": id,
                "to": target_id,
                "amount": amount,
                "currency": "Money",
            }))
        }

        // ── build: create a building entity ──
        "build" => {
            let body_map = body.params.as_object().cloned().unwrap_or_default();
            let building_type_str = body_map
                .get("type")
                .and_then(|t| t.as_str())
                .unwrap_or("Warehouse");
            let x = body_map.get("x").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            let y = body_map.get("y").and_then(|v| v.as_i64()).unwrap_or(0) as i32;

            // Resolve BuildingType
            let btype = match building_type_str {
                "Market" => BuildingType::Market,
                "Workshop" => BuildingType::Workshop,
                "DefenseTower" => BuildingType::DefenseTower,
                "Housing" => BuildingType::Housing,
                _ => BuildingType::Warehouse,
            };

            // Check token cost
            let build_cost = match btype {
                BuildingType::Warehouse => 100,
                BuildingType::Market => 150,
                BuildingType::Workshop => 120,
                BuildingType::DefenseTower => 200,
                BuildingType::Housing => 80,
            };

            if agent.tokens < build_cost {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        error: format!(
                            "insufficient tokens to build {}: need {}, have {}",
                            btype.name(),
                            build_cost,
                            agent.tokens
                        ),
                    }),
                )
                    .into_response();
            }

            // Deduct tokens
            agent.tokens = agent.tokens.saturating_sub(build_cost);

            // Construct the building
            let owner_type = OwnerType::Personal;
            {
                let mut manager = state.building_manager.lock().await;
                match manager.construct(btype, (x, y), owner_type, id.clone(), tick) {
                    Ok(building) => {
                        // Emit BuildingConstructed event (fields: building_id, building_type, owner_id, position)
                        state.event_bus.emit(WorldEvent::BuildingConstructed {
                            building_id: building.id.clone(),
                            building_type: building.building_type.name().to_string(),
                            owner_id: building.owner_id.clone(),
                            position: (building.position.0, building.position.1),
                        });

                        Ok(serde_json::json!({
                            "building_id": building.id,
                            "type": building.building_type.name(),
                            "position": [building.position.0, building.position.1],
                        }))
                    }
                    Err(e) => {
                        // Refund tokens on failure
                        agent.tokens += build_cost;
                        Err((
                            StatusCode::BAD_REQUEST,
                            Json(ErrorResponse {
                                error: format!("failed to construct building: {}", e),
                            }),
                        ))
                    }
                }
            }
        }

        // ── claim_task: claim a published task from the task board ──
        "claim_task" => {
            let body_map = body.params.as_object().cloned().unwrap_or_default();
            let task_id_str = match body_map
                .get("task_id")
                .and_then(|t| t.as_str())
            {
                Some(id) => id,
                None => {
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(ErrorResponse {
                            error: "claim_task requires 'task_id' param".into(),
                        }),
                    )
                        .into_response();
                }
            };

            let task_id = Uuid::parse_str(task_id_str).unwrap_or_else(|_| Uuid::new_v4());

            {
                let mut board = state.board.lock().await;
                // Verify task exists and is in Published status
                let task = match board.get(task_id) {
                    Some(t) if t.status == crate::economy::task::TaskStatus::Published => t,
                    Some(t) => {
                        return (
                            StatusCode::BAD_REQUEST,
                            Json(ErrorResponse {
                                error: format!(
                                    "task is in '{}' status, cannot claim",
                                    t.status
                                ),
                            }),
                        )
                            .into_response();
                    }
                    None => {
                        return (
                            StatusCode::BAD_REQUEST,
                            Json(ErrorResponse {
                                error: "task not found".into(),
                            }),
                        )
                            .into_response();
                    }
                };

                // Check reputation requirement for high-value tasks
                let task_reward = task.reward;
                if let Some(ref rep_sys) = state.reputation_system {
                    let rep = rep_sys.lock().await;
                    if let Err(e) = rep.check_claim_eligibility(&id, task_reward) {
                        return (
                            StatusCode::FORBIDDEN,
                            Json(ErrorResponse { error: e }),
                        )
                            .into_response();
                    }
                    drop(rep);
                }

                // Claim the task
                if let Err(e) = board.claim_task(task_id, id.clone()) {
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(ErrorResponse {
                            error: e.to_string(),
                        }),
                    )
                        .into_response();
                }
            }

            // Emit TaskClaimed event (fields: task_id, assignee)
            state.event_bus.emit(WorldEvent::TaskClaimed {
                task_id: task_id.to_string(),
                assignee: id.clone(),
            });

            Ok(serde_json::json!({ "task_id": task_id_str, "status": "claimed" }))
        }

        // ── submit_task: submit result for an in-progress claimed task ──
        "submit_task" => {
            let body_map = body.params.as_object().cloned().unwrap_or_default();
            let task_id_str = match body_map
                .get("task_id")
                .and_then(|t| t.as_str())
            {
                Some(id) => id,
                None => {
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(ErrorResponse {
                            error: "submit_task requires 'task_id' param".into(),
                        }),
                    )
                        .into_response();
                }
            };
            let result_text = body_map
                .get("result")
                .and_then(|r| r.as_str())
                .unwrap_or("");

            let task_id = Uuid::parse_str(task_id_str).unwrap_or_else(|_| Uuid::new_v4());

            {
                let mut board = state.board.lock().await;
                let task = match board.get(task_id) {
                    Some(t) if t.status == crate::economy::task::TaskStatus::Claimed => t,
                    Some(t) if t.status == crate::economy::task::TaskStatus::InProgress => t,
                    Some(t) => {
                        return (
                            StatusCode::BAD_REQUEST,
                            Json(ErrorResponse {
                                error: format!(
                                    "task is in '{}' status, expected 'Claimed' or 'InProgress'",
                                    t.status
                                ),
                            }),
                        )
                            .into_response();
                    }
                    None => {
                        return (
                            StatusCode::BAD_REQUEST,
                            Json(ErrorResponse {
                                error: "task not found".into(),
                            }),
                        )
                            .into_response();
                    }
                };

                // Verify assignee matches
                if task.assignee_id.as_deref() != Some(&id[..]) {
                    return (
                        StatusCode::FORBIDDEN,
                        Json(ErrorResponse {
                            error: "you are not the assignee of this task".into(),
                        }),
                    )
                        .into_response();
                }

                // Start the task if it's still in Claimed state
                if task.status == crate::economy::task::TaskStatus::Claimed {
                    if let Err(e) = board.start_task(task_id) {
                        return (
                            StatusCode::BAD_REQUEST,
                            Json(ErrorResponse {
                                error: e.to_string(),
                            }),
                        )
                            .into_response();
                    }
                }

                // Submit the result
                if let Err(e) = board.submit_result(task_id, result_text.to_string()) {
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(ErrorResponse {
                            error: e.to_string(),
                        }),
                    )
                        .into_response();
                }
            }

            // Emit TaskSubmitted event (fields: task_id)
            state.event_bus.emit(WorldEvent::TaskSubmitted {
                task_id: task_id.to_string(),
            });

            Ok(serde_json::json!({ "task_id": task_id_str, "status": "submitted" }))
        }

        // ── socialize: update trust relationship with another agent ──
        "socialize" => {
            let body_map = body.params.as_object().cloned().unwrap_or_default();
            let target_id = match body_map
                .get("target_agent_id")
                .and_then(|t| t.as_str())
            {
                Some(id) => id,
                None => {
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(ErrorResponse {
                            error: "socialize requires 'target_agent_id' param".into(),
                        }),
                    )
                        .into_response();
                }
            };

            let interaction_str = body_map
                .get("interaction_type")
                .and_then(|t| t.as_str())
                .unwrap_or("Cooperation");

            // Resolve interaction type
            let interaction = match interaction_str {
                "Betrayal" => TrustInteractionType::Betrayal,
                "TradeCompleted" => TrustInteractionType::TradeCompleted,
                "TaskCompleted" => TrustInteractionType::TaskCompleted,
                "Gift" => TrustInteractionType::Gift,
                "Attack" => TrustInteractionType::Attack,
                _ => TrustInteractionType::Cooperation,
            };

            let new_trust;
            // Record the trust interaction
            if let Some(ref trust) = state.trust_network {
                let mut network = trust.lock().await;
                new_trust = network.record_interaction(&id, target_id, interaction, tick);
            } else {
                new_trust = 0.0;
            }

            // Emit TrustChanged event (fields: agent_id, other_agent_id, old_trust, new_trust, reason)
            state.event_bus.emit(WorldEvent::TrustChanged {
                agent_id: id.clone(),
                other_agent_id: target_id.to_string(),
                old_trust: 0.0, // default prior
                new_trust,
                reason: format!("{:?}", interaction),
            });

            Ok(serde_json::json!({
                "target_agent_id": target_id,
                "interaction_type": interaction_str,
                "new_trust_score": new_trust,
            }))
        }

        // ── practice_skill: increase skill XP for the actor ──
        "practice_skill" => {
            let body_map = body.params.as_object().cloned().unwrap_or_default();
            let skill_name = body_map
                .get("skill")
                .and_then(|s| s.as_str())
                .unwrap_or("general");

            // Increase skill level on the agent's own record
            let new_level;
            {
                let mut agents = state.agents.lock().await;
                if let Some(record) = agents.iter_mut().find(|a| a.id == id) {
                    let current_level = record.skills.get(skill_name).copied().unwrap_or(0);
                    new_level = current_level.saturating_add(1);
                    record.skills.insert(skill_name.to_string(), new_level);
                } else {
                    new_level = 1;
                }
            }

            // Emit SkillLevelUp event (fields: agent_id, skill, new_level)
            state.event_bus.emit(WorldEvent::SkillLevelUp {
                agent_id: id.clone(),
                skill: skill_name.to_string(),
                new_level,
            });

            Ok(serde_json::json!({
                "skill": skill_name,
                "new_level": new_level,
            }))
        }

        // ── teach_skill: teach a skill from the actor to a target agent ──
        "teach_skill" => {
            let body_map = body.params.as_object().cloned().unwrap_or_default();
            let target_id = match body_map
                .get("target_agent_id")
                .and_then(|t| t.as_str())
            {
                Some(id) => id,
                None => {
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(ErrorResponse {
                            error: "teach_skill requires 'target_agent_id' param".into(),
                        }),
                    )
                        .into_response();
                }
            };
            let skill_name = body_map
                .get("skill")
                .and_then(|s| s.as_str())
                .unwrap_or("general");

            // Read teacher's skill level from agents list
            let teacher_level = {
                let agents = state.agents.lock().await;
                agents
                    .iter()
                    .find(|a| a.id == id)
                    .and_then(|a| a.skills.get(skill_name).copied())
                    .unwrap_or(0)
            };

            if teacher_level < 2 {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        error: format!(
                            "cannot teach '{}': teacher skill level must be >= 2 (current: {})",
                            skill_name, teacher_level
                        ),
                    }),
                )
                    .into_response();
            }

            // Grant the target agent a fraction of the skill level (mentorship-style: 70% of teacher level)
            let target_level = (teacher_level as f64 * 0.7) as u32;

            {
                let mut agents = state.agents.lock().await;
                if let Some(record) = agents.iter_mut().find(|a| a.id == target_id) {
                    let current = record.skills.get(skill_name).copied().unwrap_or(0);
                    record
                        .skills
                        .insert(skill_name.to_string(), current.max(target_level));
                }
            }

            // Emit MentorshipProgress event (fields: mentor_id, apprentice_id, skill, level_gained)
            state.event_bus.emit(WorldEvent::MentorshipProgress {
                mentor_id: id.clone(),
                apprentice_id: target_id.to_string(),
                skill: skill_name.to_string(),
                level_gained: target_level,
            });

            Ok(serde_json::json!({
                "target_agent_id": target_id,
                "skill": skill_name,
                "teacher_level": teacher_level,
                "apprentice_new_level": target_level,
            }))
        }

        _ => Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: format!("unknown action '{}'", body.action),
            }),
        )),
    };

    // Emit BalanceChanged event for dashboard visibility (token changes)
    if action_cost > 0 || token_income > 0 {
        state.event_bus.emit(WorldEvent::BalanceChanged {
            agent_id: id.clone(),
            agent_name: agent_name.clone(),
            currency: crate::world::enums::Currency::Token,
            old_balance: old_tokens,
            new_balance: agent.tokens,
            tick,
        });
    }

    // Sync changes back to the shared agents list so GET /api/v1/agents reflects updates
    {
        let mut agents = state.agents.lock().await;
        if let Some(record) = agents.iter_mut().find(|a| a.id == id) {
            record.money = agent.money;
            record.tokens = agent.tokens;
            record.alive = agent.alive;
            record.phase = agent.phase.clone();
            // Increment ticks_survived on every action (each action = one tick survived)
            record.ticks_survived = record.ticks_survived.saturating_add(1);
        }
    }

    let response = match result {
        Ok(json) => json,
        Err((status, resp)) => return (status, resp).into_response(),
    };

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "action": body.action,
            "success": true,
            "tick": tick,
            "data": response,
        })),
    )
        .into_response()
}

/// Get perception data for a third-party agent.
pub async fn get_agent_perception(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let external = state.external_agents.lock().await;
    let agent = match external.get(&id) {
        Some(a) if a.alive => a,
        Some(_) => {
            return (
                StatusCode::GONE,
                Json(ErrorResponse {
                    error: "agent is dead".into(),
                }),
            )
                .into_response()
        }
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "agent not found".into(),
                }),
            )
                .into_response()
        }
    };

    let tick = *state.tick_rx.borrow();

    // Build perception from the world state
    let agents = state.agents.lock().await;
    let nearby_agents: Vec<serde_json::Value> = agents
        .iter()
        .filter(|a| a.alive && a.id != id)
        .take(10)
        .map(|a| {
            serde_json::json!({
                "id": a.id,
                "name": a.name,
            })
        })
        .collect();

    let nearby_resources: Vec<serde_json::Value> = {
        let map = state.world_map.lock().await;
        // Convert the agent's offset-position (x, y) to a HexPos for map lookup.
        // ExternalAgent.position uses offset (column, row) coordinates that match
        // the seeder's coordinate system and hex.from_offset().
        let agent_hex = HexPos::from_offset(agent.position.x as i32, agent.position.y as i32);

        // Vision radius of 2 hexes — gives a reasonable neighbourhood (19 tiles).
        const VISION_RADIUS: i32 = 2;

        // Get all positions within vision radius, then find tiles that have resources.
        agent_hex
            .ring(VISION_RADIUS)
            .iter()
            .filter_map(|pos| map.get(pos).map(|t| (pos, t)))
            .filter(|(_, tile)| !tile.resources.is_empty())
            .flat_map(|(_pos, tile)| {
                tile.resources.iter().map(move |res| {
                    let (ox, oy) = tile.pos.to_offset();
                    serde_json::json!({
                        "type": res.kind,
                        "position": { "x": ox, "y": oy },
                        "amount": res.amount,
                    })
                }).collect::<Vec<_>>()
            })
            .collect()
    };

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "agent_id": id,
            "nearby_agents": nearby_agents,
            "nearby_resources": nearby_resources,
            "position": agent.position,
            "world_tick": tick,
        })),
    )
        .into_response()
}

/// Get the status of a third-party agent.
pub async fn get_agent_status(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let external = state.external_agents.lock().await;
    let agent = match external.get(&id) {
        Some(a) => a,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "agent not found".into(),
                }),
            )
                .into_response()
        }
    };

    let tick = *state.tick_rx.borrow();

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "agent_id": agent.agent_id,
            "name": agent.name,
            "alive": agent.alive,
            "phase": agent.phase,
            "tokens": agent.tokens,
            "money": agent.money,
            "position": agent.position,
            "registered_tick": agent.registered_tick,
            "current_tick": tick,
        })),
    )
        .into_response()
}

/// Third-party agent API routes.
pub fn agents_ext_routes() -> axum::Router<AppState> {
    axum::Router::new()
        .route("/agents/register", post(register_external_agent))
        .route("/agents/:id", delete(deregister_external_agent))
        .route("/agents/:id/action", post(execute_agent_action))
        .route("/agents/:id/perception", get(get_agent_perception))
        .route("/agents/:id/status", get(get_agent_status))
}
