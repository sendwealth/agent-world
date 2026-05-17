use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use tokio::sync::Mutex;
use tonic::{Request, Response, Status};
use uuid::Uuid;

use crate::economy::token_burn::{ConsumptionConfig, TokenBurnEngine};
use crate::economy::task::TaskBoard;
use crate::world::enums::AgentPhase;
use crate::world::event::WorldEvent;
use crate::world::state::EventBus;

// Include the generated protobuf code.
pub mod proto {
    tonic::include_proto!("agentworld.engine.v1");
}

use proto::world_engine_service_server::{WorldEngineService, WorldEngineServiceServer};
use proto::*;

/// In-memory agent registry entry.
struct RegisteredAgent {
    id: String,
    name: String,
    phase: AgentPhase,
    tokens: u64,
    last_heartbeat: Instant,
    metadata: HashMap<String, String>,
}

/// Shared state backing the gRPC service.
#[derive(Clone)]
pub struct GrpcState {
    event_bus: Arc<EventBus>,
    task_board: Arc<Mutex<TaskBoard>>,
    agents: Arc<Mutex<HashMap<String, RegisteredAgent>>>,
    tick: Arc<Mutex<u64>>,
    token_engine: TokenBurnEngine,
}

impl GrpcState {
    pub fn new(
        event_bus: Arc<EventBus>,
        task_board: Arc<Mutex<TaskBoard>>,
    ) -> Self {
        Self {
            event_bus,
            task_board,
            agents: Arc::new(Mutex::new(HashMap::new())),
            tick: Arc::new(Mutex::new(0)),
            token_engine: TokenBurnEngine::with_defaults(),
        }
    }

    pub fn with_config(
        event_bus: Arc<EventBus>,
        task_board: Arc<Mutex<TaskBoard>>,
        config: ConsumptionConfig,
    ) -> Self {
        Self {
            event_bus,
            task_board,
            agents: Arc::new(Mutex::new(HashMap::new())),
            tick: Arc::new(Mutex::new(0)),
            token_engine: TokenBurnEngine::new(config),
        }
    }
}

/// The gRPC service implementation for the World Engine.
#[derive(Clone)]
pub struct GrpcServer {
    state: GrpcState,
}

impl GrpcServer {
    pub fn new(state: GrpcState) -> Self {
        Self { state }
    }

    /// Build a tonic server with this service installed.
    pub fn into_service(self) -> WorldEngineServiceServer<Self> {
        WorldEngineServiceServer::new(self)
    }

    /// Parse a phase string into `AgentPhase`. Defaults to `Adult`.
    fn parse_phase(phase: &str) -> AgentPhase {
        match phase.to_lowercase().as_str() {
            "birth" => AgentPhase::Birth,
            "childhood" => AgentPhase::Childhood,
            "elder" => AgentPhase::Elder,
            "dying" => AgentPhase::Dying,
            "dead" => AgentPhase::Dead,
            _ => AgentPhase::Adult,
        }
    }
}

#[tonic::async_trait]
impl WorldEngineService for GrpcServer {
    async fn register(
        &self,
        request: Request<RegisterRequest>,
    ) -> Result<Response<RegisterResponse>, Status> {
        let req = request.into_inner();

        if req.name.is_empty() {
            return Ok(Response::new(RegisterResponse {
                agent_id: String::new(),
                success: false,
                error: "name is required".into(),
            }));
        }

        let agent_id = if req.agent_id.is_empty() {
            Uuid::new_v4().to_string()
        } else {
            req.agent_id.clone()
        };

        let mut agents = self.state.agents.lock().await;
        if agents.contains_key(&agent_id) {
            return Ok(Response::new(RegisterResponse {
                agent_id,
                success: false,
                error: "agent already registered".into(),
            }));
        }

        agents.insert(
            agent_id.clone(),
            RegisteredAgent {
                id: agent_id.clone(),
                name: req.name.clone(),
                phase: AgentPhase::Birth,
                tokens: 0,
                last_heartbeat: Instant::now(),
                metadata: req.metadata,
            },
        );

        Ok(Response::new(RegisterResponse {
            agent_id,
            success: true,
            error: String::new(),
        }))
    }

    async fn spawn(
        &self,
        request: Request<SpawnRequest>,
    ) -> Result<Response<SpawnResponse>, Status> {
        let req = request.into_inner();

        if req.agent_id.is_empty() {
            return Ok(Response::new(SpawnResponse {
                agent_id: String::new(),
                success: false,
                error: "agent_id is required".into(),
            }));
        }

        let phase = Self::parse_phase(&req.phase);
        let initial_tokens = if req.initial_tokens == 0 { 1000 } else { req.initial_tokens };

        let mut agents = self.state.agents.lock().await;

        let (agent_id, name) = match agents.get_mut(&req.agent_id) {
            Some(agent) => {
                agent.phase = phase;
                agent.tokens = initial_tokens;
                agent.last_heartbeat = Instant::now();
                (agent.id.clone(), agent.name.clone())
            }
            None => {
                let agent_id = req.agent_id.clone();
                let name = req.agent_id.clone();
                // Auto-register if not already registered
                agents.insert(
                    agent_id.clone(),
                    RegisteredAgent {
                        id: agent_id.clone(),
                        name: name.clone(),
                        phase,
                        tokens: initial_tokens,
                        last_heartbeat: Instant::now(),
                        metadata: HashMap::new(),
                    },
                );
                (agent_id, name)
            }
        };
        drop(agents);

        // Emit spawn event
        self.state.event_bus.emit(WorldEvent::AgentSpawned {
            agent_id: agent_id.clone(),
            name,
        });

        Ok(Response::new(SpawnResponse {
            agent_id,
            success: true,
            error: String::new(),
        }))
    }

    async fn heartbeat(
        &self,
        request: Request<HeartbeatRequest>,
    ) -> Result<Response<HeartbeatResponse>, Status> {
        let req = request.into_inner();

        if req.agent_id.is_empty() {
            return Ok(Response::new(HeartbeatResponse {
                alive: false,
                server_tick: 0,
                error: "agent_id is required".into(),
            }));
        }

        let mut agents = self.state.agents.lock().await;
        let tick = *self.state.tick.lock().await;

        match agents.get_mut(&req.agent_id) {
            Some(agent) => {
                if agent.phase == AgentPhase::Dead {
                    return Ok(Response::new(HeartbeatResponse {
                        alive: false,
                        server_tick: tick,
                        error: "agent is dead".into(),
                    }));
                }

                agent.last_heartbeat = Instant::now();
                Ok(Response::new(HeartbeatResponse {
                    alive: true,
                    server_tick: tick,
                    error: String::new(),
                }))
            }
            None => Ok(Response::new(HeartbeatResponse {
                alive: false,
                server_tick: tick,
                error: "agent not found".into(),
            })),
        }
    }

    async fn submit_task(
        &self,
        request: Request<SubmitTaskRequest>,
    ) -> Result<Response<SubmitTaskResponse>, Status> {
        let req = request.into_inner();

        if req.task_id.is_empty() {
            return Ok(Response::new(SubmitTaskResponse {
                accepted: false,
                error: "task_id is required".into(),
            }));
        }
        if req.agent_id.is_empty() {
            return Ok(Response::new(SubmitTaskResponse {
                accepted: false,
                error: "agent_id is required".into(),
            }));
        }
        if req.result.is_empty() {
            return Ok(Response::new(SubmitTaskResponse {
                accepted: false,
                error: "result is required".into(),
            }));
        }

        // Verify agent exists
        {
            let agents = self.state.agents.lock().await;
            if !agents.contains_key(&req.agent_id) {
                return Ok(Response::new(SubmitTaskResponse {
                    accepted: false,
                    error: "agent not found".into(),
                }));
            }
        }

        let task_id = match Uuid::parse_str(&req.task_id) {
            Ok(id) => id,
            Err(_) => {
                return Ok(Response::new(SubmitTaskResponse {
                    accepted: false,
                    error: "invalid task_id format".into(),
                }));
            }
        };

        let mut board = self.state.task_board.lock().await;
        match board.submit_result(task_id, req.result.clone()) {
            Ok(()) => Ok(Response::new(SubmitTaskResponse {
                accepted: true,
                error: String::new(),
            })),
            Err(e) => Ok(Response::new(SubmitTaskResponse {
                accepted: false,
                error: e.to_string(),
            })),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::event::EventType;

    fn make_server() -> GrpcServer {
        let event_bus = Arc::new(EventBus::new(64));
        let task_board = Arc::new(Mutex::new(TaskBoard::with_event_bus((*event_bus).clone())));
        let state = GrpcState::new(event_bus, task_board);
        GrpcServer::new(state)
    }

    // ── Register Tests ────────────────────────────────────

    #[tokio::test]
    async fn test_register_success() {
        let server = make_server();
        let req = Request::new(RegisterRequest {
            name: "Alice".into(),
            agent_id: "agent-001".into(),
            metadata: HashMap::new(),
        });
        let resp = server.register(req).await.unwrap().into_inner();
        assert!(resp.success);
        assert_eq!(resp.agent_id, "agent-001");
        assert!(resp.error.is_empty());
    }

    #[tokio::test]
    async fn test_register_auto_assigns_id() {
        let server = make_server();
        let req = Request::new(RegisterRequest {
            name: "Bob".into(),
            agent_id: String::new(),
            metadata: HashMap::new(),
        });
        let resp = server.register(req).await.unwrap().into_inner();
        assert!(resp.success);
        assert!(!resp.agent_id.is_empty());
    }

    #[tokio::test]
    async fn test_register_empty_name_fails() {
        let server = make_server();
        let req = Request::new(RegisterRequest {
            name: String::new(),
            agent_id: String::new(),
            metadata: HashMap::new(),
        });
        let resp = server.register(req).await.unwrap().into_inner();
        assert!(!resp.success);
        assert!(resp.error.contains("name is required"));
    }

    #[tokio::test]
    async fn test_register_duplicate_fails() {
        let server = make_server();
        let req1 = Request::new(RegisterRequest {
            name: "Alice".into(),
            agent_id: "agent-001".into(),
            metadata: HashMap::new(),
        });
        let _ = server.register(req1).await.unwrap();
        let req2 = Request::new(RegisterRequest {
            name: "Alice".into(),
            agent_id: "agent-001".into(),
            metadata: HashMap::new(),
        });
        let resp = server.register(req2).await.unwrap().into_inner();
        assert!(!resp.success);
        assert!(resp.error.contains("already registered"));
    }

    // ── Spawn Tests ───────────────────────────────────────

    #[tokio::test]
    async fn test_spawn_success() {
        let server = make_server();
        let mut rx = server.state.event_bus.subscribe();

        // Register first
        let reg = Request::new(RegisterRequest {
            name: "Alice".into(),
            agent_id: "agent-001".into(),
            metadata: HashMap::new(),
        });
        server.register(reg).await.unwrap();

        let req = Request::new(SpawnRequest {
            agent_id: "agent-001".into(),
            initial_tokens: 5000,
            phase: "adult".into(),
        });
        let resp = server.spawn(req).await.unwrap().into_inner();
        assert!(resp.success);
        assert_eq!(resp.agent_id, "agent-001");

        // Verify event emitted
        let event = rx.try_recv().unwrap();
        assert_eq!(event.event_type(), EventType::AgentSpawned);
    }

    #[tokio::test]
    async fn test_spawn_auto_registers() {
        let server = make_server();
        let req = Request::new(SpawnRequest {
            agent_id: "new-agent".into(),
            initial_tokens: 1000,
            phase: "childhood".into(),
        });
        let resp = server.spawn(req).await.unwrap().into_inner();
        assert!(resp.success);
        assert_eq!(resp.agent_id, "new-agent");
    }

    #[tokio::test]
    async fn test_spawn_empty_agent_id_fails() {
        let server = make_server();
        let req = Request::new(SpawnRequest {
            agent_id: String::new(),
            initial_tokens: 1000,
            phase: "adult".into(),
        });
        let resp = server.spawn(req).await.unwrap().into_inner();
        assert!(!resp.success);
        assert!(resp.error.contains("agent_id is required"));
    }

    #[tokio::test]
    async fn test_spawn_default_tokens() {
        let server = make_server();
        let req = Request::new(SpawnRequest {
            agent_id: "agent-default".into(),
            initial_tokens: 0, // should default to 1000
            phase: "adult".into(),
        });
        let resp = server.spawn(req).await.unwrap().into_inner();
        assert!(resp.success);

        // Verify default tokens were set
        let agents = server.state.agents.lock().await;
        let agent = agents.get("agent-default").unwrap();
        assert_eq!(agent.tokens, 1000);
    }

    #[tokio::test]
    async fn test_spawn_phase_parsing() {
        let server = make_server();

        for (phase_str, expected_phase) in [
            ("birth", AgentPhase::Birth),
            ("childhood", AgentPhase::Childhood),
            ("adult", AgentPhase::Adult),
            ("elder", AgentPhase::Elder),
            ("", AgentPhase::Adult), // default
        ] {
            let agent_id = format!("agent-{}", phase_str);
            let req = Request::new(SpawnRequest {
                agent_id: agent_id.clone(),
                initial_tokens: 1000,
                phase: phase_str.to_string(),
            });
            server.spawn(req).await.unwrap();

            let agents = server.state.agents.lock().await;
            let agent = agents.get(&agent_id).unwrap();
            assert_eq!(agent.phase, expected_phase, "phase mismatch for '{}'", phase_str);
        }
    }

    // ── Heartbeat Tests ───────────────────────────────────

    #[tokio::test]
    async fn test_heartbeat_alive() {
        let server = make_server();
        // Spawn an agent first
        let spawn = Request::new(SpawnRequest {
            agent_id: "agent-001".into(),
            initial_tokens: 1000,
            phase: "adult".into(),
        });
        server.spawn(spawn).await.unwrap();

        let req = Request::new(HeartbeatRequest {
            agent_id: "agent-001".into(),
            timestamp: 0,
        });
        let resp = server.heartbeat(req).await.unwrap().into_inner();
        assert!(resp.alive);
        assert!(resp.error.is_empty());
    }

    #[tokio::test]
    async fn test_heartbeat_not_found() {
        let server = make_server();
        let req = Request::new(HeartbeatRequest {
            agent_id: "nonexistent".into(),
            timestamp: 0,
        });
        let resp = server.heartbeat(req).await.unwrap().into_inner();
        assert!(!resp.alive);
        assert!(resp.error.contains("not found"));
    }

    #[tokio::test]
    async fn test_heartbeat_empty_agent_id() {
        let server = make_server();
        let req = Request::new(HeartbeatRequest {
            agent_id: String::new(),
            timestamp: 0,
        });
        let resp = server.heartbeat(req).await.unwrap().into_inner();
        assert!(!resp.alive);
        assert!(resp.error.contains("agent_id is required"));
    }

    // ── SubmitTask Tests ──────────────────────────────────

    #[tokio::test]
    async fn test_submit_task_success() {
        let server = make_server();

        // Spawn agent
        let spawn = Request::new(SpawnRequest {
            agent_id: "agent-001".into(),
            initial_tokens: 1000,
            phase: "adult".into(),
        });
        server.spawn(spawn).await.unwrap();

        // Create a task via TaskBoard
        let task_id = {
            let mut board = server.state.task_board.lock().await;
            board.create_task("Test".into(), "desc".into(), 100, "publisher".into(), 1, None).unwrap()
        };
        // Claim and start it
        {
            let mut board = server.state.task_board.lock().await;
            board.claim_task(task_id, "agent-001".into()).unwrap();
            board.start_task(task_id).unwrap();
        }

        let req = Request::new(SubmitTaskRequest {
            task_id: task_id.to_string(),
            agent_id: "agent-001".into(),
            result: "work done".into(),
        });
        let resp = server.submit_task(req).await.unwrap().into_inner();
        assert!(resp.accepted);
    }

    #[tokio::test]
    async fn test_submit_task_agent_not_found() {
        let server = make_server();
        let req = Request::new(SubmitTaskRequest {
            task_id: "some-id".into(),
            agent_id: "ghost".into(),
            result: "work".into(),
        });
        let resp = server.submit_task(req).await.unwrap().into_inner();
        assert!(!resp.accepted);
        assert!(resp.error.contains("agent not found"));
    }

    #[tokio::test]
    async fn test_submit_task_empty_fields() {
        let server = make_server();

        // Empty task_id
        let req = Request::new(SubmitTaskRequest {
            task_id: String::new(),
            agent_id: "a".into(),
            result: "r".into(),
        });
        let resp = server.submit_task(req).await.unwrap().into_inner();
        assert!(!resp.accepted);

        // Empty agent_id
        let req = Request::new(SubmitTaskRequest {
            task_id: "t".into(),
            agent_id: String::new(),
            result: "r".into(),
        });
        let resp = server.submit_task(req).await.unwrap().into_inner();
        assert!(!resp.accepted);

        // Empty result
        let req = Request::new(SubmitTaskRequest {
            task_id: "t".into(),
            agent_id: "a".into(),
            result: String::new(),
        });
        let resp = server.submit_task(req).await.unwrap().into_inner();
        assert!(!resp.accepted);
    }

    #[tokio::test]
    async fn test_submit_task_invalid_uuid() {
        let server = make_server();
        // Spawn agent
        let spawn = Request::new(SpawnRequest {
            agent_id: "agent-001".into(),
            initial_tokens: 1000,
            phase: "adult".into(),
        });
        server.spawn(spawn).await.unwrap();

        let req = Request::new(SubmitTaskRequest {
            task_id: "not-a-uuid".into(),
            agent_id: "agent-001".into(),
            result: "work".into(),
        });
        let resp = server.submit_task(req).await.unwrap().into_inner();
        assert!(!resp.accepted);
        assert!(resp.error.contains("invalid task_id"));
    }

    // ── Integration: Full Flow ────────────────────────────

    #[tokio::test]
    async fn test_full_grpc_flow_register_spawn_heartbeat_submit() {
        let server = make_server();
        let mut rx = server.state.event_bus.subscribe();

        // 1. Register
        let reg = Request::new(RegisterRequest {
            name: "TestAgent".into(),
            agent_id: "flow-agent".into(),
            metadata: HashMap::new(),
        });
        let reg_resp = server.register(reg).await.unwrap().into_inner();
        assert!(reg_resp.success);

        // 2. Spawn
        let spawn = Request::new(SpawnRequest {
            agent_id: "flow-agent".into(),
            initial_tokens: 10_000,
            phase: "adult".into(),
        });
        let spawn_resp = server.spawn(spawn).await.unwrap().into_inner();
        assert!(spawn_resp.success);

        // Verify AgentSpawned event
        let event = rx.try_recv().unwrap();
        assert!(matches!(event, WorldEvent::AgentSpawned { ref agent_id, .. } if agent_id == "flow-agent"));

        // 3. Heartbeat
        let hb = Request::new(HeartbeatRequest {
            agent_id: "flow-agent".into(),
            timestamp: 12345,
        });
        let hb_resp = server.heartbeat(hb).await.unwrap().into_inner();
        assert!(hb_resp.alive);

        // 4. Create task and submit
        let task_id = {
            let mut board = server.state.task_board.lock().await;
            let id = board.create_task("Task".into(), "desc".into(), 500, "pub".into(), 1, None).unwrap();
            board.claim_task(id, "flow-agent".into()).unwrap();
            board.start_task(id).unwrap();
            id
        };

        let submit = Request::new(SubmitTaskRequest {
            task_id: task_id.to_string(),
            agent_id: "flow-agent".into(),
            result: "completed".into(),
        });
        let submit_resp = server.submit_task(submit).await.unwrap().into_inner();
        assert!(submit_resp.accepted);

        // Verify task status
        let board = server.state.task_board.lock().await;
        let task = board.get(task_id).unwrap();
        assert_eq!(task.status, crate::economy::task::TaskStatus::Submitted);
    }
}
