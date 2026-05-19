//! Banking system: savings/checking accounts, loans, interest, collateral,
//! central bank operations, and bad-debt collection.
//!
//! Design:
//! - `BankingSystem` wraps a `MoneyLedger` and adds bank-specific logic.
//! - Two account types: savings (interest-bearing) and checking (transactional).
//! - Loans follow a full lifecycle: Application → Approval → Disbursement → Repayment.
//! - Interest rates are configurable per account type and driven by the central bank.
//! - Collateral can be skills (represented as skill points) or reputation score.
//! - Overdue loans trigger automatic deductions until liquidation.
//! - Central bank can adjust base rates, mint money, and write off bad debt.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::economy::ledger::{
    Account, AccountType, ExchangeRate, MoneyLedger,
};
use crate::world::enums::Currency;
use crate::world::event::WorldEvent;
use crate::world::state::EventBus;

// ── Account Types ─────────────────────────────────────────

/// Type of bank account.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BankAccountType {
    /// Savings account — earns interest on deposits.
    Savings,
    /// Checking account — for everyday transactions, no interest.
    Checking,
}

// ── Bank Account ──────────────────────────────────────────

/// A bank account held by an agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BankAccount {
    pub id: Uuid,
    pub owner_id: String,
    pub account_type: BankAccountType,
    /// Human-readable label (e.g. "Alice Savings").
    pub label: String,
    pub created_tick: u64,
}

// ── Collateral ────────────────────────────────────────────

/// Types of collateral that can secure a loan.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub enum Collateral {
    /// Skill points pledged as collateral.
    Skill { skill_name: String, level: u64 },
    /// Reputation score pledged as collateral.
    Reputation { score: f64 },
}

// ── Loan Lifecycle ────────────────────────────────────────

/// Status of a loan application.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoanStatus {
    /// Application submitted, awaiting review.
    Pending,
    /// Approved by the bank, funds not yet disbursed.
    Approved,
    /// Funds disbursed; repayment in progress.
    Active,
    /// Fully repaid.
    Repaid,
    /// Payment overdue; automatic deductions ongoing.
    Defaulted,
    /// Written off by central bank as bad debt.
    WrittenOff,
}

/// A loan record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Loan {
    pub id: Uuid,
    pub borrower_id: String,
    /// Principal amount borrowed (in Money).
    pub principal: u64,
    /// Outstanding balance (principal + accrued interest - repayments).
    pub outstanding_balance: u64,
    /// Annualised interest rate (per-tick basis, e.g. 0.001 = 0.1%/tick).
    pub interest_rate: f64,
    /// Number of ticks the borrower has to repay.
    pub term_ticks: u64,
    pub status: LoanStatus,
    /// Collateral pledged against this loan.
    pub collateral: Option<Collateral>,
    /// Tick at which the loan was created.
    pub created_tick: u64,
    /// Tick at which the loan was disbursed.
    pub disbursed_tick: Option<u64>,
    /// Tick by which the loan should be fully repaid.
    pub due_tick: Option<u64>,
    /// Total repaid so far.
    pub total_repaid: u64,
    /// Number of ticks overdue.
    pub ticks_overdue: u64,
}

// ── Central Bank Config ───────────────────────────────────

/// Central bank policy configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CentralBankConfig {
    /// Base interest rate for savings accounts (per tick).
    pub savings_rate: f64,
    /// Base interest rate for loans (per tick).
    pub loan_rate: f64,
    /// Fraction of overdue balance automatically deducted per tick.
    pub auto_deduct_rate: f64,
    /// Maximum loan-to-value ratio (collateral_value * ltv_ratio = max loan).
    pub ltv_ratio: f64,
    /// Ticks of grace before a loan is marked as defaulted.
    pub grace_ticks: u64,
    /// Skill-collateral value: each skill level is worth this many Money.
    pub skill_collateral_value: u64,
    /// Reputation-collateral value: each point of reputation is worth this many Money.
    pub reputation_collateral_value: u64,
}

impl Default for CentralBankConfig {
    fn default() -> Self {
        Self {
            savings_rate: 0.0005,    // 0.05% per tick
            loan_rate: 0.001,        // 0.1% per tick
            auto_deduct_rate: 0.1,   // 10% of overdue balance per tick
            ltv_ratio: 0.7,          // 70% LTV
            grace_ticks: 10,
            skill_collateral_value: 100,
            reputation_collateral_value: 50,
        }
    }
}

// ── Errors ────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum BankingError {
    #[error("account not found: {0}")]
    AccountNotFound(String),
    #[error("loan not found: {0}")]
    LoanNotFound(String),
    #[error("insufficient funds: account {account} has {available}, needs {required}")]
    InsufficientFunds {
        account: String,
        available: u64,
        required: u64,
    },
    #[error("invalid loan status: expected {expected:?}, got {actual:?}")]
    InvalidLoanStatus {
        expected: Vec<LoanStatus>,
        actual: LoanStatus,
    },
    #[error("insufficient collateral: provided {provided}, required {required}")]
    InsufficientCollateral { provided: u64, required: u64 },
    #[error("loan amount exceeds maximum: requested {requested}, max {max}")]
    LoanAmountExceedsMax { requested: u64, max: u64 },
    #[error("ledger error: {0}")]
    Ledger(String),
    #[error("duplicate account: {0}")]
    DuplicateAccount(String),
    #[error("agent already has an account of this type: {agent_id}")]
    DuplicateAccountType { agent_id: String },
    #[error("no bank account found for agent {agent_id}")]
    NoBankAccount { agent_id: String },
}

// ── Result Types ──────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepositResult {
    pub account_id: String,
    pub amount: u64,
    pub new_balance: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WithdrawResult {
    pub account_id: String,
    pub amount: u64,
    pub new_balance: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoanApplicationResult {
    pub loan_id: Uuid,
    pub borrower_id: String,
    pub principal: u64,
    pub interest_rate: f64,
    pub term_ticks: u64,
    pub status: LoanStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepaymentResult {
    pub loan_id: Uuid,
    pub amount_paid: u64,
    pub outstanding_balance: u64,
    pub fully_repaid: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterestPaymentResult {
    pub account_id: String,
    pub interest: u64,
    pub new_balance: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateAdjustmentResult {
    pub new_savings_rate: f64,
    pub new_loan_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MintResult {
    pub amount: u64,
    pub total_money_supply: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WriteOffResult {
    pub loan_id: Uuid,
    pub amount_written_off: u64,
}

// ── Banking System ────────────────────────────────────────

/// The banking system, managing accounts, loans, interest, and central bank operations.
pub struct BankingSystem {
    /// Underlying double-entry ledger.
    ledger: MoneyLedger,
    /// Bank accounts keyed by id.
    bank_accounts: HashMap<Uuid, BankAccount>,
    /// Map from (owner_id, account_type) to account id — one per type per agent.
    owner_account_index: HashMap<(String, BankAccountType), Uuid>,
    /// Loans keyed by id.
    loans: HashMap<Uuid, Loan>,
    /// Central bank policy.
    config: CentralBankConfig,
    /// Optional event bus for emitting events.
    event_bus: Option<EventBus>,
}

impl BankingSystem {
    /// Create a new banking system with default exchange rate.
    pub fn new(config: CentralBankConfig) -> Self {
        let ledger = MoneyLedger::new(ExchangeRate::default());
        Self {
            ledger,
            bank_accounts: HashMap::new(),
            owner_account_index: HashMap::new(),
            loans: HashMap::new(),
            config,
            event_bus: None,
        }
    }

    /// Create with an event bus for broadcasting banking events.
    pub fn with_event_bus(config: CentralBankConfig, event_bus: EventBus) -> Self {
        let mut banking = Self::new(config);
        banking.event_bus = Some(event_bus);
        banking
    }

    // ── Account Management ───────────────────────────────

    /// Open a new bank account for an agent.
    pub fn open_account(
        &mut self,
        owner_id: &str,
        account_type: BankAccountType,
        label: &str,
        tick: u64,
    ) -> Result<BankAccount, BankingError> {
        let key = (owner_id.to_string(), account_type);
        if self.owner_account_index.contains_key(&key) {
            return Err(BankingError::DuplicateAccountType {
                agent_id: owner_id.to_string(),
            });
        }

        let account_id = Uuid::new_v4();
        let ledger_account_id = format!("bank_{}", account_id);

        // Register the account in the ledger.
        self.ledger.create_account(Account {
            id: ledger_account_id.clone(),
            name: label.to_string(),
            account_type: AccountType::Agent,
        });

        let bank_account = BankAccount {
            id: account_id,
            owner_id: owner_id.to_string(),
            account_type,
            label: label.to_string(),
            created_tick: tick,
        };

        self.bank_accounts.insert(account_id, bank_account.clone());
        self.owner_account_index.insert(key, account_id);

        self.emit(WorldEvent::BankAccountOpened {
            account_id: account_id.to_string(),
            owner_id: owner_id.to_string(),
            account_type: format!("{:?}", account_type).to_lowercase(),
        });

        Ok(bank_account)
    }

    /// Get a bank account by its id.
    pub fn get_account(&self, account_id: Uuid) -> Option<&BankAccount> {
        self.bank_accounts.get(&account_id)
    }

    /// Get the bank account for an agent by type.
    pub fn get_account_by_owner(
        &self,
        owner_id: &str,
        account_type: BankAccountType,
    ) -> Option<&BankAccount> {
        let key = (owner_id.to_string(), account_type);
        self.owner_account_index
            .get(&key)
            .and_then(|id| self.bank_accounts.get(id))
    }

    /// Get all bank accounts for an agent.
    pub fn get_accounts_by_owner(&self, owner_id: &str) -> Vec<&BankAccount> {
        self.bank_accounts
            .values()
            .filter(|a| a.owner_id == owner_id)
            .collect()
    }

    /// List all bank accounts.
    pub fn list_accounts(&self) -> Vec<&BankAccount> {
        self.bank_accounts.values().collect()
    }

    /// Get the balance of a bank account.
    pub fn get_balance(&self, account_id: Uuid) -> Option<u64> {
        let ledger_id = format!("bank_{}", account_id);
        if self.ledger.account_exists(&ledger_id) {
            Some(self.ledger.get_balance(&ledger_id, Currency::Money))
        } else {
            None
        }
    }

    // ── Deposit / Withdraw ───────────────────────────────

    /// Deposit money into a bank account.
    ///
    /// Transfers Money from the agent's wallet (ledger account = agent_id) into
    /// the bank account.
    pub fn deposit(
        &mut self,
        account_id: Uuid,
        owner_id: &str,
        amount: u64,
        tick: u64,
    ) -> Result<DepositResult, BankingError> {
        let _account = self.bank_accounts.get(&account_id)
            .ok_or_else(|| BankingError::AccountNotFound(account_id.to_string()))?;

        if amount == 0 {
            return Ok(DepositResult {
                account_id: account_id.to_string(),
                amount: 0,
                new_balance: self.get_balance(account_id).unwrap_or(0),
            });
        }

        let ledger_id = format!("bank_{}", account_id);

        // Ensure the agent has a ledger account.
        if !self.ledger.account_exists(owner_id) {
            self.ledger.create_account(Account::new_agent(owner_id, owner_id));
        }

        self.ledger
            .transfer(
                owner_id,
                &ledger_id,
                amount,
                Currency::Money,
                crate::economy::reward::TransactionType::BankDeposit,
                format!("Deposit to bank account {}", account_id),
                tick,
                Some(account_id.to_string()),
            )
            .map_err(|e| BankingError::Ledger(e.to_string()))?;

        let new_balance = self.ledger.get_balance(&ledger_id, Currency::Money);

        self.emit(WorldEvent::BankDeposit {
            account_id: account_id.to_string(),
            owner_id: owner_id.to_string(),
            amount,
            new_balance,
        });

        Ok(DepositResult {
            account_id: account_id.to_string(),
            amount,
            new_balance,
        })
    }

    /// Withdraw money from a bank account.
    pub fn withdraw(
        &mut self,
        account_id: Uuid,
        owner_id: &str,
        amount: u64,
        tick: u64,
    ) -> Result<WithdrawResult, BankingError> {
        let _account = self.bank_accounts.get(&account_id)
            .ok_or_else(|| BankingError::AccountNotFound(account_id.to_string()))?;

        if amount == 0 {
            return Ok(WithdrawResult {
                account_id: account_id.to_string(),
                amount: 0,
                new_balance: self.get_balance(account_id).unwrap_or(0),
            });
        }

        let ledger_id = format!("bank_{}", account_id);
        let available = self.ledger.get_balance(&ledger_id, Currency::Money);
        if available < amount {
            return Err(BankingError::InsufficientFunds {
                account: account_id.to_string(),
                available,
                required: amount,
            });
        }

        // Ensure the agent has a ledger account to receive the withdrawal.
        if !self.ledger.account_exists(owner_id) {
            self.ledger.create_account(Account::new_agent(owner_id, owner_id));
        }

        self.ledger
            .transfer(
                &ledger_id,
                owner_id,
                amount,
                Currency::Money,
                crate::economy::reward::TransactionType::BankWithdrawal,
                format!("Withdrawal from bank account {}", account_id),
                tick,
                Some(account_id.to_string()),
            )
            .map_err(|e| BankingError::Ledger(e.to_string()))?;

        let new_balance = self.ledger.get_balance(&ledger_id, Currency::Money);

        self.emit(WorldEvent::BankWithdrawal {
            account_id: account_id.to_string(),
            owner_id: owner_id.to_string(),
            amount,
            new_balance,
        });

        Ok(WithdrawResult {
            account_id: account_id.to_string(),
            amount,
            new_balance,
        })
    }

    // ── Loan Lifecycle ───────────────────────────────────

    /// Apply for a loan.
    pub fn apply_for_loan(
        &mut self,
        borrower_id: &str,
        amount: u64,
        term_ticks: u64,
        collateral: Option<Collateral>,
        tick: u64,
    ) -> Result<LoanApplicationResult, BankingError> {
        if amount == 0 {
            return Err(BankingError::LoanAmountExceedsMax {
                requested: 0,
                max: 0,
            });
        }

        // Check collateral if provided.
        if let Some(ref col) = collateral {
            let provided = self.collateral_value(col);
            let max_loan = (provided as f64 * self.config.ltv_ratio) as u64;
            if amount > max_loan {
                return Err(BankingError::LoanAmountExceedsMax {
                    requested: amount,
                    max: max_loan,
                });
            }
        }

        let loan_id = Uuid::new_v4();
        let loan = Loan {
            id: loan_id,
            borrower_id: borrower_id.to_string(),
            principal: amount,
            outstanding_balance: 0, // Will be set on disbursement
            interest_rate: self.config.loan_rate,
            term_ticks,
            status: LoanStatus::Pending,
            collateral,
            created_tick: tick,
            disbursed_tick: None,
            due_tick: None,
            total_repaid: 0,
            ticks_overdue: 0,
        };

        self.loans.insert(loan_id, loan.clone());

        self.emit(WorldEvent::LoanApplied {
            loan_id: loan_id.to_string(),
            borrower_id: borrower_id.to_string(),
            amount,
            term_ticks,
        });

        Ok(LoanApplicationResult {
            loan_id,
            borrower_id: borrower_id.to_string(),
            principal: amount,
            interest_rate: loan.interest_rate,
            term_ticks,
            status: LoanStatus::Pending,
        })
    }

    /// Approve a pending loan application.
    pub fn approve_loan(&mut self, loan_id: Uuid, _tick: u64) -> Result<Loan, BankingError> {
        let loan = self.loans.get_mut(&loan_id)
            .ok_or_else(|| BankingError::LoanNotFound(loan_id.to_string()))?;

        if loan.status != LoanStatus::Pending {
            return Err(BankingError::InvalidLoanStatus {
                expected: vec![LoanStatus::Pending],
                actual: loan.status,
            });
        }

        loan.status = LoanStatus::Approved;
        let borrower_id = loan.borrower_id.clone();
        let principal = loan.principal;
        let result = loan.clone();

        self.emit(WorldEvent::LoanApproved {
            loan_id: loan_id.to_string(),
            borrower_id,
            amount: principal,
        });

        Ok(result)
    }

    /// Disburse an approved loan, transferring funds to the borrower's wallet.
    pub fn disburse_loan(&mut self, loan_id: Uuid, tick: u64) -> Result<Loan, BankingError> {
        let loan = self.loans.get_mut(&loan_id)
            .ok_or_else(|| BankingError::LoanNotFound(loan_id.to_string()))?;

        if loan.status != LoanStatus::Approved {
            return Err(BankingError::InvalidLoanStatus {
                expected: vec![LoanStatus::Approved],
                actual: loan.status,
            });
        }

        let borrower_id = loan.borrower_id.clone();
        let principal = loan.principal;

        // Ensure the borrower has a ledger account.
        if !self.ledger.account_exists(&borrower_id) {
            self.ledger.create_account(Account::new_agent(&borrower_id, &borrower_id));
        }

        // Transfer from central bank to borrower.
        self.ledger
            .transfer(
                "central_bank",
                &borrower_id,
                principal,
                Currency::Money,
                crate::economy::reward::TransactionType::LoanDisbursement,
                format!("Loan {} disbursement", loan_id),
                tick,
                Some(loan_id.to_string()),
            )
            .map_err(|e| BankingError::Ledger(e.to_string()))?;

        loan.status = LoanStatus::Active;
        loan.outstanding_balance = principal;
        loan.disbursed_tick = Some(tick);
        loan.due_tick = Some(tick + loan.term_ticks);
        let due_tick = loan.due_tick.unwrap();
        let result = loan.clone();

        self.emit(WorldEvent::LoanDisbursed {
            loan_id: loan_id.to_string(),
            borrower_id: borrower_id.clone(),
            amount: principal,
            due_tick,
        });

        Ok(result)
    }

    /// Repay part or all of an active loan.
    pub fn repay_loan(
        &mut self,
        loan_id: Uuid,
        amount: u64,
        tick: u64,
    ) -> Result<RepaymentResult, BankingError> {
        let loan = self.loans.get_mut(&loan_id)
            .ok_or_else(|| BankingError::LoanNotFound(loan_id.to_string()))?;

        if loan.status != LoanStatus::Active && loan.status != LoanStatus::Defaulted {
            return Err(BankingError::InvalidLoanStatus {
                expected: vec![LoanStatus::Active, LoanStatus::Defaulted],
                actual: loan.status,
            });
        }

        if amount == 0 {
            return Ok(RepaymentResult {
                loan_id,
                amount_paid: 0,
                outstanding_balance: loan.outstanding_balance,
                fully_repaid: false,
            });
        }

        let borrower_id = loan.borrower_id.clone();
        let actual_payment = amount.min(loan.outstanding_balance);

        // Check borrower has sufficient funds.
        let borrower_balance = self.ledger.get_balance(&borrower_id, Currency::Money);
        let payment = actual_payment.min(borrower_balance);

        if payment == 0 {
            return Err(BankingError::InsufficientFunds {
                account: borrower_id,
                available: borrower_balance,
                required: actual_payment,
            });
        }

        // Transfer from borrower to central bank.
        self.ledger
            .transfer(
                &borrower_id,
                "central_bank",
                payment,
                Currency::Money,
                crate::economy::reward::TransactionType::LoanRepayment,
                format!("Repayment for loan {}", loan_id),
                tick,
                Some(loan_id.to_string()),
            )
            .map_err(|e| BankingError::Ledger(e.to_string()))?;

        loan.total_repaid += payment;
        loan.outstanding_balance = loan.outstanding_balance.saturating_sub(payment);

        let fully_repaid = loan.outstanding_balance == 0;
        if fully_repaid {
            loan.status = LoanStatus::Repaid;
            loan.ticks_overdue = 0;
        }
        let outstanding_balance = loan.outstanding_balance;

        self.emit(WorldEvent::LoanRepayment {
            loan_id: loan_id.to_string(),
            borrower_id: borrower_id.clone(),
            amount: payment,
            outstanding_balance,
            fully_repaid,
        });

        Ok(RepaymentResult {
            loan_id,
            amount_paid: payment,
            outstanding_balance,
            fully_repaid,
        })
    }

    // ── Interest ─────────────────────────────────────────

    /// Accrue interest on savings accounts.
    pub fn accrue_savings_interest(&mut self, tick: u64) -> Vec<InterestPaymentResult> {
        let rate = self.config.savings_rate;
        let mut results = Vec::new();

        let savings_accounts: Vec<(Uuid, String)> = self.bank_accounts.values()
            .filter(|a| a.account_type == BankAccountType::Savings)
            .map(|a| (a.id, a.owner_id.clone()))
            .collect();

        for (account_id, _owner_id) in savings_accounts {
            let ledger_id = format!("bank_{}", account_id);
            let balance = self.ledger.get_balance(&ledger_id, Currency::Money);
            if balance == 0 {
                continue;
            }

            let interest = ((balance as f64) * rate) as u64;
            if interest == 0 {
                continue;
            }

            // Central bank pays interest to the savings account.
            if let Ok(_) = self.ledger.transfer(
                "central_bank",
                &ledger_id,
                interest,
                Currency::Money,
                crate::economy::reward::TransactionType::Interest,
                format!("Savings interest for account {}", account_id),
                tick,
                Some(account_id.to_string()),
            ) {
                let new_balance = self.ledger.get_balance(&ledger_id, Currency::Money);
                results.push(InterestPaymentResult {
                    account_id: account_id.to_string(),
                    interest,
                    new_balance,
                });
            }
        }

        results
    }

    /// Accrue interest on active loans (adds to outstanding balance).
    pub fn accrue_loan_interest(&mut self, _tick: u64) {
        let rate = self.config.loan_rate;
        let active_loans: Vec<Uuid> = self.loans.values()
            .filter(|l| l.status == LoanStatus::Active || l.status == LoanStatus::Defaulted)
            .map(|l| l.id)
            .collect();

        for loan_id in active_loans {
            if let Some(loan) = self.loans.get_mut(&loan_id) {
                let interest = ((loan.outstanding_balance as f64) * rate) as u64;
                if interest > 0 {
                    loan.outstanding_balance += interest;
                }
            }
        }
    }

    // ── Bad Debt / Auto-Deduction ────────────────────────

    /// Process overdue loans: auto-deduct from borrower wallets.
    pub fn process_overdue_loans(&mut self, tick: u64) -> Vec<RepaymentResult> {
        let mut results = Vec::new();
        let rate = self.config.auto_deduct_rate;

        // Find loans past their due date.
        let overdue_loans: Vec<Uuid> = self.loans.values()
            .filter(|l| {
                (l.status == LoanStatus::Active || l.status == LoanStatus::Defaulted)
                    && l.due_tick.map_or(false, |due| tick > due)
            })
            .map(|l| l.id)
            .collect();

        for loan_id in overdue_loans {
            if let Some(loan) = self.loans.get_mut(&loan_id) {
                if loan.status == LoanStatus::Active {
                    loan.status = LoanStatus::Defaulted;
                }
                loan.ticks_overdue += 1;

                // Only auto-deduct after grace period.
                if loan.ticks_overdue <= self.config.grace_ticks {
                    continue;
                }

                let deduction = ((loan.outstanding_balance as f64) * rate) as u64;
                if deduction == 0 {
                    continue;
                }

                let borrower_id = loan.borrower_id.clone();
                let borrower_balance = self.ledger.get_balance(&borrower_id, Currency::Money);
                let actual_deduction = deduction.min(borrower_balance).min(loan.outstanding_balance);

                if actual_deduction == 0 {
                    continue;
                }

                // Deduct from borrower wallet.
                if let Ok(_) = self.ledger.transfer(
                    &borrower_id,
                    "central_bank",
                    actual_deduction,
                    Currency::Money,
                    crate::economy::reward::TransactionType::LoanRepayment,
                    format!("Auto-deduction for overdue loan {}", loan_id),
                    tick,
                    Some(loan_id.to_string()),
                ) {
                    loan.total_repaid += actual_deduction;
                    loan.outstanding_balance = loan.outstanding_balance.saturating_sub(actual_deduction);

                    let fully_repaid = loan.outstanding_balance == 0;
                    if fully_repaid {
                        loan.status = LoanStatus::Repaid;
                        loan.ticks_overdue = 0;
                    }

                    results.push(RepaymentResult {
                        loan_id,
                        amount_paid: actual_deduction,
                        outstanding_balance: loan.outstanding_balance,
                        fully_repaid,
                    });
                }
            }
        }

        results
    }

    // ── Central Bank Operations ──────────────────────────

    /// Adjust the central bank interest rates.
    pub fn adjust_rates(
        &mut self,
        new_savings_rate: f64,
        new_loan_rate: f64,
    ) -> RateAdjustmentResult {
        self.config.savings_rate = new_savings_rate;
        self.config.loan_rate = new_loan_rate;

        // Update interest rate on all active loans.
        for loan in self.loans.values_mut() {
            if loan.status == LoanStatus::Active || loan.status == LoanStatus::Defaulted {
                loan.interest_rate = new_loan_rate;
            }
        }

        self.emit(WorldEvent::BankRateAdjusted {
            new_savings_rate,
            new_loan_rate,
        });

        RateAdjustmentResult {
            new_savings_rate,
            new_loan_rate,
        }
    }

    /// Mint new money (central bank creates money out of thin air).
    /// The money is deposited into the central bank's own account.
    pub fn mint_money(&mut self, amount: u64, _tick: u64) -> MintResult {
        // Use genesis-style balance injection for minting.
        let current = self.ledger.get_balance("central_bank", Currency::Money);
        self.ledger.set_balance_genesis("central_bank", Currency::Money, current + amount);

        self.emit(WorldEvent::MoneyMinted {
            amount,
            total_supply: self.ledger.total_money_supply(),
        });

        MintResult {
            amount,
            total_money_supply: self.ledger.total_money_supply(),
        }
    }

    /// Write off a bad debt loan. The outstanding balance is absorbed by the central bank.
    pub fn write_off_bad_debt(&mut self, loan_id: Uuid, _tick: u64) -> Result<WriteOffResult, BankingError> {
        let loan = self.loans.get_mut(&loan_id)
            .ok_or_else(|| BankingError::LoanNotFound(loan_id.to_string()))?;

        if loan.status != LoanStatus::Defaulted {
            return Err(BankingError::InvalidLoanStatus {
                expected: vec![LoanStatus::Defaulted],
                actual: loan.status,
            });
        }

        let amount_written_off = loan.outstanding_balance;
        let borrower_id = loan.borrower_id.clone();
        loan.outstanding_balance = 0;
        loan.status = LoanStatus::WrittenOff;

        self.emit(WorldEvent::BadDebtWrittenOff {
            loan_id: loan_id.to_string(),
            borrower_id,
            amount: amount_written_off,
        });

        Ok(WriteOffResult {
            loan_id,
            amount_written_off,
        })
    }

    // ── Queries ──────────────────────────────────────────

    /// Get a loan by id.
    pub fn get_loan(&self, loan_id: Uuid) -> Option<&Loan> {
        self.loans.get(&loan_id)
    }

    /// List all loans, optionally filtered by borrower or status.
    pub fn list_loans(
        &self,
        borrower_id: Option<&str>,
        status: Option<LoanStatus>,
    ) -> Vec<&Loan> {
        self.loans.values()
            .filter(|l| {
                borrower_id.map_or(true, |b| l.borrower_id == b)
                    && status.map_or(true, |s| l.status == s)
            })
            .collect()
    }

    /// Get the current central bank config.
    pub fn config(&self) -> &CentralBankConfig {
        &self.config
    }

    /// Get a reference to the underlying ledger.
    pub fn ledger(&self) -> &MoneyLedger {
        &self.ledger
    }

    /// Get the total money supply.
    pub fn total_money_supply(&self) -> u64 {
        self.ledger.total_money_supply()
    }

    /// Get the total outstanding loan debt.
    pub fn total_loan_debt(&self) -> u64 {
        self.loans.values()
            .filter(|l| l.status == LoanStatus::Active || l.status == LoanStatus::Defaulted)
            .map(|l| l.outstanding_balance)
            .sum()
    }

    // ── Internal Helpers ─────────────────────────────────

    /// Calculate the monetary value of a collateral.
    fn collateral_value(&self, collateral: &Collateral) -> u64 {
        match collateral {
            Collateral::Skill { level, .. } => *level * self.config.skill_collateral_value,
            Collateral::Reputation { score } => {
                (*score * self.config.reputation_collateral_value as f64) as u64
            }
        }
    }

    fn emit(&self, event: WorldEvent) {
        if let Some(ref bus) = self.event_bus {
            bus.emit(event);
        }
    }
}

// ── Tests ─────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_system() -> BankingSystem {
        BankingSystem::new(CentralBankConfig::default())
    }

    // ── Account Management ───────────────────────────────

    #[test]
    fn test_open_savings_account() {
        let mut sys = make_system();
        let account = sys.open_account("alice", BankAccountType::Savings, "Alice Savings", 0).unwrap();
        assert_eq!(account.owner_id, "alice");
        assert_eq!(account.account_type, BankAccountType::Savings);
        assert_eq!(sys.get_balance(account.id), Some(0));
    }

    #[test]
    fn test_open_checking_account() {
        let mut sys = make_system();
        let account = sys.open_account("bob", BankAccountType::Checking, "Bob Checking", 0).unwrap();
        assert_eq!(account.account_type, BankAccountType::Checking);
    }

    #[test]
    fn test_open_both_account_types() {
        let mut sys = make_system();
        let savings = sys.open_account("alice", BankAccountType::Savings, "Alice Savings", 0).unwrap();
        let checking = sys.open_account("alice", BankAccountType::Checking, "Alice Checking", 0).unwrap();
        assert_ne!(savings.id, checking.id);
    }

    #[test]
    fn test_duplicate_account_type_rejected() {
        let mut sys = make_system();
        sys.open_account("alice", BankAccountType::Savings, "Alice Savings", 0).unwrap();
        let result = sys.open_account("alice", BankAccountType::Savings, "Alice Savings 2", 0);
        assert!(result.is_err());
    }

    #[test]
    fn test_list_accounts() {
        let mut sys = make_system();
        sys.open_account("alice", BankAccountType::Savings, "Alice Savings", 0).unwrap();
        sys.open_account("alice", BankAccountType::Checking, "Alice Checking", 0).unwrap();
        sys.open_account("bob", BankAccountType::Savings, "Bob Savings", 0).unwrap();
        assert_eq!(sys.list_accounts().len(), 3);
        assert_eq!(sys.get_accounts_by_owner("alice").len(), 2);
        assert_eq!(sys.get_accounts_by_owner("bob").len(), 1);
    }

    // ── Deposit / Withdraw ───────────────────────────────

    #[test]
    fn test_deposit_and_withdraw() {
        let mut sys = make_system();
        // Give alice some initial money.
        sys.ledger.create_account(Account::new_agent("alice", "Alice"));
        sys.ledger.set_balance_genesis("alice", Currency::Money, 1000);

        let account = sys.open_account("alice", BankAccountType::Savings, "Alice Savings", 0).unwrap();

        // Deposit 500.
        let result = sys.deposit(account.id, "alice", 500, 1).unwrap();
        assert_eq!(result.amount, 500);
        assert_eq!(result.new_balance, 500);

        // Alice's wallet should be reduced.
        assert_eq!(sys.ledger.get_balance("alice", Currency::Money), 500);

        // Withdraw 200.
        let result = sys.withdraw(account.id, "alice", 200, 2).unwrap();
        assert_eq!(result.amount, 200);
        assert_eq!(result.new_balance, 300);
        assert_eq!(sys.ledger.get_balance("alice", Currency::Money), 700);
    }

    #[test]
    fn test_withdraw_insufficient_funds() {
        let mut sys = make_system();
        let account = sys.open_account("alice", BankAccountType::Checking, "Alice Checking", 0).unwrap();
        let result = sys.withdraw(account.id, "alice", 100, 1);
        assert!(result.is_err());
    }

    #[test]
    fn test_deposit_to_nonexistent_account() {
        let mut sys = make_system();
        let result = sys.deposit(Uuid::new_v4(), "alice", 100, 1);
        assert!(result.is_err());
    }

    // ── Loan Lifecycle ───────────────────────────────────

    #[test]
    fn test_full_loan_lifecycle() {
        let mut sys = make_system();
        // Setup borrower.
        sys.ledger.create_account(Account::new_agent("alice", "Alice"));

        let collateral = Collateral::Skill {
            skill_name: "trading".to_string(),
            level: 10,
        };
        // Collateral value = 10 * 100 = 1000, max loan = 1000 * 0.7 = 700.

        // Apply.
        let application = sys.apply_for_loan("alice", 500, 100, Some(collateral), 1).unwrap();
        assert_eq!(application.status, LoanStatus::Pending);
        assert_eq!(application.principal, 500);

        // Approve.
        let loan = sys.approve_loan(application.loan_id, 2).unwrap();
        assert_eq!(loan.status, LoanStatus::Approved);

        // Disburse.
        let loan = sys.disburse_loan(application.loan_id, 3).unwrap();
        assert_eq!(loan.status, LoanStatus::Active);
        assert_eq!(loan.outstanding_balance, 500);
        assert_eq!(loan.due_tick, Some(103));
        // Alice should now have 500 Money.
        assert_eq!(sys.ledger.get_balance("alice", Currency::Money), 500);

        // Repay 200.
        let repayment = sys.repay_loan(application.loan_id, 200, 10).unwrap();
        assert_eq!(repayment.amount_paid, 200);
        assert_eq!(repayment.outstanding_balance, 300);
        assert!(!repayment.fully_repaid);

        // Repay the rest.
        let repayment = sys.repay_loan(application.loan_id, 300, 11).unwrap();
        assert_eq!(repayment.amount_paid, 300);
        assert!(repayment.fully_repaid);

        // Loan should now be Repaid.
        let loan = sys.get_loan(application.loan_id).unwrap();
        assert_eq!(loan.status, LoanStatus::Repaid);
    }

    #[test]
    fn test_loan_collateral_insufficient() {
        let mut sys = make_system();
        let collateral = Collateral::Skill {
            skill_name: "trading".to_string(),
            level: 1, // value = 100, max loan = 70
        };
        let result = sys.apply_for_loan("alice", 500, 100, Some(collateral), 1);
        assert!(result.is_err());
    }

    #[test]
    fn test_loan_zero_amount_rejected() {
        let mut sys = make_system();
        let result = sys.apply_for_loan("alice", 0, 100, None, 1);
        assert!(result.is_err());
    }

    #[test]
    fn test_approve_non_pending_rejected() {
        let mut sys = make_system();
        sys.ledger.create_account(Account::new_agent("alice", "Alice"));
        let app = sys.apply_for_loan("alice", 100, 50, None, 1).unwrap();
        sys.approve_loan(app.loan_id, 2).unwrap();
        // Try to approve again.
        let result = sys.approve_loan(app.loan_id, 3);
        assert!(result.is_err());
    }

    #[test]
    fn test_disburse_non_approved_rejected() {
        let mut sys = make_system();
        let app = sys.apply_for_loan("alice", 100, 50, None, 1).unwrap();
        // Try to disburse without approving.
        let result = sys.disburse_loan(app.loan_id, 2);
        assert!(result.is_err());
    }

    #[test]
    fn test_repay_insufficient_funds() {
        let mut sys = make_system();
        sys.ledger.create_account(Account::new_agent("alice", "Alice"));
        let app = sys.apply_for_loan("alice", 500, 100, None, 1).unwrap();
        sys.approve_loan(app.loan_id, 2).unwrap();
        sys.disburse_loan(app.loan_id, 3).unwrap();

        // Alice has 500 but loan was also 500, so she has the funds.
        // Let's spend it all first.
        sys.ledger.set_balance_genesis("alice", Currency::Money, 0);
        let result = sys.repay_loan(app.loan_id, 100, 10);
        assert!(result.is_err());
    }

    // ── Interest ─────────────────────────────────────────

    #[test]
    fn test_savings_interest() {
        let mut sys = make_system();
        sys.ledger.create_account(Account::new_agent("alice", "Alice"));

        let account = sys.open_account("alice", BankAccountType::Savings, "Alice Savings", 0).unwrap();
        sys.ledger.set_balance_genesis("alice", Currency::Money, 10000);
        sys.deposit(account.id, "alice", 10000, 1).unwrap();

        // Accrue interest.
        let results = sys.accrue_savings_interest(2);
        assert_eq!(results.len(), 1);
        // 10000 * 0.0005 = 5
        assert_eq!(results[0].interest, 5);
        assert_eq!(results[0].new_balance, 10005);
    }

    #[test]
    fn test_checking_no_interest() {
        let mut sys = make_system();
        sys.ledger.create_account(Account::new_agent("alice", "Alice"));

        let account = sys.open_account("alice", BankAccountType::Checking, "Alice Checking", 0).unwrap();
        sys.ledger.set_balance_genesis("alice", Currency::Money, 10000);
        sys.deposit(account.id, "alice", 10000, 1).unwrap();

        // Checking accounts should not accrue interest.
        let results = sys.accrue_savings_interest(2);
        assert!(results.is_empty());
    }

    #[test]
    fn test_loan_interest_accrual() {
        let mut sys = make_system();
        sys.ledger.create_account(Account::new_agent("alice", "Alice"));

        let app = sys.apply_for_loan("alice", 1000, 100, None, 1).unwrap();
        sys.approve_loan(app.loan_id, 2).unwrap();
        sys.disburse_loan(app.loan_id, 3).unwrap();

        // Accrue interest.
        sys.accrue_loan_interest(4);

        let loan = sys.get_loan(app.loan_id).unwrap();
        // 1000 * 0.001 = 1
        assert_eq!(loan.outstanding_balance, 1001);
    }

    // ── Bad Debt / Auto-Deduction ────────────────────────

    #[test]
    fn test_overdue_loan_auto_deduction() {
        let mut sys = make_system();
        sys.ledger.create_account(Account::new_agent("alice", "Alice"));

        let app = sys.apply_for_loan("alice", 1000, 10, None, 1).unwrap();
        sys.approve_loan(app.loan_id, 2).unwrap();
        sys.disburse_loan(app.loan_id, 3).unwrap();

        // Alice has 1000 Money from loan. Due tick is 13.
        // Move past due tick + grace period (10 ticks).
        let results = sys.process_overdue_loans(14);
        // First overdue tick — within grace period, no deduction yet.
        assert!(results.is_empty());

        let loan = sys.get_loan(app.loan_id).unwrap();
        assert_eq!(loan.status, LoanStatus::Defaulted);

        // Move past grace period.
        for _ in 0..sys.config.grace_ticks {
            sys.process_overdue_loans(100);
        }
        let results = sys.process_overdue_loans(100);
        // Now auto-deduction should happen.
        assert!(!results.is_empty());
        assert!(results[0].amount_paid > 0);
    }

    // ── Central Bank Operations ──────────────────────────

    #[test]
    fn test_adjust_rates() {
        let mut sys = make_system();
        let result = sys.adjust_rates(0.001, 0.002);
        assert_eq!(result.new_savings_rate, 0.001);
        assert_eq!(result.new_loan_rate, 0.002);
        assert_eq!(sys.config().savings_rate, 0.001);
        assert_eq!(sys.config().loan_rate, 0.002);
    }

    #[test]
    fn test_mint_money() {
        let mut sys = make_system();
        let result = sys.mint_money(5000, 1);
        assert_eq!(result.amount, 5000);
        assert!(result.total_money_supply >= 5000);
    }

    #[test]
    fn test_write_off_bad_debt() {
        let mut sys = make_system();
        sys.ledger.create_account(Account::new_agent("alice", "Alice"));

        let app = sys.apply_for_loan("alice", 1000, 10, None, 1).unwrap();
        sys.approve_loan(app.loan_id, 2).unwrap();
        sys.disburse_loan(app.loan_id, 3).unwrap();

        // Move past due.
        sys.process_overdue_loans(14);
        let loan = sys.get_loan(app.loan_id).unwrap();
        assert_eq!(loan.status, LoanStatus::Defaulted);

        // Write off.
        let result = sys.write_off_bad_debt(app.loan_id, 100).unwrap();
        assert_eq!(result.amount_written_off, 1000);

        let loan = sys.get_loan(app.loan_id).unwrap();
        assert_eq!(loan.status, LoanStatus::WrittenOff);
        assert_eq!(loan.outstanding_balance, 0);
    }

    // ── Queries ──────────────────────────────────────────

    #[test]
    fn test_list_loans_by_borrower() {
        let mut sys = make_system();
        sys.apply_for_loan("alice", 100, 50, None, 1).unwrap();
        sys.apply_for_loan("alice", 200, 50, None, 2).unwrap();
        sys.apply_for_loan("bob", 300, 50, None, 3).unwrap();

        assert_eq!(sys.list_loans(Some("alice"), None).len(), 2);
        assert_eq!(sys.list_loans(Some("bob"), None).len(), 1);
        assert_eq!(sys.list_loans(None, None).len(), 3);
    }

    #[test]
    fn test_list_loans_by_status() {
        let mut sys = make_system();
        sys.apply_for_loan("alice", 100, 50, None, 1).unwrap();
        let app2 = sys.apply_for_loan("bob", 200, 50, None, 2).unwrap();
        sys.approve_loan(app2.loan_id, 3).unwrap();

        assert_eq!(sys.list_loans(None, Some(LoanStatus::Pending)).len(), 1);
        assert_eq!(sys.list_loans(None, Some(LoanStatus::Approved)).len(), 1);
    }

    #[test]
    fn test_total_loan_debt() {
        let mut sys = make_system();
        sys.ledger.create_account(Account::new_agent("alice", "Alice"));
        sys.ledger.create_account(Account::new_agent("bob", "Bob"));

        let app1 = sys.apply_for_loan("alice", 500, 100, None, 1).unwrap();
        sys.approve_loan(app1.loan_id, 2).unwrap();
        sys.disburse_loan(app1.loan_id, 3).unwrap();

        let app2 = sys.apply_for_loan("bob", 300, 100, None, 4).unwrap();
        sys.approve_loan(app2.loan_id, 5).unwrap();
        sys.disburse_loan(app2.loan_id, 6).unwrap();

        assert_eq!(sys.total_loan_debt(), 800);
    }

    // ── Collateral Value ─────────────────────────────────

    #[test]
    fn test_collateral_skill_value() {
        let sys = make_system();
        let col = Collateral::Skill {
            skill_name: "trading".to_string(),
            level: 10,
        };
        assert_eq!(sys.collateral_value(&col), 1000); // 10 * 100
    }

    #[test]
    fn test_collateral_reputation_value() {
        let sys = make_system();
        let col = Collateral::Reputation { score: 50.0 };
        assert_eq!(sys.collateral_value(&col), 2500); // 50 * 50
    }

    // ── Zero Amounts ─────────────────────────────────────

    #[test]
    fn test_zero_deposit_is_noop() {
        let mut sys = make_system();
        let account = sys.open_account("alice", BankAccountType::Savings, "Alice Savings", 0).unwrap();
        let result = sys.deposit(account.id, "alice", 0, 1).unwrap();
        assert_eq!(result.amount, 0);
    }

    #[test]
    fn test_zero_withdraw_is_noop() {
        let mut sys = make_system();
        let account = sys.open_account("alice", BankAccountType::Savings, "Alice Savings", 0).unwrap();
        let result = sys.withdraw(account.id, "alice", 0, 1).unwrap();
        assert_eq!(result.amount, 0);
    }

    #[test]
    fn test_zero_repayment_is_noop() {
        let mut sys = make_system();
        sys.ledger.create_account(Account::new_agent("alice", "Alice"));
        let app = sys.apply_for_loan("alice", 100, 50, None, 1).unwrap();
        sys.approve_loan(app.loan_id, 2).unwrap();
        sys.disburse_loan(app.loan_id, 3).unwrap();
        let result = sys.repay_loan(app.loan_id, 0, 10).unwrap();
        assert_eq!(result.amount_paid, 0);
        assert!(!result.fully_repaid);
    }
}
