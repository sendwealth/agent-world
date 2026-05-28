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
pub mod inheritance;
pub mod investment;
pub mod ledger;
pub mod marketplace;
pub mod mentorship;
pub mod reputation;
pub mod reward;
pub mod stock_market;
pub mod task;
pub mod token_burn;
pub mod tool_marketplace;
pub mod trust;

pub use banking::{
    BankAccount, BankAccountType, BankingError, BankingSystem, CentralBankConfig, Collateral,
    DepositResult, InterestPaymentResult, Loan, LoanApplicationResult, LoanStatus, MintResult,
    RateAdjustmentResult, RepaymentResult, WithdrawResult, WriteOffResult,
};
pub use escrow::{EscrowManager, EscrowRecord, EscrowStatus};
pub use inheritance::{
    Beneficiary, InheritanceConfig, InheritanceResult, InheritanceSystem, ValueExperience, Will,
};
pub use investment::{
    BuySharesRequest, CloseInvestmentRequest, CreateProductRequest, DistributeReturnsRequest,
    DividendDistribution, DividendRecipient, FreezeProductRequest, InvestmentConfig,
    InvestmentEntityType, InvestmentError, InvestmentPosition, InvestmentProduct, InvestmentStatus,
    InvestmentSystem, InvestmentTransaction, InvestmentTxType, LeaderboardEntry,
    ListTransactionsQuery, PortfolioEntry, PositionStatus, SellSharesRequest,
    UpdatePerformanceRequest,
};
pub use marketplace::{KnowledgeCategory, KnowledgeListing, ListingStatus, Marketplace};
pub use mentorship::{MentorshipConfig, MentorshipSession, MentorshipStatus, MentorshipSystem};
pub use reputation::{
    ReputationChangeReason, ReputationConfig, ReputationRankingEntry, ReputationSystem,
};
pub use reward::{
    CentralBank, Ledger, LedgerEntry, RewardConfig, RewardDistribution, RewardDistributor,
    TransactionType,
};
pub use stock_market::{
    DividendRecipient as StockDividendRecipient, DividendRecord, Order, OrderKind, OrderStatus,
    OrderType, ShareHolding, StockListing, StockMarket, StockMarketError, Trade,
};
pub use task::{
    Contribution, CoordinationTask, CoordinationTaskError, CoordinationTaskStatus, Task, TaskBoard,
    TaskError, TaskStatus,
};
pub use token_burn::{BurnResult, TokenBurnEngine};
pub use tool_marketplace::{
    RentalRecord, RentalStatus, ToolCategory, ToolListing, ToolListingMode, ToolListingStatus,
    ToolMarketplace, ToolMarketplaceError, ToolMarketplaceFilter, ToolMarketplaceSort,
    ToolPurchaseRecord, ToolRating,
};
pub use trust::{TrustConfig, TrustEdge, TrustNetwork, TrustScore};
