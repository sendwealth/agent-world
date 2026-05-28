//! Marketplace Integration Tests — [SEN-50]
//!
//! Validates the complete marketplace stack end-to-end:
//!   1. Publish → Claim → Submit → Complete → Reward Distribution full flow
//!   2. Expired task escrow refund verification
//!   3. Escrow consistency across the full lifecycle
//!   4. Reputation and XP update verification

use std::sync::Arc;

use tokio::sync::RwLock;
use uuid::Uuid;

use agent_world_engine::economy::{
    RewardConfig, RewardDistribution, TaskBoard, TaskStatus, TransactionType,
};
use agent_world_engine::world::enums::Currency;

// ══════════════════════════════════════════════════════════════════════════
// Helper: create a TaskBoard wired up with RewardDistributor + balances
// ══════════════════════════════════════════════════════════════════════════

fn make_marketplace() -> TaskBoard {
    let mut board = TaskBoard::with_reward_distributor(RewardConfig::default());
    // Publisher balance lives on TaskBoard (used for escrow locking/refund)
    board.set_balance("publisher", 10_000);
    // Worker balances live on RewardDistributor (used for reward payout)
    board
        .reward_distributor_mut()
        .unwrap()
        .set_balance("worker_a", 1_000);
    board
        .reward_distributor_mut()
        .unwrap()
        .set_balance("worker_b", 1_000);
    board
}

/// Drive a single task through the full lifecycle and return the RewardDistribution.
fn complete_task_flow(
    board: &mut TaskBoard,
    publisher: &str,
    worker: &str,
    title: &str,
    reward: u64,
    created_tick: u64,
    expires_at: Option<u64>,
) -> (Uuid, Option<RewardDistribution>) {
    let id = board
        .create_task(
            title.to_string(),
            format!("Description for {}", title),
            reward,
            publisher.to_string(),
            created_tick,
            expires_at,
        )
        .unwrap();

    board.claim_task(id, worker.to_string()).unwrap();
    board.start_task(id).unwrap();
    board
        .submit_result(id, format!("Result for {}", title))
        .unwrap();
    board.review_task(id, publisher, true).unwrap();
    let dist = board.complete_task(id, 10).unwrap();

    (id, dist)
}

// ══════════════════════════════════════════════════════════════════════════
// TEST 1: Full Lifecycle with Reward Distribution
// ══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_full_lifecycle_with_reward_distribution() {
    let board = Arc::new(RwLock::new(make_marketplace()));

    let (task_id, dist) = {
        let mut b = board.write().await;
        complete_task_flow(
            &mut b,
            "publisher",
            "worker_a",
            "Build Widget",
            1000,
            1,
            None,
        )
    };

    // ── Verify task status ──────────────────────────────────────────────

    {
        let b = board.read().await;
        let task = b.get(task_id).unwrap();
        assert_eq!(task.status, TaskStatus::Completed);
        assert!(!task.escrow_held);
        assert_eq!(task.reward, 1000);
        assert_eq!(task.assignee_id.as_deref(), Some("worker_a"));
    }

    // ── Verify RewardDistribution result ────────────────────────────────

    let dist = dist.unwrap();
    assert_eq!(dist.gross_reward, 1000);
    assert_eq!(dist.platform_fee, 20); // 2% of 1000
    assert_eq!(dist.net_reward, 980);
    assert_eq!(dist.xp_awarded, 50);
    assert_eq!(dist.reputation_change, 2.0);

    // Conservation invariant: gross = net + fee
    assert_eq!(dist.gross_reward, dist.net_reward + dist.platform_fee);

    // ── Verify escrow deducted from publisher ───────────────────────────

    {
        let b = board.read().await;
        // Publisher: 10000 - 1000 (escrow) = 9000
        // Escrow is consumed by reward distribution (paid to worker via RewardDistributor)
        assert_eq!(b.get_balance("publisher"), 9_000);
    }

    // ── Verify reward paid to worker via RewardDistributor ──────────────

    {
        let b = board.read().await;
        let rd = b.reward_distributor().unwrap();
        // Worker gets net reward: 1000 + 980 = 1980
        assert_eq!(rd.get_balance("worker_a"), 1_980);
    }

    // ── Verify XP awarded ───────────────────────────────────────────────

    {
        let b = board.read().await;
        let rd = b.reward_distributor().unwrap();
        assert_eq!(rd.get_experience("worker_a"), 50);
    }

    // ── Verify reputation updated ───────────────────────────────────────

    {
        let b = board.read().await;
        let rd = b.reward_distributor().unwrap();
        assert_eq!(rd.get_reputation("worker_a"), 2.0);
    }

    // ── Verify ledger entries ───────────────────────────────────────────

    {
        let b = board.read().await;
        let rd = b.reward_distributor().unwrap();
        let ledger = rd.ledger();
        let all = ledger.list();
        assert_eq!(all.len(), 2); // reward + fee

        // Reward entry
        let reward_entry = &all[0];
        assert_eq!(reward_entry.tx_type, TransactionType::TaskReward);
        assert_eq!(reward_entry.amount, 980);
        assert_eq!(reward_entry.to_agent.as_deref(), Some("worker_a"));
        assert_eq!(
            reward_entry.reference_id.as_deref(),
            Some(task_id.to_string().as_str())
        );

        // Fee entry
        let fee_entry = &all[1];
        assert_eq!(fee_entry.tx_type, TransactionType::PlatformFee);
        assert_eq!(fee_entry.amount, 20);
        assert_eq!(fee_entry.from_agent.as_deref(), Some("worker_a"));
    }

    // ── Verify central bank collected fees ──────────────────────────────

    {
        let b = board.read().await;
        let rd = b.reward_distributor().unwrap();
        assert_eq!(rd.central_bank().total_fees(Currency::Money), 20);
    }
}

// ══════════════════════════════════════════════════════════════════════════
// TEST 2: Multiple Tasks — Accumulating Rewards, XP, Reputation
// ══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_multiple_tasks_accumulate_rewards() {
    let board = Arc::new(RwLock::new(make_marketplace()));

    let tasks = vec![
        ("Gather Resources", "worker_a", 500u64),
        ("Build Shelter", "worker_a", 1000u64),
        ("Research Tech", "worker_b", 750u64),
    ];

    let mut task_ids: Vec<Uuid> = Vec::new();
    let mut total_gross: u64 = 0;
    let mut total_fee: u64 = 0;

    for (title, worker, reward) in &tasks {
        let (id, dist) = {
            let mut b = board.write().await;
            complete_task_flow(&mut b, "publisher", worker, title, *reward, 1, None)
        };
        task_ids.push(id);
        let d = dist.unwrap();
        total_gross += d.gross_reward;
        total_fee += d.platform_fee;
    }

    // ── Verify publisher balance ────────────────────────────────────────

    {
        let b = board.read().await;
        // Publisher: 10000 - 500 - 1000 - 750 = 7750
        assert_eq!(b.get_balance("publisher"), 7_750);
    }

    // ── Verify worker balances in RewardDistributor ─────────────────────

    {
        let b = board.read().await;
        let rd = b.reward_distributor().unwrap();
        // worker_a: 1000 + (500-10) + (1000-20) = 1000 + 490 + 980 = 2470
        assert_eq!(rd.get_balance("worker_a"), 2_470);
        // worker_b: 1000 + (750-15) = 1000 + 735 = 1735
        assert_eq!(rd.get_balance("worker_b"), 1_735);
    }

    // ── Verify XP accumulation ──────────────────────────────────────────

    {
        let b = board.read().await;
        let rd = b.reward_distributor().unwrap();
        // worker_a completed 2 tasks: 50 + 50 = 100
        assert_eq!(rd.get_experience("worker_a"), 100);
        // worker_b completed 1 task: 50
        assert_eq!(rd.get_experience("worker_b"), 50);
    }

    // ── Verify reputation accumulation ──────────────────────────────────

    {
        let b = board.read().await;
        let rd = b.reward_distributor().unwrap();
        // worker_a: 2.0 + 2.0 = 4.0
        assert_eq!(rd.get_reputation("worker_a"), 4.0);
        // worker_b: 2.0
        assert_eq!(rd.get_reputation("worker_b"), 2.0);
    }

    // ── Verify ledger entries ───────────────────────────────────────────

    {
        let b = board.read().await;
        let rd = b.reward_distributor().unwrap();
        let ledger = rd.ledger();
        // 3 tasks × 2 entries = 6
        assert_eq!(ledger.list().len(), 6);

        let reward_entries = ledger.list_by_type(TransactionType::TaskReward);
        let fee_entries = ledger.list_by_type(TransactionType::PlatformFee);
        assert_eq!(reward_entries.len(), 3);
        assert_eq!(fee_entries.len(), 3);

        // Cross-check: total rewards + total fees = total gross
        let total_reward_paid: u64 = reward_entries.iter().map(|e| e.amount).sum();
        let total_fee_collected: u64 = fee_entries.iter().map(|e| e.amount).sum();
        assert_eq!(total_reward_paid + total_fee_collected, total_gross);
    }

    // ── Verify central bank ─────────────────────────────────────────────

    {
        let b = board.read().await;
        let rd = b.reward_distributor().unwrap();
        // 10 + 20 + 15 = 45
        assert_eq!(rd.central_bank().total_fees(Currency::Money), total_fee);
    }
}

// ══════════════════════════════════════════════════════════════════════════
// TEST 3: Expired Task — Escrow Refund to Publisher
// ══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_expired_task_escrow_refund() {
    let mut board = make_marketplace();

    // Create a task with escrow
    let task_id = board
        .create_task(
            "Expiring Task".to_string(),
            "This task will expire".to_string(),
            500,
            "publisher".to_string(),
            1,
            Some(100),
        )
        .unwrap();

    // Verify escrow deducted from publisher
    assert_eq!(board.get_balance("publisher"), 9_500);

    // Expire the task
    board.expire_task(task_id).unwrap();

    // ── Verify status ───────────────────────────────────────────────────

    let task = board.get(task_id).unwrap();
    assert_eq!(task.status, TaskStatus::Expired);
    assert!(!task.escrow_held);

    // ── Verify escrow refunded ──────────────────────────────────────────

    assert_eq!(board.get_balance("publisher"), 10_000);

    // ── Verify no reward distribution occurred ──────────────────────────

    let rd = board.reward_distributor().unwrap();
    assert_eq!(rd.get_balance("worker_a"), 1_000); // unchanged
    assert_eq!(rd.get_experience("worker_a"), 0); // unchanged
    assert_eq!(rd.get_reputation("worker_a"), 0.0); // unchanged
    assert_eq!(rd.ledger().list().len(), 0); // no ledger entries
    assert_eq!(rd.central_bank().total_fees(Currency::Money), 0); // no fees
}

// ══════════════════════════════════════════════════════════════════════════
// TEST 4: Expired Claimed Task — Escrow Refund
// ══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_expired_claimed_task_escrow_refund() {
    let mut board = make_marketplace();

    let task_id = board
        .create_task(
            "Claimed then Expired".to_string(),
            "Worker claimed but it expired".to_string(),
            800,
            "publisher".to_string(),
            1,
            Some(100),
        )
        .unwrap();

    assert_eq!(board.get_balance("publisher"), 9_200);

    // Worker claims the task
    board.claim_task(task_id, "worker_a".to_string()).unwrap();
    assert_eq!(board.get(task_id).unwrap().status, TaskStatus::Claimed);

    // Task expires while claimed
    board.expire_task(task_id).unwrap();

    let task = board.get(task_id).unwrap();
    assert_eq!(task.status, TaskStatus::Expired);
    assert!(!task.escrow_held);

    // Escrow returned to publisher, not to the worker
    assert_eq!(board.get_balance("publisher"), 10_000);

    // Worker gets nothing
    let rd = board.reward_distributor().unwrap();
    assert_eq!(rd.get_balance("worker_a"), 1_000);
    assert_eq!(rd.get_reputation("worker_a"), 0.0);
}

// ══════════════════════════════════════════════════════════════════════════
// TEST 5: Batch Expiry with Escrow Consistency
// ══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_batch_expiry_escrow_consistency() {
    let mut board = make_marketplace();

    // Create 5 tasks with different expiry times
    let id1 = board
        .create_task("T1".into(), "".into(), 100, "publisher".into(), 1, Some(10))
        .unwrap();
    let id2 = board
        .create_task("T2".into(), "".into(), 200, "publisher".into(), 1, Some(10))
        .unwrap();
    let id3 = board
        .create_task(
            "T3".into(),
            "".into(),
            300,
            "publisher".into(),
            1,
            Some(100),
        )
        .unwrap();
    let id4 = board
        .create_task(
            "T4".into(),
            "".into(),
            400,
            "publisher".into(),
            1,
            Some(100),
        )
        .unwrap();
    let id5 = board
        .create_task(
            "T5".into(),
            "".into(),
            500,
            "publisher".into(),
            1,
            Some(200),
        )
        .unwrap();

    // Publisher: 10000 - 100 - 200 - 300 - 400 - 500 = 8500
    assert_eq!(board.get_balance("publisher"), 8_500);

    // Process expiry at tick 50 — should expire T1 and T2
    let expired = board.process_expiry(50);
    assert_eq!(expired.len(), 2);
    assert!(expired.contains(&id1));
    assert!(expired.contains(&id2));

    // Refund: 100 + 200 = 300 → publisher balance = 8500 + 300 = 8800
    assert_eq!(board.get_balance("publisher"), 8_800);

    // Complete T3 normally (should not expire)
    board.claim_task(id3, "worker_a".to_string()).unwrap();
    board.start_task(id3).unwrap();
    board.submit_result(id3, "Done".into()).unwrap();
    board.review_task(id3, "publisher", true).unwrap();
    let dist = board.complete_task(id3, 10).unwrap().unwrap();

    // T3 reward = 300, fee = 6, net = 294
    assert_eq!(dist.gross_reward, 300);
    assert_eq!(dist.platform_fee, 6);
    assert_eq!(dist.net_reward, 294);

    // Publisher balance unchanged by completion (escrow already deducted)
    assert_eq!(board.get_balance("publisher"), 8_800);

    // Process expiry at tick 150 — should expire T4
    let expired = board.process_expiry(150);
    assert_eq!(expired.len(), 1);
    assert!(expired.contains(&id4));

    // Refund: 400 → publisher = 8800 + 400 = 9200
    assert_eq!(board.get_balance("publisher"), 9_200);

    // T5 still active
    assert_eq!(board.get(id5).unwrap().status, TaskStatus::Published);

    // Complete T5 at tick 160
    board.claim_task(id5, "worker_b".to_string()).unwrap();
    board.start_task(id5).unwrap();
    board.submit_result(id5, "Done too".into()).unwrap();
    board.review_task(id5, "publisher", true).unwrap();
    board.complete_task(id5, 10).unwrap();

    // ── Final escrow consistency ────────────────────────────────────────

    // Total created: 100+200+300+400+500 = 1500
    // Expired refund: 100+200+400 = 700
    // Completed (consumed): 300+500 = 800
    // Total accounted: 700 + 800 = 1500 ✓
    let final_publisher_balance = board.get_balance("publisher");
    assert_eq!(final_publisher_balance, 10_000 - 300 - 500);
    // publisher: 10000 - 300 (T3 consumed) - 500 (T5 consumed) = 9200
    // Wait, T4 expired and refunded, so:
    // 10000 - 100(T1 refund) - 200(T2 refund) - 300(T3 consumed) - 400(T4 refund) - 500(T5 consumed)
    // = 10000 - 300 - 500 = 9200
    assert_eq!(final_publisher_balance, 9_200);

    // ── Verify worker rewards via RewardDistributor ─────────────────────

    let rd = board.reward_distributor().unwrap();
    // worker_a completed T3: 1000 + 294 = 1294
    assert_eq!(rd.get_balance("worker_a"), 1_294);
    // worker_b completed T5: 1000 + 490 = 1490
    assert_eq!(rd.get_balance("worker_b"), 1_490);

    // ── Ledger: only completed tasks generate entries (2 per task) ──────

    let ledger = rd.ledger();
    assert_eq!(ledger.list().len(), 4); // 2 completed tasks × 2 entries

    // ── Central bank: fees from completed tasks only ────────────────────

    assert_eq!(rd.central_bank().total_fees(Currency::Money), 6 + 10);
    // T3 fee: 6, T5 fee: 10 = 16
}

// ══════════════════════════════════════════════════════════════════════════
// TEST 6: Escrow Consistency — Money-In == Money-Out
// ══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_escrow_consistency_money_in_equals_money_out() {
    let mut board = make_marketplace();

    let tasks = vec![
        ("Task A", "worker_a", 1000u64),
        ("Task B", "worker_b", 2000u64),
        ("Task C", "worker_a", 500u64),
    ];

    let total_escrowed: u64 = tasks.iter().map(|t| t.2).sum();

    // Create all tasks
    let mut task_ids: Vec<Uuid> = Vec::new();
    for (title, _worker, reward) in &tasks {
        let id = board
            .create_task(
                title.to_string(),
                "".to_string(),
                *reward,
                "publisher".to_string(),
                1,
                None,
            )
            .unwrap();
        task_ids.push(id);
    }

    // Publisher balance after escrow: 10000 - 3500 = 6500
    assert_eq!(board.get_balance("publisher"), 10_000 - total_escrowed);

    // Complete all tasks
    let mut distributions: Vec<RewardDistribution> = Vec::new();
    for ((title, worker, _reward), task_id) in tasks.iter().zip(&task_ids) {
        board.claim_task(*task_id, worker.to_string()).unwrap();
        board.start_task(*task_id).unwrap();
        board
            .submit_result(*task_id, format!("Result for {}", title))
            .unwrap();
        board.review_task(*task_id, "publisher", true).unwrap();
        let dist = board.complete_task(*task_id, 10).unwrap().unwrap();
        distributions.push(dist);
    }

    // ── Conservation: total escrowed == total net rewards + total fees ──

    let total_net: u64 = distributions.iter().map(|d| d.net_reward).sum();
    let total_fees: u64 = distributions.iter().map(|d| d.platform_fee).sum();

    assert_eq!(
        total_net + total_fees,
        total_escrowed,
        "Conservation: total_escrowed ({}) must equal total_net ({}) + total_fees ({})",
        total_escrowed,
        total_net,
        total_fees
    );

    // ── Per-distribution conservation ───────────────────────────────────

    for dist in &distributions {
        assert_eq!(
            dist.gross_reward,
            dist.net_reward + dist.platform_fee,
            "Per-task conservation violated for gross={}",
            dist.gross_reward
        );
    }

    // ── Ledger cross-check ──────────────────────────────────────────────

    let rd = board.reward_distributor().unwrap();
    let ledger = rd.ledger();

    let reward_sum: u64 = ledger
        .list_by_type(TransactionType::TaskReward)
        .iter()
        .map(|e| e.amount)
        .sum();
    let fee_sum: u64 = ledger
        .list_by_type(TransactionType::PlatformFee)
        .iter()
        .map(|e| e.amount)
        .sum();

    assert_eq!(reward_sum, total_net);
    assert_eq!(fee_sum, total_fees);
    assert_eq!(reward_sum + fee_sum, total_escrowed);
}

// ══════════════════════════════════════════════════════════════════════════
// TEST 7: Reputation and XP Update Verification
// ══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_reputation_and_xp_verification() {
    let mut board = make_marketplace();

    // worker_a completes 3 tasks, worker_b completes 1 task
    let flows = vec![
        ("Task 1", "worker_a", 100u64),
        ("Task 2", "worker_a", 200u64),
        ("Task 3", "worker_a", 300u64),
        ("Task 4", "worker_b", 400u64),
    ];

    for (title, worker, reward) in &flows {
        complete_task_flow(&mut board, "publisher", worker, title, *reward, 1, None);
    }

    let rd = board.reward_distributor().unwrap();

    // ── XP: 50 per task completion ──────────────────────────────────────

    assert_eq!(rd.get_experience("worker_a"), 150); // 3 × 50
    assert_eq!(rd.get_experience("worker_b"), 50); // 1 × 50
    assert_eq!(rd.get_experience("publisher"), 0); // publisher gets no XP

    // ── Reputation: +2.0 per task completion ────────────────────────────

    assert_eq!(rd.get_reputation("worker_a"), 6.0); // 3 × 2.0
    assert_eq!(rd.get_reputation("worker_b"), 2.0); // 1 × 2.0
    assert_eq!(rd.get_reputation("publisher"), 0.0); // publisher gets no reputation

    // ── Reputation is per-agent, not per-task ───────────────────────────

    // Check that no unknown agents accumulated reputation
    assert_eq!(rd.get_reputation("nonexistent"), 0.0);

    // ── Verify worker balances match rewards ────────────────────────────

    // worker_a: 1000 + (100-2) + (200-4) + (300-6) = 1000 + 98 + 196 + 294 = 1588
    assert_eq!(rd.get_balance("worker_a"), 1_588);
    // worker_b: 1000 + (400-8) = 1000 + 392 = 1392
    assert_eq!(rd.get_balance("worker_b"), 1_392);
}

// ══════════════════════════════════════════════════════════════════════════
// TEST 8: Review Rejection + Resubmit — Reward Only on Approval
// ══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_review_rejection_resubmit_reward_on_approval() {
    let mut board = make_marketplace();

    let task_id = board
        .create_task(
            "Iterative Task".to_string(),
            "Requires multiple submissions".to_string(),
            500,
            "publisher".to_string(),
            1,
            None,
        )
        .unwrap();

    assert_eq!(board.get_balance("publisher"), 9_500);

    // Worker claims and starts
    board.claim_task(task_id, "worker_a".to_string()).unwrap();
    board.start_task(task_id).unwrap();

    // First submission — rejected
    board
        .submit_result(task_id, "Bad quality".to_string())
        .unwrap();
    assert_eq!(board.get(task_id).unwrap().status, TaskStatus::Submitted);

    board.review_task(task_id, "publisher", false).unwrap();
    assert_eq!(board.get(task_id).unwrap().status, TaskStatus::InProgress);

    // No reward distributed yet
    let rd = board.reward_distributor().unwrap();
    assert_eq!(rd.get_balance("worker_a"), 1_000);
    assert_eq!(rd.get_reputation("worker_a"), 0.0);
    assert_eq!(rd.get_experience("worker_a"), 0);

    // Second submission — approved
    board
        .submit_result(task_id, "Good quality".to_string())
        .unwrap();
    board.review_task(task_id, "publisher", true).unwrap();
    assert_eq!(board.get(task_id).unwrap().status, TaskStatus::Reviewed);

    // Complete — now reward is distributed
    let dist = board.complete_task(task_id, 10).unwrap().unwrap();

    assert_eq!(dist.gross_reward, 500);
    assert_eq!(dist.platform_fee, 10); // 2% of 500
    assert_eq!(dist.net_reward, 490);

    let rd = board.reward_distributor().unwrap();
    assert_eq!(rd.get_balance("worker_a"), 1_490);
    assert_eq!(rd.get_reputation("worker_a"), 2.0); // only 1 task completion
    assert_eq!(rd.get_experience("worker_a"), 50);
    assert_eq!(rd.ledger().list().len(), 2); // only 2 entries, not 4
}

// ══════════════════════════════════════════════════════════════════════════
// TEST 9: Zero-Reward Task — No Escrow, No Fee, XP Still Awarded
// ══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_zero_reward_task_xp_still_awarded() {
    let mut board = make_marketplace();

    let task_id = board
        .create_task(
            "Volunteer Task".to_string(),
            "No monetary reward".to_string(),
            0,
            "publisher".to_string(),
            1,
            None,
        )
        .unwrap();

    // No escrow held
    assert!(!board.get(task_id).unwrap().escrow_held);
    assert_eq!(board.get_balance("publisher"), 10_000); // unchanged

    // Full lifecycle
    board.claim_task(task_id, "worker_a".to_string()).unwrap();
    board.start_task(task_id).unwrap();
    board
        .submit_result(task_id, "Volunteer work done".to_string())
        .unwrap();
    board.review_task(task_id, "publisher", true).unwrap();

    let dist = board.complete_task(task_id, 10).unwrap().unwrap();

    // Zero reward but still processed
    assert_eq!(dist.gross_reward, 0);
    assert_eq!(dist.platform_fee, 0);
    assert_eq!(dist.net_reward, 0);

    // XP and reputation still awarded
    assert_eq!(dist.xp_awarded, 50);
    assert_eq!(dist.reputation_change, 2.0);

    let rd = board.reward_distributor().unwrap();
    assert_eq!(rd.get_experience("worker_a"), 50);
    assert_eq!(rd.get_reputation("worker_a"), 2.0);
    assert_eq!(rd.get_balance("worker_a"), 1_000); // no reward added
}

// ══════════════════════════════════════════════════════════════════════════
// TEST 10: Mixed Flow — Completed + Expired + Escrow Final Reconciliation
// ══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_mixed_completed_expired_escrow_reconciliation() {
    let mut board = make_marketplace();

    // Scenario: 5 tasks, 3 completed, 2 expired
    // Total escrow: 200 + 300 + 500 + 400 + 600 = 2000

    // ── Task 1: will be completed ───────────────────────────────────────
    let t1 = board
        .create_task(
            "Complete 1".into(),
            "".into(),
            200,
            "publisher".into(),
            1,
            None,
        )
        .unwrap();

    // ── Task 2: will expire while published ─────────────────────────────
    let t2 = board
        .create_task(
            "Expire Pub".into(),
            "".into(),
            300,
            "publisher".into(),
            1,
            Some(50),
        )
        .unwrap();

    // ── Task 3: will be completed ───────────────────────────────────────
    let t3 = board
        .create_task(
            "Complete 2".into(),
            "".into(),
            500,
            "publisher".into(),
            1,
            None,
        )
        .unwrap();

    // ── Task 4: will be claimed then expired ────────────────────────────
    let t4 = board
        .create_task(
            "Expire Claimed".into(),
            "".into(),
            400,
            "publisher".into(),
            1,
            Some(50),
        )
        .unwrap();

    // ── Task 5: will be completed ───────────────────────────────────────
    let t5 = board
        .create_task(
            "Complete 3".into(),
            "".into(),
            600,
            "publisher".into(),
            1,
            None,
        )
        .unwrap();

    // Total escrowed: 2000
    assert_eq!(board.get_balance("publisher"), 10_000 - 2_000);

    // ── Complete tasks 1, 3, 5 ──────────────────────────────────────────

    for &id in &[t1, t3, t5] {
        board.claim_task(id, "worker_a".to_string()).unwrap();
        board.start_task(id).unwrap();
        board.submit_result(id, "Result".to_string()).unwrap();
        board.review_task(id, "publisher", true).unwrap();
        board.complete_task(id, 10).unwrap();
    }

    // ── Expire tasks 2 and 4 ────────────────────────────────────────────

    // Claim t4 first, then expire
    board.claim_task(t4, "worker_b".to_string()).unwrap();
    assert_eq!(board.get(t4).unwrap().status, TaskStatus::Claimed);

    let expired = board.process_expiry(100);
    assert_eq!(expired.len(), 2);
    assert!(expired.contains(&t2));
    assert!(expired.contains(&t4));

    // ── Final state verification ────────────────────────────────────────

    assert_eq!(board.get(t1).unwrap().status, TaskStatus::Completed);
    assert_eq!(board.get(t2).unwrap().status, TaskStatus::Expired);
    assert_eq!(board.get(t3).unwrap().status, TaskStatus::Completed);
    assert_eq!(board.get(t4).unwrap().status, TaskStatus::Expired);
    assert_eq!(board.get(t5).unwrap().status, TaskStatus::Completed);

    // ── Escrow reconciliation ───────────────────────────────────────────
    //
    // Total escrowed: 2000
    // Expired + refunded: 300 + 400 = 700
    // Completed (consumed by reward distribution): 200 + 500 + 600 = 1300
    // Total accounted: 700 + 1300 = 2000 ✓

    let publisher_balance = board.get_balance("publisher");
    // Publisher balance: 10000 - (consumed by completed tasks) = 10000 - 1300 = 8700
    assert_eq!(publisher_balance, 8_700);

    // ── Reward distribution for completed tasks only ────────────────────

    let rd = board.reward_distributor().unwrap();
    let ledger = rd.ledger();
    let reward_entries = ledger.list_by_type(TransactionType::TaskReward);
    let fee_entries = ledger.list_by_type(TransactionType::PlatformFee);

    // 3 completed tasks × 2 entries = 6
    assert_eq!(ledger.list().len(), 6);
    assert_eq!(reward_entries.len(), 3);
    assert_eq!(fee_entries.len(), 3);

    // Net rewards: (200-4) + (500-10) + (600-12) = 196 + 490 + 588 = 1274
    let total_net: u64 = reward_entries.iter().map(|e| e.amount).sum();
    assert_eq!(total_net, 1_274);

    // Fees: 4 + 10 + 12 = 26
    let total_fee: u64 = fee_entries.iter().map(|e| e.amount).sum();
    assert_eq!(total_fee, 26);

    // Conservation: net + fees = total completed escrow
    assert_eq!(total_net + total_fee, 1_300);

    // ── Worker balances ─────────────────────────────────────────────────

    // worker_a completed 3 tasks: 1000 + 1274 = 2274
    assert_eq!(rd.get_balance("worker_a"), 2_274);
    // worker_b claimed but task expired: no reward
    assert_eq!(rd.get_balance("worker_b"), 1_000);

    // ── Reputation: only completed tasks count ──────────────────────────

    assert_eq!(rd.get_reputation("worker_a"), 6.0); // 3 tasks × 2.0
    assert_eq!(rd.get_reputation("worker_b"), 0.0); // expired, no reward

    // ── XP ──────────────────────────────────────────────────────────────

    assert_eq!(rd.get_experience("worker_a"), 150); // 3 × 50
    assert_eq!(rd.get_experience("worker_b"), 0);

    // ── Central bank ────────────────────────────────────────────────────

    assert_eq!(rd.central_bank().total_fees(Currency::Money), 26);
}
