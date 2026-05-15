use std::collections::HashSet;
use std::sync::Arc;

use super::event::{EventType, WorldEvent};

/// Broadcasts world events to all registered subscribers.
///
/// Uses `tokio::sync::broadcast` for async-friendly fan-out.
/// Each subscriber gets its own receiver, allowing independent consumption.
pub struct EventBus {
    sender: tokio::sync::broadcast::Sender<WorldEvent>,
}

impl EventBus {
    /// Create a new event bus with the given channel capacity.
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = tokio::sync::broadcast::channel(capacity);
        Self { sender }
    }

    /// Broadcast an event to all subscribers (fire-and-forget).
    ///
    /// If no subscribers exist, the event is silently dropped.
    pub fn publish(&self, event: WorldEvent) {
        let _ = self.sender.send(event);
    }

    /// Broadcast an event to all subscribers.
    pub fn emit(&self, event: WorldEvent) {
        let _ = self.sender.send(event);
    }

    /// Subscribe to all events. Returns a receiver that will receive every
    /// event emitted after this call.
    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<WorldEvent> {
        self.sender.subscribe()
    }

    /// Subscribe to events filtered by event types and/or agent ID.
    ///
    /// Returns a [`FilteredReceiver`] that yields only matching events.
    /// - If `types` is empty, all event types pass the filter.
    /// - If `agent_id` is `None`, no agent filtering is applied.
    pub fn subscribe_filtered(
        &self,
        types: Vec<EventType>,
        agent_id: Option<String>,
    ) -> FilteredReceiver {
        FilteredReceiver {
            rx: self.sender.subscribe(),
            types: types.into_iter().collect(),
            agent_id,
        }
    }
}

/// A wrapper around a broadcast receiver that filters events by type and/or agent ID.
pub struct FilteredReceiver {
    rx: tokio::sync::broadcast::Receiver<WorldEvent>,
    types: HashSet<EventType>,
    agent_id: Option<String>,
}

impl FilteredReceiver {
    /// Attempt to receive the next matching event without blocking.
    pub fn try_recv(&mut self) -> Result<WorldEvent, tokio::sync::broadcast::error::TryRecvError> {
        loop {
            match self.rx.try_recv() {
                Ok(event) => {
                    if self.matches(&event) {
                        return Ok(event);
                    }
                }
                Err(e) => return Err(e),
            }
        }
    }

    /// Receive the next matching event asynchronously.
    pub async fn recv(&mut self) -> Result<WorldEvent, tokio::sync::broadcast::error::RecvError> {
        loop {
            match self.rx.recv().await {
                Ok(event) => {
                    if self.matches(&event) {
                        return Ok(event);
                    }
                }
                Err(e) => return Err(e),
            }
        }
    }

    fn matches(&self, event: &WorldEvent) -> bool {
        if !self.types.is_empty() && !self.types.contains(&event.event_type()) {
            return false;
        }
        if let Some(ref filter_id) = self.agent_id {
            match event.agent_id() {
                Some(aid) if aid == filter_id => {}
                _ => return false,
            }
        }
        true
    }
}

/// Shared reference-counted event bus.
pub type SharedEventBus = Arc<EventBus>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::enums::{AgentPhase, Currency, DeathReason};

    #[tokio::test]
    async fn event_bus_emit_and_receive() {
        let bus = EventBus::new(16);
        let mut rx = bus.subscribe();

        bus.emit(WorldEvent::TickAdvanced { tick: 1 });
        bus.emit(WorldEvent::TickAdvanced { tick: 2 });

        let evt1 = rx.try_recv().unwrap();
        assert_eq!(evt1, WorldEvent::TickAdvanced { tick: 1 });
        let evt2 = rx.try_recv().unwrap();
        assert_eq!(evt2, WorldEvent::TickAdvanced { tick: 2 });
    }

    #[tokio::test]
    async fn event_bus_no_subscribers() {
        let bus = EventBus::new(16);
        bus.emit(WorldEvent::TickAdvanced { tick: 1 });
    }

    #[tokio::test]
    async fn event_bus_publish_alias() {
        let bus = EventBus::new(16);
        let mut rx = bus.subscribe();

        bus.publish(WorldEvent::TickAdvanced { tick: 42 });
        let evt = rx.try_recv().unwrap();
        assert_eq!(evt, WorldEvent::TickAdvanced { tick: 42 });
    }

    #[tokio::test]
    async fn event_bus_agent_died_event() {
        let bus = EventBus::new(16);
        let mut rx = bus.subscribe();

        bus.emit(WorldEvent::AgentDied {
            agent_id: "agent-001".into(),
            reason: DeathReason::TokenDepleted,
        });

        let evt = rx.try_recv().unwrap();
        assert_eq!(
            evt,
            WorldEvent::AgentDied {
                agent_id: "agent-001".into(),
                reason: DeathReason::TokenDepleted,
            }
        );
    }

    #[tokio::test]
    async fn event_bus_multiple_subscribers_receive_same_events() {
        let bus = EventBus::new(16);
        let mut rx1 = bus.subscribe();
        let mut rx2 = bus.subscribe();
        let mut rx3 = bus.subscribe();

        bus.emit(WorldEvent::TickAdvanced { tick: 1 });
        bus.emit(WorldEvent::AgentSpawned {
            agent_id: "a1".into(),
            name: "Alice".into(),
        });

        for rx in [&mut rx1, &mut rx2, &mut rx3] {
            let tick_evt = rx.try_recv().unwrap();
            assert_eq!(tick_evt, WorldEvent::TickAdvanced { tick: 1 });
            let spawn_evt = rx.try_recv().unwrap();
            assert_eq!(
                spawn_evt,
                WorldEvent::AgentSpawned {
                    agent_id: "a1".into(),
                    name: "Alice".into(),
                }
            );
        }
    }

    #[tokio::test]
    async fn event_bus_subscriber_joined_later_misses_events() {
        let bus = EventBus::new(16);
        bus.emit(WorldEvent::TickAdvanced { tick: 1 });
        let mut rx = bus.subscribe();
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn event_bus_filter_by_type() {
        let bus = EventBus::new(64);
        let mut rx = bus.subscribe_filtered(
            vec![EventType::AgentDied, EventType::AgentRescued],
            None,
        );

        bus.emit(WorldEvent::TickAdvanced { tick: 1 });
        bus.emit(WorldEvent::AgentDied {
            agent_id: "a1".into(),
            reason: DeathReason::TokenDepleted,
        });
        bus.emit(WorldEvent::TickAdvanced { tick: 2 });
        bus.emit(WorldEvent::AgentRescued {
            agent_id: "a2".into(),
        });

        let evt1 = rx.try_recv().unwrap();
        assert_eq!(
            evt1,
            WorldEvent::AgentDied {
                agent_id: "a1".into(),
                reason: DeathReason::TokenDepleted,
            }
        );
        let evt2 = rx.try_recv().unwrap();
        assert_eq!(
            evt2,
            WorldEvent::AgentRescued {
                agent_id: "a2".into(),
            }
        );
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn event_bus_filter_by_agent_id() {
        let bus = EventBus::new(64);
        let mut rx = bus.subscribe_filtered(vec![], Some("agent-001".into()));

        bus.emit(WorldEvent::AgentSpawned {
            agent_id: "agent-001".into(),
            name: "Alice".into(),
        });
        bus.emit(WorldEvent::AgentSpawned {
            agent_id: "agent-002".into(),
            name: "Bob".into(),
        });
        bus.emit(WorldEvent::PhaseChanged {
            agent_id: "agent-001".into(),
            old_phase: AgentPhase::Childhood,
            new_phase: AgentPhase::Adult,
        });
        bus.emit(WorldEvent::TickAdvanced { tick: 1 });

        let evt1 = rx.try_recv().unwrap();
        assert_eq!(
            evt1,
            WorldEvent::AgentSpawned {
                agent_id: "agent-001".into(),
                name: "Alice".into(),
            }
        );
        let evt2 = rx.try_recv().unwrap();
        assert_eq!(
            evt2,
            WorldEvent::PhaseChanged {
                agent_id: "agent-001".into(),
                old_phase: AgentPhase::Childhood,
                new_phase: AgentPhase::Adult,
            }
        );
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn event_bus_filter_by_type_and_agent_id() {
        let bus = EventBus::new(64);
        let mut rx = bus.subscribe_filtered(
            vec![EventType::PhaseChanged],
            Some("agent-001".into()),
        );

        bus.emit(WorldEvent::PhaseChanged {
            agent_id: "agent-001".into(),
            old_phase: AgentPhase::Childhood,
            new_phase: AgentPhase::Adult,
        });
        bus.emit(WorldEvent::PhaseChanged {
            agent_id: "agent-002".into(),
            old_phase: AgentPhase::Childhood,
            new_phase: AgentPhase::Adult,
        });
        bus.emit(WorldEvent::AgentDied {
            agent_id: "agent-001".into(),
            reason: DeathReason::TokenDepleted,
        });

        let evt = rx.try_recv().unwrap();
        assert_eq!(
            evt,
            WorldEvent::PhaseChanged {
                agent_id: "agent-001".into(),
                old_phase: AgentPhase::Childhood,
                new_phase: AgentPhase::Adult,
            }
        );
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn event_bus_filter_no_filters_gets_everything() {
        let bus = EventBus::new(64);
        let mut rx = bus.subscribe_filtered(vec![], None);

        bus.emit(WorldEvent::TickAdvanced { tick: 1 });
        bus.emit(WorldEvent::AgentDied {
            agent_id: "a1".into(),
            reason: DeathReason::TokenDepleted,
        });

        let _ = rx.try_recv().unwrap();
        let _ = rx.try_recv().unwrap();
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn event_bus_filtered_recv_async() {
        let bus = Arc::new(EventBus::new(64));
        let bus_clone = bus.clone();

        let mut rx = bus.subscribe_filtered(vec![EventType::TickAdvanced], None);

        let handle = tokio::spawn(async move {
            bus_clone.emit(WorldEvent::AgentDied {
                agent_id: "a1".into(),
                reason: DeathReason::TokenDepleted,
            });
            bus_clone.emit(WorldEvent::TickAdvanced { tick: 42 });
        });

        let evt = rx.recv().await.unwrap();
        assert_eq!(evt, WorldEvent::TickAdvanced { tick: 42 });

        handle.await.unwrap();
    }

    #[tokio::test]
    async fn event_bus_multiple_filtered_subscribers() {
        let bus = EventBus::new(64);

        let mut rx_a = bus.subscribe_filtered(vec![EventType::AgentDied], None);
        let mut rx_b = bus.subscribe_filtered(vec![EventType::TickAdvanced], None);
        let mut rx_c = bus.subscribe();

        bus.emit(WorldEvent::TickAdvanced { tick: 1 });
        bus.emit(WorldEvent::AgentDied {
            agent_id: "a1".into(),
            reason: DeathReason::TokenDepleted,
        });
        bus.emit(WorldEvent::AgentRescued {
            agent_id: "a2".into(),
        });

        let evt_a = rx_a.try_recv().unwrap();
        assert_eq!(evt_a.event_type(), EventType::AgentDied);
        assert!(rx_a.try_recv().is_err());

        let evt_b = rx_b.try_recv().unwrap();
        assert_eq!(evt_b.event_type(), EventType::TickAdvanced);
        assert!(rx_b.try_recv().is_err());

        let _ = rx_c.try_recv().unwrap();
        let _ = rx_c.try_recv().unwrap();
        let _ = rx_c.try_recv().unwrap();
        assert!(rx_c.try_recv().is_err());
    }

    #[tokio::test]
    async fn event_bus_event_json_roundtrip_via_bus() {
        let bus = EventBus::new(16);
        let mut rx = bus.subscribe();

        let event = WorldEvent::TransactionCompleted {
            from: "a1".into(),
            to: "a2".into(),
            amount: 500,
            currency: Currency::Token,
        };
        bus.emit(event);

        let received = rx.try_recv().unwrap();
        let json = received.to_json();
        let deserialized = WorldEvent::from_json(&json).unwrap();
        assert_eq!(received, deserialized);
    }

    #[tokio::test]
    async fn shared_event_bus_works_across_tasks() {
        let bus: SharedEventBus = Arc::new(EventBus::new(64));
        let bus1 = bus.clone();
        let bus2 = bus.clone();

        let mut rx = bus.subscribe();

        let handle = tokio::spawn(async move {
            bus1.emit(WorldEvent::TickAdvanced { tick: 1 });
        });

        bus2.emit(WorldEvent::TickAdvanced { tick: 2 });

        handle.await.unwrap();

        let evt1 = rx.try_recv().unwrap();
        let evt2 = rx.try_recv().unwrap();

        assert!(matches!(evt1, WorldEvent::TickAdvanced { .. }));
        assert!(matches!(evt2, WorldEvent::TickAdvanced { .. }));
    }
}
