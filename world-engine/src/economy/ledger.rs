//! Double-entry bookkeeping ledger with central bank exchange, WAL persistence,
//! balance queries, and audit logging.
//!
//! Design:
//! - Every transaction creates **two** ledger entries (debit + credit) that must balance.
//! - `Account` represents a balance holder (agents, central bank, fee pool).
//! - `ExchangeRate` configures the Token↔Money conversion.
//! - All mutations are recorded to a WAL for crash recovery.
//! - Audit trail captures who/what/when/why for every state change.

use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::wal::WAL;
use crate::world::enums::Currency;
use crate::world::event::WorldEvent;

use super::reward::TransactionType;

// ── Account ──────────────────────────────────────────────

/// An account in the double-entry ledger.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub account_type: AccountType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccountType {
    Agent,
    CentralBank,
    FeePool,
}

impl Account {
    pub fn new_agent(id: &str, name: &str) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            account_type: AccountType::Agent,
        }
    }

    pub fn central_bank() -> Self {
        Self {
            id: "central_bank".to_string(),
            name: "Central Bank".to_string(),
            account_type: AccountType::CentralBank,
        }
    }

    pub fn fee_pool() -> Self {
        Self {
            id: "fee_pool".to_string(),
            name: "Fee Pool".to_string(),
            account_type: AccountType::FeePool,
        }
    }
}

// ── Exchange Rate ────────────────────────────────────────

/// Configuration for Token↔Money conversion at the central bank.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExchangeRate {
    /// How many Tokens equal 1 Money (from genesis: token_price = 100).
    pub tokens_per_money: u64,
}

impl Default for ExchangeRate {
    fn default() -> Self {
        Self {
            tokens_per_money: 100,
        }
    }
}

impl ExchangeRate {
    pub fn new(tokens_per_money: u64) -> Self {
        assert!(tokens_per_money > 0, "tokens_per_money must be > 0");
        Self { tokens_per_money }
    }

    /// Convert Money → Token. 1 Money = `tokens_per_money` Tokens.
    /// Returns `None` if `money_amount` overflows.
    pub fn money_to_tokens(&self, money_amount: u64) -> Option<u64> {
        (money_amount as u128 * self.tokens_per_money as u128)
            .try_into()
            .ok()
    }

    /// Convert Token → Money. Rounds down (truncates).
    /// Returns `None` if the result would be 0 (insufficient tokens for 1 Money).
    pub fn tokens_to_money(&self, token_amount: u64) -> Option<u64> {
        if token_amount < self.tokens_per_money {
            return None;
        }
        Some(token_amount / self.tokens_per_money)
    }
}

// ── Double-Entry Pair ────────────────────────────────────

/// A single leg of a double-entry pair (debit or credit).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    pub id: Uuid,
    /// The paired entry (debit ↔ credit share the same pair_id).
    pub pair_id: Uuid,
    pub account_id: String,
    pub side: EntrySide,
    pub amount: u64,
    pub currency: Currency,
    pub tx_type: TransactionType,
    pub description: String,
    pub tick: u64,
    pub reference_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntrySide {
    Debit,
    Credit,
}

// ── Audit Log ────────────────────────────────────────────

/// An audit record capturing the full context of a ledger operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditRecord {
    pub id: Uuid,
    pub tick: u64,
    pub operation: String,
    pub actor: String,
    pub details: serde_json::Value,
    pub entry_ids: Vec<Uuid>,
}

// ── Errors ───────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum LedgerError {
    #[error("account not found: {0}")]
    AccountNotFound(String),
    #[error(
        "insufficient balance: account {account} has {available} {currency:?}, needs {required}"
    )]
    InsufficientBalance {
        account: String,
        available: u64,
        required: u64,
        currency: Currency,
    },
    #[error("invalid exchange: {0}")]
    InvalidExchange(String),
    #[error("WAL error: {0}")]
    Wal(String),
    #[error("pair does not balance: debit={debit} credit={credit}")]
    Unbalanced { debit: u64, credit: u64 },
}

// ── Balance Sheet ────────────────────────────────────────

/// A snapshot of an account's balances.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalanceSheet {
    pub account_id: String,
    pub balances: HashMap<Currency, u64>,
}

// ── Money Ledger (Double-Entry) ──────────────────────────

/// Double-entry bookkeeping ledger with central bank exchange capabilities.
///
/// Invariant: for every pair, total debits == total credits (in the same currency).
pub struct MoneyLedger {
    accounts: HashMap<String, Account>,
    /// Running balance per (account, currency).
    balances: HashMap<(String, Currency), u64>,
    entries: Vec<Entry>,
    audit_log: Vec<AuditRecord>,
    exchange_rate: ExchangeRate,
    wal: Option<WAL>,
    /// Index of the last entry that was persisted to the WAL.
    last_persisted_idx: usize,
}

impl MoneyLedger {
    /// Create a new ledger with the given exchange rate and no WAL.
    pub fn new(exchange_rate: ExchangeRate) -> Self {
        let mut ledger = Self {
            accounts: HashMap::new(),
            balances: HashMap::new(),
            entries: Vec::new(),
            audit_log: Vec::new(),
            exchange_rate,
            wal: None,
            last_persisted_idx: 0,
        };
        // Bootstrap system accounts
        let cb = Account::central_bank();
        ledger.accounts.insert(cb.id.clone(), cb);
        let fp = Account::fee_pool();
        ledger.accounts.insert(fp.id.clone(), fp);
        ledger
    }

    /// Create a new ledger with WAL persistence.
    pub fn with_wal<P: AsRef<Path>>(
        exchange_rate: ExchangeRate,
        data_dir: P,
    ) -> Result<Self, LedgerError> {
        let mut ledger = Self::new(exchange_rate);
        let mut wal = WAL::new(data_dir);
        wal.open().map_err(|e| LedgerError::Wal(e.to_string()))?;
        ledger.wal = Some(wal);
        Ok(ledger)
    }

    // ── Account Management ───────────────────────────────

    /// Register a new agent account.
    pub fn create_account(&mut self, account: Account) {
        self.accounts.insert(account.id.clone(), account);
    }

    /// Check if an account exists.
    pub fn account_exists(&self, account_id: &str) -> bool {
        self.accounts.contains_key(account_id)
    }

    /// Get account info.
    pub fn get_account(&self, account_id: &str) -> Option<&Account> {
        self.accounts.get(account_id)
    }

    // ── Balance Queries ──────────────────────────────────

    /// Get the balance of an account in a specific currency.
    pub fn get_balance(&self, account_id: &str, currency: Currency) -> u64 {
        self.balances
            .get(&(account_id.to_string(), currency))
            .copied()
            .unwrap_or(0)
    }

    /// Get the full balance sheet for an account.
    pub fn get_balance_sheet(&self, account_id: &str) -> BalanceSheet {
        let mut balances = HashMap::new();
        for curr in [Currency::Token, Currency::Money] {
            let bal = self.get_balance(account_id, curr);
            if bal > 0 {
                balances.insert(curr, bal);
            }
        }
        BalanceSheet {
            account_id: account_id.to_string(),
            balances,
        }
    }

    /// Set the initial balance for an account (genesis / restore only).
    ///
    /// This bypasses double-entry recording and should only be used for
    /// initial minting or crash recovery. An audit record is always created.
    pub(crate) fn set_balance(
        &mut self,
        account_id: &str,
        currency: Currency,
        amount: u64,
        tick: u64,
    ) {
        self.balances
            .insert((account_id.to_string(), currency), amount);
        self.record_audit(
            "set_balance",
            account_id,
            serde_json::json!({
                "currency": format!("{:?}", currency).to_lowercase(),
                "amount": amount,
                "note": "genesis/restore — bypasses double-entry",
            }),
            &[],
            tick,
        );
    }

    /// Public convenience for setting balance at tick 0 (genesis).
    /// Delegates to `set_balance` with tick=0.
    pub fn set_balance_genesis(&mut self, account_id: &str, currency: Currency, amount: u64) {
        self.set_balance(account_id, currency, amount, 0);
    }

    /// Credit (increase) an account balance.
    fn credit(&mut self, account_id: &str, currency: Currency, amount: u64) {
        let key = (account_id.to_string(), currency);
        *self.balances.entry(key).or_insert(0) += amount;
    }

    /// Debit (decrease) an account balance. Returns error if insufficient.
    /// The central bank is exempt from balance checks — it is the currency issuer.
    fn debit(
        &mut self,
        account_id: &str,
        currency: Currency,
        amount: u64,
    ) -> Result<(), LedgerError> {
        let key = (account_id.to_string(), currency);
        if account_id == "central_bank" {
            // Central bank can issue unlimited currency; balance goes negative tracked as u64 wrap
            // Instead, we just track the balance normally (it can be any value)
            let current = self.balances.get(&key).copied().unwrap_or(0);
            self.balances.insert(key, current.saturating_sub(amount));
            return Ok(());
        }
        let current = self.balances.get(&key).copied().unwrap_or(0);
        if current < amount {
            return Err(LedgerError::InsufficientBalance {
                account: account_id.to_string(),
                available: current,
                required: amount,
                currency,
            });
        }
        self.balances.insert(key, current - amount);
        Ok(())
    }

    // ── Double-Entry Transfer ─────────────────────────────

    /// Execute a double-entry transfer between two accounts.
    ///
    /// Creates a debit entry on `from` and a credit entry on `to`.
    /// Both entries share the same `pair_id` to form a balanced pair.
    #[allow(clippy::too_many_arguments)]
    pub fn transfer(
        &mut self,
        from: &str,
        to: &str,
        amount: u64,
        currency: Currency,
        tx_type: TransactionType,
        description: String,
        tick: u64,
        reference_id: Option<String>,
    ) -> Result<(Uuid, Uuid), LedgerError> {
        if amount == 0 {
            // Zero-amount transfers are no-ops
            let pair_id = Uuid::new_v4();
            return Ok((pair_id, pair_id));
        }

        // Validate both accounts exist before mutating state
        self.require_account(from)?;
        self.require_account(to)?;

        let pair_id = Uuid::new_v4();

        // Debit the sender
        self.debit(from, currency, amount)?;
        let debit_id = Uuid::new_v4();
        let debit_entry = Entry {
            id: debit_id,
            pair_id,
            account_id: from.to_string(),
            side: EntrySide::Debit,
            amount,
            currency,
            tx_type,
            description: description.clone(),
            tick,
            reference_id: reference_id.clone(),
        };

        // Credit the receiver
        self.credit(to, currency, amount);
        let credit_id = Uuid::new_v4();
        let credit_entry = Entry {
            id: credit_id,
            pair_id,
            account_id: to.to_string(),
            side: EntrySide::Credit,
            amount,
            currency,
            tx_type,
            description,
            tick,
            reference_id,
        };

        self.entries.push(debit_entry);
        self.entries.push(credit_entry);

        // Persist to WAL
        self.persist_entries();

        // Verify invariant
        debug_assert!(self.verify_pair(pair_id, currency));

        Ok((debit_id, credit_id))
    }

    // ── Central Bank Exchange ─────────────────────────────

    /// Exchange Money → Token at the central bank.
    ///
    /// Creates two balanced pairs (4 entries total):
    ///   Pair 1: agent debit Money + central_bank credit Money
    ///   Pair 2: central_bank debit Token + agent credit Token
    pub fn exchange_money_to_tokens(
        &mut self,
        agent_id: &str,
        money_amount: u64,
        tick: u64,
    ) -> Result<ExchangeResult, LedgerError> {
        self.require_account(agent_id)?;

        let token_amount = self
            .exchange_rate
            .money_to_tokens(money_amount)
            .ok_or_else(|| LedgerError::InvalidExchange("overflow in conversion".into()))?;

        if token_amount == 0 {
            return Err(LedgerError::InvalidExchange("zero token result".into()));
        }

        let cb = "central_bank";
        let pair1_id = Uuid::new_v4(); // Money pair
        let pair2_id = Uuid::new_v4(); // Token pair

        // ── Pair 1: Money leg (agent → central bank) ──
        self.debit(agent_id, Currency::Money, money_amount)?;
        let d1 = Uuid::new_v4();
        self.entries.push(Entry {
            id: d1,
            pair_id: pair1_id,
            account_id: agent_id.to_string(),
            side: EntrySide::Debit,
            amount: money_amount,
            currency: Currency::Money,
            tx_type: TransactionType::Exchange,
            description: format!("Exchange {} Money → {} Tokens", money_amount, token_amount),
            tick,
            reference_id: None,
        });
        self.credit(cb, Currency::Money, money_amount);
        let c1 = Uuid::new_v4();
        self.entries.push(Entry {
            id: c1,
            pair_id: pair1_id,
            account_id: cb.to_string(),
            side: EntrySide::Credit,
            amount: money_amount,
            currency: Currency::Money,
            tx_type: TransactionType::Exchange,
            description: format!(
                "Central bank receives {} Money for Token exchange",
                money_amount
            ),
            tick,
            reference_id: None,
        });

        // ── Pair 2: Token leg (central bank → agent) ──
        self.debit(cb, Currency::Token, token_amount)?;
        let d2 = Uuid::new_v4();
        self.entries.push(Entry {
            id: d2,
            pair_id: pair2_id,
            account_id: cb.to_string(),
            side: EntrySide::Debit,
            amount: token_amount,
            currency: Currency::Token,
            tx_type: TransactionType::Exchange,
            description: format!(
                "Central bank issues {} Tokens for Money exchange",
                token_amount
            ),
            tick,
            reference_id: None,
        });
        self.credit(agent_id, Currency::Token, token_amount);
        let c2 = Uuid::new_v4();
        self.entries.push(Entry {
            id: c2,
            pair_id: pair2_id,
            account_id: agent_id.to_string(),
            side: EntrySide::Credit,
            amount: token_amount,
            currency: Currency::Token,
            tx_type: TransactionType::Exchange,
            description: format!(
                "Received {} Tokens for {} Money",
                token_amount, money_amount
            ),
            tick,
            reference_id: None,
        });

        self.persist_entries();
        self.record_audit(
            "exchange_money_to_tokens",
            agent_id,
            serde_json::json!({
                "money_amount": money_amount,
                "token_amount": token_amount,
                "rate": self.exchange_rate.tokens_per_money,
            }),
            &[d1, c1, d2, c2],
            tick,
        );

        debug_assert!(self.verify_pair(pair1_id, Currency::Money));
        debug_assert!(self.verify_pair(pair2_id, Currency::Token));

        Ok(ExchangeResult {
            pair_id: pair1_id,
            from_amount: money_amount,
            from_currency: Currency::Money,
            to_amount: token_amount,
            to_currency: Currency::Token,
        })
    }

    /// Exchange Token → Money at the central bank.
    ///
    /// Creates two balanced pairs (4 entries total):
    ///   Pair 1: agent debit Token + central_bank credit Token
    ///   Pair 2: central_bank debit Money + agent credit Money
    ///
    /// Tokens are rounded down (any remainder stays in agent's Token balance).
    pub fn exchange_tokens_to_money(
        &mut self,
        agent_id: &str,
        token_amount: u64,
        tick: u64,
    ) -> Result<ExchangeResult, LedgerError> {
        self.require_account(agent_id)?;

        let money_amount = self
            .exchange_rate
            .tokens_to_money(token_amount)
            .ok_or_else(|| {
                LedgerError::InvalidExchange(format!(
                    "insufficient tokens: {} < minimum {}",
                    token_amount, self.exchange_rate.tokens_per_money
                ))
            })?;

        // Tokens actually consumed (may be less than token_amount due to rounding)
        let tokens_consumed = money_amount * self.exchange_rate.tokens_per_money;
        let tokens_returned = token_amount - tokens_consumed;

        let cb = "central_bank";
        let pair1_id = Uuid::new_v4(); // Token pair
        let pair2_id = Uuid::new_v4(); // Money pair

        // ── Pair 1: Token leg (agent → central bank) ──
        self.debit(agent_id, Currency::Token, tokens_consumed)?;
        let d1 = Uuid::new_v4();
        self.entries.push(Entry {
            id: d1,
            pair_id: pair1_id,
            account_id: agent_id.to_string(),
            side: EntrySide::Debit,
            amount: tokens_consumed,
            currency: Currency::Token,
            tx_type: TransactionType::Exchange,
            description: format!(
                "Exchange {} Tokens → {} Money",
                tokens_consumed, money_amount
            ),
            tick,
            reference_id: None,
        });
        self.credit(cb, Currency::Token, tokens_consumed);
        let c1 = Uuid::new_v4();
        self.entries.push(Entry {
            id: c1,
            pair_id: pair1_id,
            account_id: cb.to_string(),
            side: EntrySide::Credit,
            amount: tokens_consumed,
            currency: Currency::Token,
            tx_type: TransactionType::Exchange,
            description: format!(
                "Central bank receives {} Tokens for Money exchange",
                tokens_consumed
            ),
            tick,
            reference_id: None,
        });

        // ── Pair 2: Money leg (central bank → agent) ──
        self.debit(cb, Currency::Money, money_amount)?;
        let d2 = Uuid::new_v4();
        self.entries.push(Entry {
            id: d2,
            pair_id: pair2_id,
            account_id: cb.to_string(),
            side: EntrySide::Debit,
            amount: money_amount,
            currency: Currency::Money,
            tx_type: TransactionType::Exchange,
            description: format!(
                "Central bank issues {} Money for Token exchange",
                money_amount
            ),
            tick,
            reference_id: None,
        });
        self.credit(agent_id, Currency::Money, money_amount);
        let c2 = Uuid::new_v4();
        self.entries.push(Entry {
            id: c2,
            pair_id: pair2_id,
            account_id: agent_id.to_string(),
            side: EntrySide::Credit,
            amount: money_amount,
            currency: Currency::Money,
            tx_type: TransactionType::Exchange,
            description: format!(
                "Received {} Money for {} Tokens",
                money_amount, tokens_consumed
            ),
            tick,
            reference_id: None,
        });

        self.persist_entries();
        self.record_audit(
            "exchange_tokens_to_money",
            agent_id,
            serde_json::json!({
                "token_amount": token_amount,
                "tokens_consumed": tokens_consumed,
                "tokens_returned": tokens_returned,
                "money_amount": money_amount,
                "rate": self.exchange_rate.tokens_per_money,
            }),
            &[d1, c1, d2, c2],
            tick,
        );

        debug_assert!(self.verify_pair(pair1_id, Currency::Token));
        debug_assert!(self.verify_pair(pair2_id, Currency::Money));

        Ok(ExchangeResult {
            pair_id: pair1_id,
            from_amount: tokens_consumed,
            from_currency: Currency::Token,
            to_amount: money_amount,
            to_currency: Currency::Money,
        })
    }

    // ── Interest ─────────────────────────────────────────

    /// Pay interest on an agent's Money balance.
    ///
    /// Creates a balanced pair: central bank debit Money + agent credit Money.
    /// Interest rate is per-tick (e.g. 0.001 = 0.1% per tick).
    pub fn pay_interest(
        &mut self,
        agent_id: &str,
        rate: f64,
        tick: u64,
    ) -> Result<Option<InterestResult>, LedgerError> {
        self.require_account(agent_id)?;

        let balance = self.get_balance(agent_id, Currency::Money);
        if balance == 0 {
            return Ok(None);
        }

        let interest = ((balance as f64) * rate) as u64;
        if interest == 0 {
            return Ok(None);
        }

        let cb = "central_bank";
        let pair_id = Uuid::new_v4();

        // Central bank debits Money (pays interest)
        self.debit(cb, Currency::Money, interest)?;
        let d1 = Uuid::new_v4();
        self.entries.push(Entry {
            id: d1,
            pair_id,
            account_id: cb.to_string(),
            side: EntrySide::Debit,
            amount: interest,
            currency: Currency::Money,
            tx_type: TransactionType::Interest,
            description: format!(
                "Central bank pays {} interest on {} Money balance",
                interest, balance
            ),
            tick,
            reference_id: None,
        });

        // Agent credits Money (receives interest)
        self.credit(agent_id, Currency::Money, interest);
        let c1 = Uuid::new_v4();
        self.entries.push(Entry {
            id: c1,
            pair_id,
            account_id: agent_id.to_string(),
            side: EntrySide::Credit,
            amount: interest,
            currency: Currency::Money,
            tx_type: TransactionType::Interest,
            description: format!("Interest on {} Money balance at rate {}", balance, rate),
            tick,
            reference_id: None,
        });

        self.persist_entries();
        self.record_audit(
            "pay_interest",
            agent_id,
            serde_json::json!({
                "principal": balance,
                "rate": rate,
                "interest": interest,
            }),
            &[d1, c1],
            tick,
        );

        debug_assert!(self.verify_pair(pair_id, Currency::Money));

        Ok(Some(InterestResult {
            pair_id,
            principal: balance,
            interest,
            new_balance: self.get_balance(agent_id, Currency::Money),
        }))
    }

    // ── Queries ──────────────────────────────────────────

    /// Get all entries.
    pub fn list_entries(&self) -> &[Entry] {
        &self.entries
    }

    /// Get entries for a specific account.
    pub fn entries_by_account(&self, account_id: &str) -> Vec<&Entry> {
        self.entries
            .iter()
            .filter(|e| e.account_id == account_id)
            .collect()
    }

    /// Get entries for a specific pair.
    pub fn entries_by_pair(&self, pair_id: Uuid) -> Vec<&Entry> {
        self.entries
            .iter()
            .filter(|e| e.pair_id == pair_id)
            .collect()
    }

    /// Get entries by transaction type.
    pub fn entries_by_type(&self, tx_type: TransactionType) -> Vec<&Entry> {
        self.entries
            .iter()
            .filter(|e| e.tx_type == tx_type)
            .collect()
    }

    /// Get entries by reference ID.
    pub fn entries_by_reference(&self, reference_id: &str) -> Vec<&Entry> {
        self.entries
            .iter()
            .filter(|e| e.reference_id.as_deref() == Some(reference_id))
            .collect()
    }

    /// Get all audit records.
    pub fn audit_log(&self) -> &[AuditRecord] {
        &self.audit_log
    }

    /// Get audit records for a specific actor.
    pub fn audit_by_actor(&self, actor: &str) -> Vec<&AuditRecord> {
        self.audit_log.iter().filter(|a| a.actor == actor).collect()
    }

    /// Get the exchange rate configuration.
    pub fn exchange_rate(&self) -> &ExchangeRate {
        &self.exchange_rate
    }

    // ── Verification ─────────────────────────────────────

    /// Verify that a specific pair balances (total debits == total credits).
    pub fn verify_pair(&self, pair_id: Uuid, currency: Currency) -> bool {
        let mut debit_total: u64 = 0;
        let mut credit_total: u64 = 0;
        for entry in &self.entries {
            if entry.pair_id == pair_id && entry.currency == currency {
                match entry.side {
                    EntrySide::Debit => debit_total += entry.amount,
                    EntrySide::Credit => credit_total += entry.amount,
                }
            }
        }
        debit_total == credit_total
    }

    /// Verify the entire ledger balances (all debits == all credits per currency).
    pub fn verify_all(&self) -> bool {
        let mut totals: HashMap<Currency, (u64, u64)> = HashMap::new();
        for entry in &self.entries {
            let (debit, credit) = totals.entry(entry.currency).or_insert((0, 0));
            match entry.side {
                EntrySide::Debit => *debit += entry.amount,
                EntrySide::Credit => *credit += entry.amount,
            }
        }
        totals.values().all(|(d, c)| d == c)
    }

    /// Compute total money supply (sum of all Money balances).
    pub fn total_money_supply(&self) -> u64 {
        self.balances
            .iter()
            .filter(|((_, curr), _)| *curr == Currency::Money)
            .map(|(_, &v)| v)
            .sum()
    }

    /// Compute total token supply (sum of all Token balances).
    pub fn total_token_supply(&self) -> u64 {
        self.balances
            .iter()
            .filter(|((_, curr), _)| *curr == Currency::Token)
            .map(|(_, &v)| v)
            .sum()
    }

    // ── Internal Helpers ─────────────────────────────────

    fn require_account(&self, account_id: &str) -> Result<(), LedgerError> {
        if !self.accounts.contains_key(account_id) {
            return Err(LedgerError::AccountNotFound(account_id.to_string()));
        }
        Ok(())
    }

    fn record_audit(
        &mut self,
        operation: &str,
        actor: &str,
        details: serde_json::Value,
        entry_ids: &[Uuid],
        tick: u64,
    ) {
        self.audit_log.push(AuditRecord {
            id: Uuid::new_v4(),
            tick,
            operation: operation.to_string(),
            actor: actor.to_string(),
            details,
            entry_ids: entry_ids.to_vec(),
        });
    }

    fn persist_entries(&mut self) {
        if self.wal.is_none() {
            return;
        }
        // Collect snapshot data first to avoid borrow conflicts.
        let start = self.last_persisted_idx;
        let events: Vec<WorldEvent> = self.entries[start..]
            .iter()
            .map(|entry| {
                let new_balance = self.get_balance(&entry.account_id, entry.currency);
                let old_balance = match entry.side {
                    EntrySide::Credit => new_balance.saturating_sub(entry.amount),
                    EntrySide::Debit => new_balance.saturating_add(entry.amount),
                };
                WorldEvent::BalanceChanged {
                    agent_id: entry.account_id.clone(),
                    agent_name: String::new(),
                    currency: entry.currency,
                    old_balance,
                    new_balance,
                    tick: entry.tick,
                }
            })
            .collect();
        if let Some(ref mut wal) = self.wal {
            for event in events {
                if let Err(e) = wal.append_event(&event) {
                    eprintln!("[Ledger] WAL write failed: {}", e);
                }
            }
        }
        self.last_persisted_idx = self.entries.len();
    }
}

// ── Result Types ─────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExchangeResult {
    pub pair_id: Uuid,
    pub from_amount: u64,
    pub from_currency: Currency,
    pub to_amount: u64,
    pub to_currency: Currency,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterestResult {
    pub pair_id: Uuid,
    pub principal: u64,
    pub interest: u64,
    pub new_balance: u64,
}

// ── Tests ────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_ledger() -> MoneyLedger {
        let mut ledger = MoneyLedger::new(ExchangeRate::default());
        ledger.create_account(Account::new_agent("alice", "Alice"));
        ledger.create_account(Account::new_agent("bob", "Bob"));
        ledger
    }

    // ── Account Management ───────────────────────────────

    #[test]
    fn test_create_account() {
        let ledger = make_ledger();
        assert!(ledger.account_exists("alice"));
        assert!(ledger.account_exists("bob"));
        assert!(ledger.account_exists("central_bank"));
        assert!(ledger.account_exists("fee_pool"));
        assert!(!ledger.account_exists("unknown"));
    }

    #[test]
    fn test_system_accounts_bootstrapped() {
        let ledger = MoneyLedger::new(ExchangeRate::default());
        assert!(ledger.account_exists("central_bank"));
        assert!(ledger.account_exists("fee_pool"));
        assert_eq!(
            ledger.get_account("central_bank").unwrap().account_type,
            AccountType::CentralBank
        );
        assert_eq!(
            ledger.get_account("fee_pool").unwrap().account_type,
            AccountType::FeePool
        );
    }

    // ── Balance Queries ──────────────────────────────────

    #[test]
    fn test_initial_balance_is_zero() {
        let ledger = make_ledger();
        assert_eq!(ledger.get_balance("alice", Currency::Money), 0);
        assert_eq!(ledger.get_balance("alice", Currency::Token), 0);
    }

    #[test]
    fn test_set_and_get_balance() {
        let mut ledger = make_ledger();
        ledger.set_balance_genesis("alice", Currency::Money, 500);
        ledger.set_balance_genesis("alice", Currency::Token, 1000);
        assert_eq!(ledger.get_balance("alice", Currency::Money), 500);
        assert_eq!(ledger.get_balance("alice", Currency::Token), 1000);
        // Genesis set_balance creates audit records
        assert_eq!(ledger.audit_log().len(), 2);
    }

    #[test]
    fn test_balance_sheet() {
        let mut ledger = make_ledger();
        ledger.set_balance_genesis("alice", Currency::Money, 100);
        ledger.set_balance_genesis("alice", Currency::Token, 500);
        let sheet = ledger.get_balance_sheet("alice");
        assert_eq!(sheet.balances.get(&Currency::Money), Some(&100));
        assert_eq!(sheet.balances.get(&Currency::Token), Some(&500));
    }

    // ── Double-Entry Transfer ─────────────────────────────

    #[test]
    fn test_transfer_basic() {
        let mut ledger = make_ledger();
        ledger.set_balance_genesis("alice", Currency::Money, 1000);
        let (debit_id, credit_id) = ledger
            .transfer(
                "alice",
                "bob",
                200,
                Currency::Money,
                TransactionType::TaskReward,
                "Task payment".into(),
                1,
                Some("task-1".into()),
            )
            .unwrap();

        assert_eq!(ledger.get_balance("alice", Currency::Money), 800);
        assert_eq!(ledger.get_balance("bob", Currency::Money), 200);
        assert_ne!(debit_id, credit_id);

        let entries = ledger.list_entries();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].side, EntrySide::Debit);
        assert_eq!(entries[0].account_id, "alice");
        assert_eq!(entries[1].side, EntrySide::Credit);
        assert_eq!(entries[1].account_id, "bob");
        assert_eq!(entries[0].pair_id, entries[1].pair_id);
    }

    #[test]
    fn test_transfer_insufficient_balance() {
        let mut ledger = make_ledger();
        ledger.set_balance_genesis("alice", Currency::Money, 50);
        let result = ledger.transfer(
            "alice",
            "bob",
            100,
            Currency::Money,
            TransactionType::TaskReward,
            "fail".into(),
            1,
            None,
        );
        assert!(result.is_err());
        let err = result.unwrap_err();
        match err {
            LedgerError::InsufficientBalance {
                account,
                available,
                required,
                ..
            } => {
                assert_eq!(account, "alice");
                assert_eq!(available, 50);
                assert_eq!(required, 100);
            }
            _ => panic!("Expected InsufficientBalance error"),
        }
        // Balances unchanged
        assert_eq!(ledger.get_balance("alice", Currency::Money), 50);
        assert_eq!(ledger.get_balance("bob", Currency::Money), 0);
    }

    #[test]
    fn test_transfer_zero_is_noop() {
        let mut ledger = make_ledger();
        ledger.set_balance_genesis("alice", Currency::Money, 100);
        ledger
            .transfer(
                "alice",
                "bob",
                0,
                Currency::Money,
                TransactionType::TaskReward,
                "zero".into(),
                1,
                None,
            )
            .unwrap();
        assert_eq!(ledger.get_balance("alice", Currency::Money), 100);
        assert_eq!(ledger.get_balance("bob", Currency::Money), 0);
    }

    #[test]
    fn test_transfer_account_not_found() {
        let mut ledger = make_ledger();
        // Unknown sender
        let result = ledger.transfer(
            "unknown",
            "bob",
            100,
            Currency::Money,
            TransactionType::TaskReward,
            "fail".into(),
            1,
            None,
        );
        assert!(result.is_err());
        // Unknown receiver — should also fail
        ledger.set_balance_genesis("alice", Currency::Money, 1000);
        let result = ledger.transfer(
            "alice",
            "ghost",
            100,
            Currency::Money,
            TransactionType::TaskReward,
            "fail".into(),
            1,
            None,
        );
        assert!(result.is_err());
        assert_eq!(ledger.get_balance("ghost", Currency::Money), 0); // no ghost balance created
    }

    #[test]
    fn test_transfer_multiple() {
        let mut ledger = make_ledger();
        ledger.set_balance_genesis("alice", Currency::Money, 1000);
        ledger.set_balance_genesis("bob", Currency::Money, 500);

        ledger
            .transfer(
                "alice",
                "bob",
                100,
                Currency::Money,
                TransactionType::TaskReward,
                "t1".into(),
                1,
                None,
            )
            .unwrap();
        ledger
            .transfer(
                "bob",
                "alice",
                50,
                Currency::Money,
                TransactionType::TaskReward,
                "t2".into(),
                2,
                None,
            )
            .unwrap();

        assert_eq!(ledger.get_balance("alice", Currency::Money), 950);
        assert_eq!(ledger.get_balance("bob", Currency::Money), 550);
        assert_eq!(ledger.list_entries().len(), 4);
    }

    // ── Verification ─────────────────────────────────────

    #[test]
    fn test_verify_pair_balances() {
        let mut ledger = make_ledger();
        ledger.set_balance_genesis("alice", Currency::Money, 1000);
        let (_debit_id, _credit_id) = ledger
            .transfer(
                "alice",
                "bob",
                300,
                Currency::Money,
                TransactionType::TaskReward,
                "verify".into(),
                1,
                None,
            )
            .unwrap();

        let entries = ledger.list_entries();
        let pair_id = entries[0].pair_id;
        assert!(ledger.verify_pair(pair_id, Currency::Money));
    }

    #[test]
    fn test_verify_all_balances() {
        let mut ledger = make_ledger();
        ledger.set_balance_genesis("alice", Currency::Money, 1000);
        ledger.set_balance_genesis("alice", Currency::Token, 5000);

        ledger
            .transfer(
                "alice",
                "bob",
                100,
                Currency::Money,
                TransactionType::TaskReward,
                "t1".into(),
                1,
                None,
            )
            .unwrap();
        ledger
            .transfer(
                "alice",
                "bob",
                1000,
                Currency::Token,
                TransactionType::Teach,
                "t2".into(),
                2,
                None,
            )
            .unwrap();

        assert!(ledger.verify_all());
    }

    #[test]
    fn test_verify_all_after_exchange() {
        let mut ledger = make_ledger();
        ledger.set_balance_genesis("alice", Currency::Money, 100);
        ledger.exchange_money_to_tokens("alice", 10, 1).unwrap();

        // All pairs should balance after exchange
        assert!(ledger.verify_all());
    }

    #[test]
    fn test_verify_all_after_interest() {
        let mut ledger = make_ledger();
        ledger.set_balance_genesis("alice", Currency::Money, 10000);
        ledger.pay_interest("alice", 0.001, 1).unwrap();

        // All pairs should balance after interest
        assert!(ledger.verify_all());
    }

    #[test]
    fn test_verify_all_after_mixed_operations() {
        let mut ledger = make_ledger();
        ledger.set_balance_genesis("alice", Currency::Money, 1000);
        ledger.set_balance_genesis("bob", Currency::Token, 10000);

        ledger
            .transfer(
                "alice",
                "bob",
                100,
                Currency::Money,
                TransactionType::TaskReward,
                "t1".into(),
                1,
                None,
            )
            .unwrap();
        ledger.exchange_money_to_tokens("alice", 5, 2).unwrap();
        ledger.exchange_tokens_to_money("bob", 500, 3).unwrap();
        ledger.pay_interest("alice", 0.001, 4).unwrap();

        // Everything should still balance
        assert!(ledger.verify_all());
    }

    // ── Exchange Rate ────────────────────────────────────

    #[test]
    fn test_exchange_rate_default() {
        let rate = ExchangeRate::default();
        assert_eq!(rate.tokens_per_money, 100);
        assert_eq!(rate.money_to_tokens(1), Some(100));
        assert_eq!(rate.money_to_tokens(5), Some(500));
        assert_eq!(rate.tokens_to_money(100), Some(1));
        assert_eq!(rate.tokens_to_money(250), Some(2));
        assert_eq!(rate.tokens_to_money(99), None);
        assert_eq!(rate.tokens_to_money(0), None);
    }

    #[test]
    fn test_exchange_rate_custom() {
        let rate = ExchangeRate::new(1000);
        assert_eq!(rate.money_to_tokens(1), Some(1000));
        assert_eq!(rate.tokens_to_money(1000), Some(1));
        assert_eq!(rate.tokens_to_money(999), None);
    }

    // ── Central Bank Exchange ────────────────────────────

    #[test]
    fn test_exchange_money_to_tokens() {
        let mut ledger = make_ledger();
        ledger.set_balance_genesis("alice", Currency::Money, 10);

        let result = ledger.exchange_money_to_tokens("alice", 5, 1).unwrap();
        assert_eq!(result.from_amount, 5);
        assert_eq!(result.from_currency, Currency::Money);
        assert_eq!(result.to_amount, 500);
        assert_eq!(result.to_currency, Currency::Token);

        assert_eq!(ledger.get_balance("alice", Currency::Money), 5);
        assert_eq!(ledger.get_balance("alice", Currency::Token), 500);
    }

    #[test]
    fn test_exchange_money_to_tokens_insufficient() {
        let mut ledger = make_ledger();
        ledger.set_balance_genesis("alice", Currency::Money, 3);
        let result = ledger.exchange_money_to_tokens("alice", 5, 1);
        assert!(result.is_err());
        assert_eq!(ledger.get_balance("alice", Currency::Money), 3);
    }

    #[test]
    fn test_exchange_money_to_tokens_unknown_account() {
        let ledger = make_ledger();
        // need mutable ref for exchange
        let mut ledger = ledger;
        let result = ledger.exchange_money_to_tokens("unknown", 5, 1);
        assert!(result.is_err());
    }

    #[test]
    fn test_exchange_tokens_to_money() {
        let mut ledger = make_ledger();
        ledger.set_balance_genesis("alice", Currency::Token, 500);

        let result = ledger.exchange_tokens_to_money("alice", 500, 1).unwrap();
        assert_eq!(result.from_amount, 500);
        assert_eq!(result.from_currency, Currency::Token);
        assert_eq!(result.to_amount, 5);
        assert_eq!(result.to_currency, Currency::Money);

        assert_eq!(ledger.get_balance("alice", Currency::Token), 0);
        assert_eq!(ledger.get_balance("alice", Currency::Money), 5);
    }

    #[test]
    fn test_exchange_tokens_to_money_rounding() {
        let mut ledger = make_ledger();
        // 250 tokens → 2 Money (100 tokens consumed × 2), 50 tokens remain
        ledger.set_balance_genesis("alice", Currency::Token, 250);

        let result = ledger.exchange_tokens_to_money("alice", 250, 1).unwrap();
        assert_eq!(result.to_amount, 2);
        assert_eq!(result.from_amount, 200);
        // 50 tokens remain (not consumed)
        assert_eq!(ledger.get_balance("alice", Currency::Token), 50);
        assert_eq!(ledger.get_balance("alice", Currency::Money), 2);
    }

    #[test]
    fn test_exchange_tokens_to_money_insufficient() {
        let mut ledger = make_ledger();
        ledger.set_balance_genesis("alice", Currency::Token, 50);
        let result = ledger.exchange_tokens_to_money("alice", 50, 1);
        assert!(result.is_err());
    }

    #[test]
    fn test_exchange_creates_double_entry() {
        let mut ledger = make_ledger();
        ledger.set_balance_genesis("alice", Currency::Money, 10);
        ledger.exchange_money_to_tokens("alice", 3, 1).unwrap();

        // Should have 4 entries (2 balanced pairs: Money leg + Token leg)
        let entries = ledger.entries_by_type(TransactionType::Exchange);
        assert_eq!(entries.len(), 4);

        let debit: Vec<_> = entries
            .iter()
            .filter(|e| e.side == EntrySide::Debit)
            .collect();
        let credit: Vec<_> = entries
            .iter()
            .filter(|e| e.side == EntrySide::Credit)
            .collect();
        assert_eq!(debit.len(), 2);
        assert_eq!(credit.len(), 2);

        // Verify two distinct pair_ids
        let pair_ids: std::collections::HashSet<Uuid> = entries.iter().map(|e| e.pair_id).collect();
        assert_eq!(pair_ids.len(), 2);
    }

    // ── Interest ─────────────────────────────────────────

    #[test]
    fn test_pay_interest_basic() {
        let mut ledger = make_ledger();
        ledger.set_balance_genesis("alice", Currency::Money, 10000);

        let result = ledger.pay_interest("alice", 0.001, 1).unwrap().unwrap();
        // 10000 * 0.001 = 10
        assert_eq!(result.principal, 10000);
        assert_eq!(result.interest, 10);
        assert_eq!(result.new_balance, 10010);
        assert_eq!(ledger.get_balance("alice", Currency::Money), 10010);
    }

    #[test]
    fn test_pay_interest_zero_balance() {
        let mut ledger = make_ledger();
        let result = ledger.pay_interest("alice", 0.001, 1).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_pay_interest_truncates() {
        let mut ledger = make_ledger();
        // 500 * 0.001 = 0.5 → truncates to 0
        ledger.set_balance_genesis("alice", Currency::Money, 500);
        let result = ledger.pay_interest("alice", 0.001, 1).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_pay_interest_compound() {
        let mut ledger = make_ledger();
        ledger.set_balance_genesis("alice", Currency::Money, 10000);

        ledger.pay_interest("alice", 0.001, 1).unwrap();
        assert_eq!(ledger.get_balance("alice", Currency::Money), 10010);

        ledger.pay_interest("alice", 0.001, 2).unwrap();
        // 10010 * 0.001 = 10.01 → truncates to 10
        assert_eq!(ledger.get_balance("alice", Currency::Money), 10020);
    }

    // ── Audit Log ────────────────────────────────────────

    #[test]
    fn test_exchange_creates_audit_record() {
        let mut ledger = make_ledger();
        ledger.set_balance_genesis("alice", Currency::Money, 10);
        ledger.exchange_money_to_tokens("alice", 5, 1).unwrap();

        // First audit record is from set_balance_genesis, second from exchange
        let exchange_audits: Vec<_> = ledger
            .audit_log()
            .iter()
            .filter(|a| a.operation == "exchange_money_to_tokens")
            .collect();
        assert_eq!(exchange_audits.len(), 1);
        assert_eq!(exchange_audits[0].actor, "alice");
        assert_eq!(exchange_audits[0].entry_ids.len(), 4);
    }

    #[test]
    fn test_tokens_to_money_creates_audit_record() {
        let mut ledger = make_ledger();
        ledger.set_balance_genesis("alice", Currency::Token, 500);
        ledger.exchange_tokens_to_money("alice", 500, 1).unwrap();

        let exchange_audits: Vec<_> = ledger
            .audit_log()
            .iter()
            .filter(|a| a.operation == "exchange_tokens_to_money")
            .collect();
        assert_eq!(exchange_audits.len(), 1);
        assert_eq!(exchange_audits[0].actor, "alice");
    }

    #[test]
    fn test_interest_creates_audit_record() {
        let mut ledger = make_ledger();
        ledger.set_balance_genesis("alice", Currency::Money, 10000);
        ledger.pay_interest("alice", 0.001, 1).unwrap();

        let interest_audits: Vec<_> = ledger
            .audit_log()
            .iter()
            .filter(|a| a.operation == "pay_interest")
            .collect();
        assert_eq!(interest_audits.len(), 1);
    }

    #[test]
    fn test_audit_by_actor() {
        let mut ledger = make_ledger();
        ledger.set_balance_genesis("alice", Currency::Money, 10000);
        ledger.set_balance_genesis("bob", Currency::Money, 5000);

        ledger.pay_interest("alice", 0.001, 1).unwrap();
        ledger.pay_interest("bob", 0.001, 2).unwrap();

        // Alice: 1 genesis + 1 interest = 2 audit records
        assert_eq!(ledger.audit_by_actor("alice").len(), 2);
        assert_eq!(ledger.audit_by_actor("bob").len(), 2);
    }

    // ── Supply Queries ───────────────────────────────────

    #[test]
    fn test_total_supply() {
        let mut ledger = make_ledger();
        ledger.set_balance_genesis("alice", Currency::Money, 100);
        ledger.set_balance_genesis("bob", Currency::Money, 200);
        ledger.set_balance_genesis("alice", Currency::Token, 1000);
        ledger.set_balance_genesis("bob", Currency::Token, 2000);

        assert_eq!(ledger.total_money_supply(), 300);
        assert_eq!(ledger.total_token_supply(), 3000);
    }

    // ── Query Helpers ────────────────────────────────────

    #[test]
    fn test_entries_by_account() {
        let mut ledger = make_ledger();
        ledger.set_balance_genesis("alice", Currency::Money, 1000);
        ledger
            .transfer(
                "alice",
                "bob",
                100,
                Currency::Money,
                TransactionType::TaskReward,
                "t1".into(),
                1,
                None,
            )
            .unwrap();

        let alice_entries = ledger.entries_by_account("alice");
        let bob_entries = ledger.entries_by_account("bob");
        assert_eq!(alice_entries.len(), 1);
        assert_eq!(bob_entries.len(), 1);
    }

    #[test]
    fn test_entries_by_type() {
        let mut ledger = make_ledger();
        ledger.set_balance_genesis("alice", Currency::Money, 1000);
        ledger.set_balance_genesis("alice", Currency::Token, 5000);
        ledger
            .transfer(
                "alice",
                "bob",
                100,
                Currency::Money,
                TransactionType::TaskReward,
                "t1".into(),
                1,
                None,
            )
            .unwrap();
        ledger.exchange_money_to_tokens("alice", 5, 2).unwrap();

        assert_eq!(ledger.entries_by_type(TransactionType::TaskReward).len(), 2);
        // Exchange creates 4 entries (2 balanced pairs)
        assert_eq!(ledger.entries_by_type(TransactionType::Exchange).len(), 4);
    }

    #[test]
    fn test_entries_by_reference() {
        let mut ledger = make_ledger();
        ledger.set_balance_genesis("alice", Currency::Money, 1000);
        ledger
            .transfer(
                "alice",
                "bob",
                100,
                Currency::Money,
                TransactionType::TaskReward,
                "t1".into(),
                1,
                Some("task-1".into()),
            )
            .unwrap();

        assert_eq!(ledger.entries_by_reference("task-1").len(), 2);
        assert_eq!(ledger.entries_by_reference("task-2").len(), 0);
    }

    // ── Serialization ────────────────────────────────────

    #[test]
    fn test_exchange_result_serialization() {
        let result = ExchangeResult {
            pair_id: Uuid::new_v4(),
            from_amount: 100,
            from_currency: Currency::Money,
            to_amount: 10000,
            to_currency: Currency::Token,
        };
        let json = serde_json::to_string(&result).unwrap();
        let back: ExchangeResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result.pair_id, back.pair_id);
        assert_eq!(result.from_amount, back.from_amount);
        assert_eq!(result.to_amount, back.to_amount);
    }

    #[test]
    fn test_entry_side_serialization() {
        let json = serde_json::to_string(&EntrySide::Debit).unwrap();
        assert_eq!(json, "\"debit\"");
        let json = serde_json::to_string(&EntrySide::Credit).unwrap();
        assert_eq!(json, "\"credit\"");
    }

    #[test]
    fn test_account_type_serialization() {
        let json = serde_json::to_string(&AccountType::Agent).unwrap();
        assert_eq!(json, "\"agent\"");
        let json = serde_json::to_string(&AccountType::CentralBank).unwrap();
        assert_eq!(json, "\"central_bank\"");
    }

    #[test]
    fn test_balance_sheet_serialization() {
        let mut balances = HashMap::new();
        balances.insert(Currency::Money, 500);
        balances.insert(Currency::Token, 10000);
        let sheet = BalanceSheet {
            account_id: "alice".into(),
            balances,
        };
        let json = serde_json::to_string(&sheet).unwrap();
        let back: BalanceSheet = serde_json::from_str(&json).unwrap();
        assert_eq!(sheet.account_id, back.account_id);
        assert_eq!(back.balances.get(&Currency::Money), Some(&500));
    }

    #[test]
    fn test_audit_record_serialization() {
        let record = AuditRecord {
            id: Uuid::new_v4(),
            tick: 42,
            operation: "test".into(),
            actor: "alice".into(),
            details: serde_json::json!({"key": "value"}),
            entry_ids: vec![Uuid::new_v4()],
        };
        let json = serde_json::to_string(&record).unwrap();
        let back: AuditRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(record.id, back.id);
        assert_eq!(record.operation, back.operation);
    }

    // ── Edge Cases ───────────────────────────────────────

    #[test]
    fn test_transfer_with_tokens() {
        let mut ledger = make_ledger();
        ledger.set_balance_genesis("alice", Currency::Token, 5000);
        ledger
            .transfer(
                "alice",
                "bob",
                1000,
                Currency::Token,
                TransactionType::Teach,
                "lesson".into(),
                1,
                None,
            )
            .unwrap();

        assert_eq!(ledger.get_balance("alice", Currency::Token), 4000);
        assert_eq!(ledger.get_balance("bob", Currency::Token), 1000);
    }

    #[test]
    fn test_multiple_currencies_independent() {
        let mut ledger = make_ledger();
        ledger.set_balance_genesis("alice", Currency::Money, 1000);
        ledger.set_balance_genesis("alice", Currency::Token, 5000);

        ledger
            .transfer(
                "alice",
                "bob",
                100,
                Currency::Money,
                TransactionType::TaskReward,
                "t1".into(),
                1,
                None,
            )
            .unwrap();

        // Only Money affected
        assert_eq!(ledger.get_balance("alice", Currency::Money), 900);
        assert_eq!(ledger.get_balance("alice", Currency::Token), 5000);
    }

    #[test]
    fn test_exchange_back_and_forth() {
        let mut ledger = make_ledger();
        ledger.set_balance_genesis("alice", Currency::Money, 10);

        // Money → Token: 5 Money → 500 Tokens
        ledger.exchange_money_to_tokens("alice", 5, 1).unwrap();
        assert_eq!(ledger.get_balance("alice", Currency::Money), 5);
        assert_eq!(ledger.get_balance("alice", Currency::Token), 500);

        // Token → Money: 500 Tokens → 5 Money
        ledger.exchange_tokens_to_money("alice", 500, 2).unwrap();
        assert_eq!(ledger.get_balance("alice", Currency::Money), 10);
        assert_eq!(ledger.get_balance("alice", Currency::Token), 0);
    }

    #[test]
    fn test_round_trip_preserves_value() {
        let mut ledger = make_ledger();
        ledger.set_balance_genesis("alice", Currency::Money, 100);

        // 100 Money → 10000 Tokens
        ledger.exchange_money_to_tokens("alice", 100, 1).unwrap();
        assert_eq!(ledger.get_balance("alice", Currency::Token), 10000);
        assert_eq!(ledger.get_balance("alice", Currency::Money), 0);

        // 10000 Tokens → 100 Money
        ledger.exchange_tokens_to_money("alice", 10000, 2).unwrap();
        assert_eq!(ledger.get_balance("alice", Currency::Money), 100);
        assert_eq!(ledger.get_balance("alice", Currency::Token), 0);
    }
}
