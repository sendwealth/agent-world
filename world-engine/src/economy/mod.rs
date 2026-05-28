//! # Economy Subsystem
//!
//! Token economy with banking, stock market, investment, marketplace,
//! escrow, reputation, inheritance, mentorship, and trust network.
//!
//! Key types: BankingSystem, StockMarket, Marketplace, ReputationSystem,
//!            InvestmentSystem, TaskBoard, EscrowManager, TrustNetwork
//! Depends on: world (WorldState, EventBus), organization (OrganizationStore)
//!
//! Sub-modules: banking, escrow, investment, ledger, inheritance,
//!              marketplace, mentorship, reputation, reward,
//!              stock_market, task, token_burn, trust
//!
pub mod banking;
pub mod escrow;
pub mod investment;
pub mod ledger;
pub mod inheritance;
pub mod marketplace;
pub mod mentorship;
pub mod reputation;
pub mod reward;
pub mod stock_market;
pub mod task;
pub mod token_burn;
pub mod trust;

pub use banking::{
    BankingSystem, BankingError, BankAccount, BankAccountType,
    Loan, LoanStatus, Collateral, CentralBankConfig,
    DepositResult, WithdrawResult, LoanApplicationResult,
    RepaymentResult, InterestPaymentResult, RateAdjustmentResult,
    MintResult, WriteOffResult,
};
pub use escrow::{EscrowManager, EscrowRecord, EscrowStatus};
pub use inheritance::{InheritanceSystem, InheritanceConfig, Will, Beneficiary, InheritanceResult, ValueExperience};
pub use investment::{
    InvestmentSystem, InvestmentConfig, InvestmentError,
    InvestmentProduct, InvestmentPosition, InvestmentTransaction,
    InvestmentTxType, InvestmentStatus, InvestmentEntityType, PositionStatus,
    DividendDistribution, DividendRecipient,
    PortfolioEntry, LeaderboardEntry,
    CreateProductRequest, BuySharesRequest, SellSharesRequest,
    CloseInvestmentRequest, DistributeReturnsRequest,
    UpdatePerformanceRequest, FreezeProductRequest,
    ListTransactionsQuery,
};
pub use marketplace::{Marketplace, KnowledgeListing, KnowledgeCategory, ListingStatus};
pub use mentorship::{MentorshipSystem, MentorshipConfig, MentorshipSession, MentorshipStatus};
pub use reputation::{
    ReputationConfig, ReputationSystem, ReputationRankingEntry,
    ReputationChangeReason,
};
pub use reward::{
    RewardConfig, RewardDistributor, RewardDistribution,
    CentralBank, Ledger, LedgerEntry, TransactionType,
};
pub use stock_market::{
    StockMarket, StockListing, ShareHolding, Order, Trade,
    DividendRecord, DividendRecipient as StockDividendRecipient, OrderType, OrderKind,
    OrderStatus, StockMarketError,
};
pub use task::{TaskBoard, Task, TaskStatus, TaskError};
pub use token_burn::{TokenBurnEngine, BurnResult};
pub use trust::{TrustNetwork, TrustConfig, TrustEdge, TrustScore};
