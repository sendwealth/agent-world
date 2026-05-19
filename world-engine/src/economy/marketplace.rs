use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::world::enums::Currency;
use crate::world::event::WorldEvent;
use crate::world::state::EventBus;

// ── Knowledge Category ────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KnowledgeCategory {
    Strategy,
    Tactics,
    Survival,
    Economy,
    Social,
    Technical,
    General,
}

impl KnowledgeCategory {
    pub fn all() -> Vec<KnowledgeCategory> {
        vec![
            KnowledgeCategory::Strategy,
            KnowledgeCategory::Tactics,
            KnowledgeCategory::Survival,
            KnowledgeCategory::Economy,
            KnowledgeCategory::Social,
            KnowledgeCategory::Technical,
            KnowledgeCategory::General,
        ]
    }
}

// ── Listing Status ────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ListingStatus {
    /// Listed and available for purchase.
    Active,
    /// Temporarily delisted by the publisher.
    Inactive,
    /// Removed from the marketplace.
    Delisted,
}

// ── Knowledge Listing ─────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeListing {
    pub id: Uuid,
    pub title: String,
    pub description: String,
    pub category: KnowledgeCategory,
    pub content_hash: String,
    pub price: u64,
    pub currency: Currency,
    pub publisher_id: String,
    pub status: ListingStatus,
    /// Number of times this knowledge has been purchased.
    pub purchase_count: u64,
    /// Sum of all ratings (for computing average).
    pub rating_sum: f64,
    /// Number of ratings.
    pub rating_count: u64,
    /// Tags for search/filter.
    pub tags: Vec<String>,
    /// Tick when the listing was created.
    pub created_tick: u64,
}

impl KnowledgeListing {
    /// Compute the average rating (0.0 if no ratings).
    pub fn average_rating(&self) -> f64 {
        if self.rating_count == 0 {
            0.0
        } else {
            self.rating_sum / self.rating_count as f64
        }
    }
}

// ── Purchase Record ───────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PurchaseRecord {
    pub id: Uuid,
    pub listing_id: Uuid,
    pub buyer_id: String,
    pub seller_id: String,
    pub price: u64,
    pub currency: Currency,
    pub tick: u64,
}

// ── Rating ────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rating {
    pub id: Uuid,
    pub listing_id: Uuid,
    pub rater_id: String,
    /// Rating value 1-5.
    pub score: u8,
    /// Optional text review.
    pub review: Option<String>,
    pub tick: u64,
}

// ── Errors ────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MarketplaceError {
    NotFound(String),
    /// The listing is not active.
    ListingInactive,
    /// The listing has been delisted.
    ListingDelisted,
    /// Buyer does not have enough balance.
    InsufficientBalance { required: u64, available: u64 },
    /// The buyer is the publisher (cannot buy own knowledge).
    SelfPurchase,
    /// The listing already exists with the same content hash from the same publisher.
    DuplicateContent,
    /// Invalid rating score (must be 1-5).
    InvalidRating,
    /// The buyer has already rated this listing.
    AlreadyRated,
    /// The buyer has not purchased this listing.
    NotPurchased,
    /// Invalid price (must be > 0).
    InvalidPrice,
    /// The caller is not authorized to perform this action.
    Unauthorized(String),
}

impl std::fmt::Display for MarketplaceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MarketplaceError::NotFound(id) => write!(f, "listing not found: {}", id),
            MarketplaceError::ListingInactive => write!(f, "listing is not active"),
            MarketplaceError::ListingDelisted => write!(f, "listing has been delisted"),
            MarketplaceError::InsufficientBalance { required, available } => {
                write!(f, "insufficient balance: required {}, available {}", required, available)
            }
            MarketplaceError::SelfPurchase => write!(f, "cannot purchase your own knowledge"),
            MarketplaceError::DuplicateContent => write!(f, "duplicate content listing"),
            MarketplaceError::InvalidRating => write!(f, "rating must be between 1 and 5"),
            MarketplaceError::AlreadyRated => write!(f, "already rated this listing"),
            MarketplaceError::NotPurchased => write!(f, "must purchase before rating"),
            MarketplaceError::InvalidPrice => write!(f, "price must be greater than 0"),
            MarketplaceError::Unauthorized(msg) => write!(f, "unauthorized: {}", msg),
        }
    }
}

impl std::error::Error for MarketplaceError {}

// ── Search / Filter ───────────────────────────────────────

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MarketplaceFilter {
    /// Filter by category.
    pub category: Option<KnowledgeCategory>,
    /// Filter by publisher.
    pub publisher_id: Option<String>,
    /// Minimum price.
    pub min_price: Option<u64>,
    /// Maximum price.
    pub max_price: Option<u64>,
    /// Filter by tag.
    pub tag: Option<String>,
    /// Text search on title (case-insensitive contains).
    pub query: Option<String>,
    /// Only show listings with at least this many purchases.
    pub min_purchases: Option<u64>,
    /// Only show listings with at least this average rating.
    pub min_rating: Option<f64>,
    /// Sort order.
    pub sort: Option<MarketplaceSort>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MarketplaceSort {
    /// Newest first.
    #[default]
    Newest,
    /// Oldest first.
    Oldest,
    /// Price low to high.
    PriceAsc,
    /// Price high to low.
    PriceDesc,
    /// Highest rated first.
    RatingDesc,
    /// Most purchased first.
    PurchasesDesc,
}

// ── Marketplace ───────────────────────────────────────────

pub struct Marketplace {
    listings: HashMap<Uuid, KnowledgeListing>,
    purchases: Vec<PurchaseRecord>,
    ratings: HashMap<Uuid, Vec<Rating>>,
    /// Agent balances for the marketplace.
    balances: HashMap<String, u64>,
    /// Track which agents have purchased which listings.
    purchase_index: HashSet<(String, Uuid)>,
    event_bus: Option<EventBus>,
}

impl Marketplace {
    pub fn new() -> Self {
        Self {
            listings: HashMap::new(),
            purchases: Vec::new(),
            ratings: HashMap::new(),
            balances: HashMap::new(),
            purchase_index: HashSet::new(),
            event_bus: None,
        }
    }

    pub fn with_event_bus(event_bus: EventBus) -> Self {
        Self {
            listings: HashMap::new(),
            purchases: Vec::new(),
            ratings: HashMap::new(),
            balances: HashMap::new(),
            purchase_index: HashSet::new(),
            event_bus: Some(event_bus),
        }
    }

    // ── Balance helpers ────────────────────────────────────

    pub fn set_balance(&mut self, agent: &str, amount: u64) {
        self.balances.insert(agent.to_string(), amount);
    }

    pub fn get_balance(&self, agent: &str) -> u64 {
        self.balances.get(agent).copied().unwrap_or(0)
    }

    // ── Query ─────────────────────────────────────────────

    pub fn get(&self, id: Uuid) -> Option<&KnowledgeListing> {
        self.listings.get(&id)
    }

    /// List all active listings.
    pub fn list_active(&self) -> Vec<&KnowledgeListing> {
        self.listings
            .values()
            .filter(|l| l.status == ListingStatus::Active)
            .collect()
    }

    /// List all listings regardless of status.
    pub fn list_all(&self) -> Vec<&KnowledgeListing> {
        self.listings.values().collect()
    }

    /// Search/filter listings with optional sorting.
    pub fn search(&self, filter: &MarketplaceFilter) -> Vec<&KnowledgeListing> {
        let mut results: Vec<&KnowledgeListing> = self.listings
            .values()
            .filter(|l| l.status == ListingStatus::Active)
            .filter(|l| {
                match filter.category {
                    Some(ref cat) => l.category == *cat,
                    None => true,
                }
            })
            .filter(|l| {
                match filter.publisher_id {
                    Some(ref pid) => l.publisher_id == *pid,
                    None => true,
                }
            })
            .filter(|l| {
                match filter.min_price {
                    Some(min) => l.price >= min,
                    None => true,
                }
            })
            .filter(|l| {
                match filter.max_price {
                    Some(max) => l.price <= max,
                    None => true,
                }
            })
            .filter(|l| {
                match filter.tag {
                    Some(ref tag) => l.tags.iter().any(|t| t.eq_ignore_ascii_case(tag)),
                    None => true,
                }
            })
            .filter(|l| {
                match filter.query {
                    Some(ref query) => l.title.to_lowercase().contains(&query.to_lowercase()),
                    None => true,
                }
            })
            .filter(|l| {
                match filter.min_purchases {
                    Some(min) => l.purchase_count >= min,
                    None => true,
                }
            })
            .filter(|l| {
                match filter.min_rating {
                    Some(min) => l.average_rating() >= min,
                    None => true,
                }
            })
            .collect();

        let sort = filter.sort.unwrap_or_default();
        results.sort_by(|a, b| match sort {
            MarketplaceSort::Newest => b.created_tick.cmp(&a.created_tick),
            MarketplaceSort::Oldest => a.created_tick.cmp(&b.created_tick),
            MarketplaceSort::PriceAsc => a.price.cmp(&b.price),
            MarketplaceSort::PriceDesc => b.price.cmp(&a.price),
            MarketplaceSort::RatingDesc => {
                b.average_rating().partial_cmp(&a.average_rating()).unwrap_or(std::cmp::Ordering::Equal)
            }
            MarketplaceSort::PurchasesDesc => b.purchase_count.cmp(&a.purchase_count),
        });

        results
    }

    /// Get all purchases for a listing.
    pub fn listing_purchases(&self, listing_id: Uuid) -> Vec<&PurchaseRecord> {
        self.purchases
            .iter()
            .filter(|p| p.listing_id == listing_id)
            .collect()
    }

    /// Get all purchases by a buyer.
    pub fn buyer_purchases(&self, buyer_id: &str) -> Vec<&PurchaseRecord> {
        self.purchases
            .iter()
            .filter(|p| p.buyer_id == buyer_id)
            .collect()
    }

    /// Get all ratings for a listing.
    pub fn listing_ratings(&self, listing_id: Uuid) -> Vec<&Rating> {
        self.ratings
            .get(&listing_id)
            .map(|v| v.iter().collect())
            .unwrap_or_default()
    }

    /// Check if a buyer has purchased a listing.
    pub fn has_purchased(&self, buyer_id: &str, listing_id: Uuid) -> bool {
        self.purchase_index.contains(&(buyer_id.to_string(), listing_id))
    }

    /// Check if a buyer has rated a listing.
    pub fn has_rated(&self, rater_id: &str, listing_id: Uuid) -> bool {
        self.ratings
            .get(&listing_id)
            .map(|v| v.iter().any(|r| r.rater_id == rater_id))
            .unwrap_or(false)
    }

    // ── CRUD ──────────────────────────────────────────────

    /// Publish a new knowledge listing.
    /// Price must be > 0 and expressed in the given currency.
    #[allow(clippy::too_many_arguments)]
    pub fn publish_listing(
        &mut self,
        title: String,
        description: String,
        category: KnowledgeCategory,
        content_hash: String,
        price: u64,
        currency: Currency,
        publisher_id: String,
        tags: Vec<String>,
        created_tick: u64,
    ) -> Result<Uuid, MarketplaceError> {
        if price == 0 {
            return Err(MarketplaceError::InvalidPrice);
        }
        if title.is_empty() {
            return Err(MarketplaceError::NotFound("title is required".into()));
        }

        // Check for duplicate content from the same publisher
        let is_duplicate = self.listings.values().any(|l| {
            l.publisher_id == publisher_id
                && l.content_hash == content_hash
                && l.status != ListingStatus::Delisted
        });
        if is_duplicate {
            return Err(MarketplaceError::DuplicateContent);
        }

        let id = Uuid::new_v4();
        let listing = KnowledgeListing {
            id,
            title,
            description,
            category,
            content_hash,
            price,
            currency,
            publisher_id: publisher_id.clone(),
            status: ListingStatus::Active,
            purchase_count: 0,
            rating_sum: 0.0,
            rating_count: 0,
            tags,
            created_tick,
        };
        self.listings.insert(id, listing);

        self.emit(WorldEvent::KnowledgeListed {
            listing_id: id.to_string(),
            publisher: publisher_id,
            price,
            currency,
        });

        Ok(id)
    }

    /// Update a listing's price, status, or tags.
    pub fn update_listing(
        &mut self,
        listing_id: Uuid,
        publisher_id: &str,
        price: Option<u64>,
        status: Option<ListingStatus>,
        tags: Option<Vec<String>>,
    ) -> Result<(), MarketplaceError> {
        let listing = self.listings.get_mut(&listing_id)
            .ok_or_else(|| MarketplaceError::NotFound(listing_id.to_string()))?;

        if listing.publisher_id != publisher_id {
            return Err(MarketplaceError::Unauthorized("only the publisher can update".into()));
        }

        if let Some(p) = price {
            if p == 0 {
                return Err(MarketplaceError::InvalidPrice);
            }
            listing.price = p;
        }
        if let Some(s) = status {
            listing.status = s;
        }
        if let Some(t) = tags {
            listing.tags = t;
        }

        Ok(())
    }

    /// Delist a listing (permanent removal from active marketplace).
    pub fn delist_listing(
        &mut self,
        listing_id: Uuid,
        publisher_id: &str,
    ) -> Result<(), MarketplaceError> {
        let listing = self.listings.get_mut(&listing_id)
            .ok_or_else(|| MarketplaceError::NotFound(listing_id.to_string()))?;

        if listing.publisher_id != publisher_id {
            return Err(MarketplaceError::Unauthorized("only the publisher can delist".into()));
        }

        listing.status = ListingStatus::Delisted;

        self.emit(WorldEvent::KnowledgeDelisted {
            listing_id: listing_id.to_string(),
        });

        Ok(())
    }

    // ── Purchase ──────────────────────────────────────────

    /// Purchase a knowledge listing.
    /// Transfers the price from buyer to seller, and records the purchase.
    pub fn purchase_listing(
        &mut self,
        listing_id: Uuid,
        buyer_id: &str,
        tick: u64,
    ) -> Result<PurchaseRecord, MarketplaceError> {
        // Validate listing and extract needed data
        let (price, currency, publisher_id) = {
            let listing = self.listings.get(&listing_id)
                .ok_or_else(|| MarketplaceError::NotFound(listing_id.to_string()))?;
            match listing.status {
                ListingStatus::Active => {}
                ListingStatus::Inactive => return Err(MarketplaceError::ListingInactive),
                ListingStatus::Delisted => return Err(MarketplaceError::ListingDelisted),
            }
            if listing.publisher_id == buyer_id {
                return Err(MarketplaceError::SelfPurchase);
            }
            (listing.price, listing.currency, listing.publisher_id.clone())
        };

        // Check buyer balance
        let buyer_balance = self.get_balance(buyer_id);
        if buyer_balance < price {
            return Err(MarketplaceError::InsufficientBalance {
                required: price,
                available: buyer_balance,
            });
        }

        // Transfer funds
        self.balances.insert(
            buyer_id.to_string(),
            buyer_balance - price,
        );
        let seller_balance = self.get_balance(&publisher_id);
        self.balances.insert(
            publisher_id.clone(),
            seller_balance + price,
        );

        // Update purchase count
        let listing = self.listings.get_mut(&listing_id).unwrap();
        listing.purchase_count += 1;

        // Record purchase
        let record = PurchaseRecord {
            id: Uuid::new_v4(),
            listing_id,
            buyer_id: buyer_id.to_string(),
            seller_id: publisher_id.clone(),
            price,
            currency,
            tick,
        };
        self.purchase_index.insert((buyer_id.to_string(), listing_id));
        self.purchases.push(record.clone());

        self.emit(WorldEvent::KnowledgePurchased {
            listing_id: listing_id.to_string(),
            buyer: buyer_id.to_string(),
            seller: publisher_id,
            price,
            currency,
        });

        Ok(record)
    }

    // ── Rating ────────────────────────────────────────────

    /// Rate a purchased knowledge listing.
    /// Score must be 1-5. The rater must have purchased the listing and not rated it already.
    pub fn rate_listing(
        &mut self,
        listing_id: Uuid,
        rater_id: &str,
        score: u8,
        review: Option<String>,
        tick: u64,
    ) -> Result<Uuid, MarketplaceError> {
        if !(1..=5).contains(&score) {
            return Err(MarketplaceError::InvalidRating);
        }

        // Must have purchased
        if !self.has_purchased(rater_id, listing_id) {
            return Err(MarketplaceError::NotPurchased);
        }

        // Must not have rated already
        if self.has_rated(rater_id, listing_id) {
            return Err(MarketplaceError::AlreadyRated);
        }

        // Validate listing exists
        if !self.listings.contains_key(&listing_id) {
            return Err(MarketplaceError::NotFound(listing_id.to_string()));
        }

        let rating_id = Uuid::new_v4();
        let rating = Rating {
            id: rating_id,
            listing_id,
            rater_id: rater_id.to_string(),
            score,
            review,
            tick,
        };

        // Update listing rating stats
        let listing = self.listings.get_mut(&listing_id).unwrap();
        listing.rating_sum += score as f64;
        listing.rating_count += 1;
        let avg = listing.average_rating();

        self.ratings
            .entry(listing_id)
            .or_default()
            .push(rating);

        self.emit(WorldEvent::KnowledgeRated {
            listing_id: listing_id.to_string(),
            rater: rater_id.to_string(),
            score,
            average_rating: avg,
        });

        Ok(rating_id)
    }

    // ── Transfer ──────────────────────────────────────────

    /// Transfer tokens between agents (for knowledge bartering outside listings).
    pub fn transfer(
        &mut self,
        from: &str,
        to: &str,
        amount: u64,
        currency: Currency,
    ) -> Result<(), MarketplaceError> {
        let from_balance = self.get_balance(from);
        if from_balance < amount {
            return Err(MarketplaceError::InsufficientBalance {
                required: amount,
                available: from_balance,
            });
        }
        self.balances.insert(from.to_string(), from_balance - amount);
        let to_balance = self.get_balance(to);
        self.balances.insert(to.to_string(), to_balance + amount);

        self.emit(WorldEvent::TransactionCompleted {
            from: from.to_string(),
            to: to.to_string(),
            amount,
            currency,
        });

        Ok(())
    }

    // ── Helpers ────────────────────────────────────────────

    fn emit(&self, event: WorldEvent) {
        if let Some(ref bus) = self.event_bus {
            bus.emit(event);
        }
    }
}

impl Default for Marketplace {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ─────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_marketplace() -> Marketplace {
        let mut mp = Marketplace::new();
        mp.set_balance("seller", 10_000);
        mp.set_balance("buyer", 5_000);
        mp
    }

    fn publish_default_listing(mp: &mut Marketplace) -> Uuid {
        mp.publish_listing(
            "How to Survive Winter".into(),
            "A guide to surviving the cold season.".into(),
            KnowledgeCategory::Survival,
            "hash_abc123".into(),
            100,
            Currency::Token,
            "seller".into(),
            vec!["winter".into(), "survival".into()],
            1,
        ).unwrap()
    }

    // ── Publish ────────────────────────────────────────────

    #[test]
    fn test_publish_listing() {
        let mut mp = make_marketplace();
        let id = publish_default_listing(&mut mp);
        let listing = mp.get(id).unwrap();
        assert_eq!(listing.title, "How to Survive Winter");
        assert_eq!(listing.price, 100);
        assert_eq!(listing.currency, Currency::Token);
        assert_eq!(listing.publisher_id, "seller");
        assert_eq!(listing.status, ListingStatus::Active);
        assert_eq!(listing.category, KnowledgeCategory::Survival);
        assert_eq!(listing.tags, vec!["winter", "survival"]);
        assert_eq!(listing.purchase_count, 0);
        assert_eq!(listing.average_rating(), 0.0);
    }

    #[test]
    fn test_publish_listing_zero_price_fails() {
        let mut mp = make_marketplace();
        let result = mp.publish_listing(
            "Free Knowledge".into(), "desc".into(),
            KnowledgeCategory::General, "hash".into(),
            0, Currency::Token, "seller".into(), vec![], 1,
        );
        assert!(matches!(result, Err(MarketplaceError::InvalidPrice)));
    }

    #[test]
    fn test_publish_listing_empty_title_fails() {
        let mut mp = make_marketplace();
        let result = mp.publish_listing(
            "".into(), "desc".into(),
            KnowledgeCategory::General, "hash".into(),
            50, Currency::Token, "seller".into(), vec![], 1,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_publish_listing_duplicate_content_fails() {
        let mut mp = make_marketplace();
        publish_default_listing(&mut mp);
        let result = mp.publish_listing(
            "Different Title".into(), "Different desc".into(),
            KnowledgeCategory::General, "hash_abc123".into(), // same hash
            200, Currency::Token, "seller".into(), vec![], 2,
        );
        assert!(matches!(result, Err(MarketplaceError::DuplicateContent)));
    }

    #[test]
    fn test_publish_listing_same_hash_different_publisher_ok() {
        let mut mp = make_marketplace();
        mp.set_balance("other_seller", 1000);
        publish_default_listing(&mut mp);
        let result = mp.publish_listing(
            "Other Guide".into(), "desc".into(),
            KnowledgeCategory::General, "hash_abc123".into(), // same hash
            50, Currency::Token, "other_seller".into(), vec![], 2,
        );
        assert!(result.is_ok());
    }

    // ── Purchase ───────────────────────────────────────────

    #[test]
    fn test_purchase_listing() {
        let mut mp = make_marketplace();
        let id = publish_default_listing(&mut mp);

        let record = mp.purchase_listing(id, "buyer", 5).unwrap();
        assert_eq!(record.price, 100);
        assert_eq!(record.buyer_id, "buyer");
        assert_eq!(record.seller_id, "seller");

        // Balances updated
        assert_eq!(mp.get_balance("buyer"), 4_900);
        assert_eq!(mp.get_balance("seller"), 10_100);

        // Purchase count incremented
        assert_eq!(mp.get(id).unwrap().purchase_count, 1);

        // Purchase tracked
        assert!(mp.has_purchased("buyer", id));
        assert!(!mp.has_purchased("seller", id));
    }

    #[test]
    fn test_purchase_self_fails() {
        let mut mp = make_marketplace();
        let id = publish_default_listing(&mut mp);
        let result = mp.purchase_listing(id, "seller", 5);
        assert!(matches!(result, Err(MarketplaceError::SelfPurchase)));
    }

    #[test]
    fn test_purchase_insufficient_balance() {
        let mut mp = make_marketplace();
        let id = publish_default_listing(&mut mp);
        mp.set_balance("poor_buyer", 50);
        let result = mp.purchase_listing(id, "poor_buyer", 5);
        assert!(matches!(result, Err(MarketplaceError::InsufficientBalance { .. })));
    }

    #[test]
    fn test_purchase_inactive_listing() {
        let mut mp = make_marketplace();
        let id = publish_default_listing(&mut mp);
        mp.update_listing(id, "seller", None, Some(ListingStatus::Inactive), None).unwrap();
        let result = mp.purchase_listing(id, "buyer", 5);
        assert!(matches!(result, Err(MarketplaceError::ListingInactive)));
    }

    #[test]
    fn test_purchase_delisted_listing() {
        let mut mp = make_marketplace();
        let id = publish_default_listing(&mut mp);
        mp.delist_listing(id, "seller").unwrap();
        let result = mp.purchase_listing(id, "buyer", 5);
        assert!(matches!(result, Err(MarketplaceError::ListingDelisted)));
    }

    #[test]
    fn test_purchase_nonexistent_listing() {
        let mut mp = Marketplace::new();
        let result = mp.purchase_listing(Uuid::new_v4(), "buyer", 5);
        assert!(matches!(result, Err(MarketplaceError::NotFound(_))));
    }

    #[test]
    fn test_multiple_purchases() {
        let mut mp = make_marketplace();
        mp.set_balance("buyer2", 5_000);
        let id = publish_default_listing(&mut mp);

        mp.purchase_listing(id, "buyer", 5).unwrap();
        mp.purchase_listing(id, "buyer2", 6).unwrap();

        assert_eq!(mp.get(id).unwrap().purchase_count, 2);
        assert_eq!(mp.get_balance("seller"), 10_200);
    }

    // ── Rating ─────────────────────────────────────────────

    #[test]
    fn test_rate_listing() {
        let mut mp = make_marketplace();
        let id = publish_default_listing(&mut mp);
        mp.purchase_listing(id, "buyer", 5).unwrap();

        let rating_id = mp.rate_listing(id, "buyer", 4, Some("Great guide!".into()), 10).unwrap();
        assert!(mp.has_rated("buyer", id));

        let listing = mp.get(id).unwrap();
        assert_eq!(listing.rating_count, 1);
        assert_eq!(listing.rating_sum, 4.0);
        assert_eq!(listing.average_rating(), 4.0);

        let ratings = mp.listing_ratings(id);
        assert_eq!(ratings.len(), 1);
        assert_eq!(ratings[0].score, 4);
        assert_eq!(ratings[0].review.as_deref(), Some("Great guide!"));
    }

    #[test]
    fn test_rate_without_purchase_fails() {
        let mut mp = make_marketplace();
        let id = publish_default_listing(&mut mp);
        let result = mp.rate_listing(id, "buyer", 5, None, 10);
        assert!(matches!(result, Err(MarketplaceError::NotPurchased)));
    }

    #[test]
    fn test_rate_twice_fails() {
        let mut mp = make_marketplace();
        let id = publish_default_listing(&mut mp);
        mp.purchase_listing(id, "buyer", 5).unwrap();
        mp.rate_listing(id, "buyer", 4, None, 10).unwrap();
        let result = mp.rate_listing(id, "buyer", 5, None, 11);
        assert!(matches!(result, Err(MarketplaceError::AlreadyRated)));
    }

    #[test]
    fn test_rate_invalid_score_zero() {
        let mut mp = make_marketplace();
        let id = publish_default_listing(&mut mp);
        mp.purchase_listing(id, "buyer", 5).unwrap();
        let result = mp.rate_listing(id, "buyer", 0, None, 10);
        assert!(matches!(result, Err(MarketplaceError::InvalidRating)));
    }

    #[test]
    fn test_rate_invalid_score_six() {
        let mut mp = make_marketplace();
        let id = publish_default_listing(&mut mp);
        mp.purchase_listing(id, "buyer", 5).unwrap();
        let result = mp.rate_listing(id, "buyer", 6, None, 10);
        assert!(matches!(result, Err(MarketplaceError::InvalidRating)));
    }

    #[test]
    fn test_average_rating_multiple_raters() {
        let mut mp = make_marketplace();
        mp.set_balance("buyer2", 5_000);
        mp.set_balance("buyer3", 5_000);
        let id = publish_default_listing(&mut mp);

        mp.purchase_listing(id, "buyer", 5).unwrap();
        mp.purchase_listing(id, "buyer2", 6).unwrap();
        mp.purchase_listing(id, "buyer3", 7).unwrap();

        mp.rate_listing(id, "buyer", 5, None, 10).unwrap();
        mp.rate_listing(id, "buyer2", 3, None, 10).unwrap();
        mp.rate_listing(id, "buyer3", 4, None, 10).unwrap();

        let listing = mp.get(id).unwrap();
        assert_eq!(listing.rating_count, 3);
        // (5 + 3 + 4) / 3 = 4.0
        assert!((listing.average_rating() - 4.0).abs() < f64::EPSILON);
    }

    // ── Update ─────────────────────────────────────────────

    #[test]
    fn test_update_listing_price() {
        let mut mp = make_marketplace();
        let id = publish_default_listing(&mut mp);
        mp.update_listing(id, "seller", Some(200), None, None).unwrap();
        assert_eq!(mp.get(id).unwrap().price, 200);
    }

    #[test]
    fn test_update_listing_tags() {
        let mut mp = make_marketplace();
        let id = publish_default_listing(&mut mp);
        mp.update_listing(id, "seller", None, None, Some(vec!["new_tag".into()])).unwrap();
        assert_eq!(mp.get(id).unwrap().tags, vec!["new_tag"]);
    }

    #[test]
    fn test_update_listing_wrong_publisher() {
        let mut mp = make_marketplace();
        let id = publish_default_listing(&mut mp);
        let result = mp.update_listing(id, "imposter", Some(200), None, None);
        assert!(matches!(result, Err(MarketplaceError::Unauthorized(_))));
    }

    #[test]
    fn test_update_listing_zero_price_fails() {
        let mut mp = make_marketplace();
        let id = publish_default_listing(&mut mp);
        let result = mp.update_listing(id, "seller", Some(0), None, None);
        assert!(matches!(result, Err(MarketplaceError::InvalidPrice)));
    }

    // ── Delist ─────────────────────────────────────────────

    #[test]
    fn test_delist_listing() {
        let mut mp = make_marketplace();
        let id = publish_default_listing(&mut mp);
        mp.delist_listing(id, "seller").unwrap();
        assert_eq!(mp.get(id).unwrap().status, ListingStatus::Delisted);
        // Not in active listings
        assert!(mp.list_active().is_empty());
    }

    #[test]
    fn test_delist_wrong_publisher_fails() {
        let mut mp = make_marketplace();
        let id = publish_default_listing(&mut mp);
        let result = mp.delist_listing(id, "imposter");
        assert!(matches!(result, Err(MarketplaceError::Unauthorized(_))));
    }

    // ── Search / Filter ────────────────────────────────────

    #[test]
    fn test_search_all_active() {
        let mut mp = make_marketplace();
        publish_default_listing(&mut mp);
        mp.publish_listing(
            "Trading Tips".into(), "How to trade.".into(),
            KnowledgeCategory::Economy, "hash2".into(),
            200, Currency::Token, "seller".into(), vec!["trading".into()], 2,
        ).unwrap();
        let results = mp.search(&MarketplaceFilter::default());
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_search_by_category() {
        let mut mp = make_marketplace();
        publish_default_listing(&mut mp);
        mp.publish_listing(
            "Trading Tips".into(), "How to trade.".into(),
            KnowledgeCategory::Economy, "hash2".into(),
            200, Currency::Token, "seller".into(), vec!["trading".into()], 2,
        ).unwrap();

        let filter = MarketplaceFilter {
            category: Some(KnowledgeCategory::Economy),
            ..Default::default()
        };
        let results = mp.search(&filter);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Trading Tips");
    }

    #[test]
    fn test_search_by_price_range() {
        let mut mp = make_marketplace();
        publish_default_listing(&mut mp); // price 100
        mp.publish_listing(
            "Trading Tips".into(), "How to trade.".into(),
            KnowledgeCategory::Economy, "hash2".into(),
            200, Currency::Token, "seller".into(), vec!["trading".into()], 2,
        ).unwrap();

        let filter = MarketplaceFilter {
            min_price: Some(150),
            max_price: Some(250),
            ..Default::default()
        };
        let results = mp.search(&filter);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].price, 200);
    }

    #[test]
    fn test_search_by_tag() {
        let mut mp = make_marketplace();
        publish_default_listing(&mut mp); // tags: winter, survival
        mp.publish_listing(
            "Trading Tips".into(), "How to trade.".into(),
            KnowledgeCategory::Economy, "hash2".into(),
            200, Currency::Token, "seller".into(), vec!["trading".into()], 2,
        ).unwrap();

        let filter = MarketplaceFilter {
            tag: Some("winter".into()),
            ..Default::default()
        };
        let results = mp.search(&filter);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "How to Survive Winter");
    }

    #[test]
    fn test_search_by_query() {
        let mut mp = make_marketplace();
        publish_default_listing(&mut mp); // "How to Survive Winter"
        mp.publish_listing(
            "Trading Tips".into(), "How to trade.".into(),
            KnowledgeCategory::Economy, "hash2".into(),
            200, Currency::Token, "seller".into(), vec!["trading".into()], 2,
        ).unwrap();

        let filter = MarketplaceFilter {
            query: Some("winter".into()),
            ..Default::default()
        };
        let results = mp.search(&filter);
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_search_by_publisher() {
        let mut mp = make_marketplace();
        mp.set_balance("other_seller", 1000);
        publish_default_listing(&mut mp);
        mp.publish_listing(
            "Other Knowledge".into(), "desc".into(),
            KnowledgeCategory::General, "hash3".into(),
            50, Currency::Token, "other_seller".into(), vec![], 3,
        ).unwrap();

        let filter = MarketplaceFilter {
            publisher_id: Some("seller".into()),
            ..Default::default()
        };
        let results = mp.search(&filter);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].publisher_id, "seller");
    }

    #[test]
    fn test_search_sort_by_price_asc() {
        let mut mp = make_marketplace();
        mp.publish_listing(
            "Expensive".into(), "desc".into(),
            KnowledgeCategory::General, "h1".into(),
            500, Currency::Token, "seller".into(), vec![], 1,
        ).unwrap();
        mp.publish_listing(
            "Cheap".into(), "desc".into(),
            KnowledgeCategory::General, "h2".into(),
            50, Currency::Token, "seller".into(), vec![], 2,
        ).unwrap();

        let filter = MarketplaceFilter {
            sort: Some(MarketplaceSort::PriceAsc),
            ..Default::default()
        };
        let results = mp.search(&filter);
        assert_eq!(results[0].price, 50);
        assert_eq!(results[1].price, 500);
    }

    #[test]
    fn test_search_sort_by_rating() {
        let mut mp = make_marketplace();
        mp.set_balance("buyer2", 5_000);

        let id1 = mp.publish_listing(
            "Good Knowledge".into(), "desc".into(),
            KnowledgeCategory::General, "h1".into(),
            100, Currency::Token, "seller".into(), vec![], 1,
        ).unwrap();
        let id2 = mp.publish_listing(
            "Bad Knowledge".into(), "desc".into(),
            KnowledgeCategory::General, "h2".into(),
            100, Currency::Token, "seller".into(), vec![], 2,
        ).unwrap();

        mp.purchase_listing(id1, "buyer", 5).unwrap();
        mp.rate_listing(id1, "buyer", 5, None, 10).unwrap();

        mp.purchase_listing(id2, "buyer2", 6).unwrap();
        mp.rate_listing(id2, "buyer2", 1, None, 11).unwrap();

        let filter = MarketplaceFilter {
            sort: Some(MarketplaceSort::RatingDesc),
            ..Default::default()
        };
        let results = mp.search(&filter);
        assert_eq!(results[0].title, "Good Knowledge");
        assert_eq!(results[1].title, "Bad Knowledge");
    }

    #[test]
    fn test_search_excludes_inactive_and_delisted() {
        let mut mp = make_marketplace();
        let id1 = mp.publish_listing(
            "Active".into(), "desc".into(),
            KnowledgeCategory::General, "h1".into(),
            100, Currency::Token, "seller".into(), vec![], 1,
        ).unwrap();
        let id2 = mp.publish_listing(
            "Inactive".into(), "desc".into(),
            KnowledgeCategory::General, "h2".into(),
            100, Currency::Token, "seller".into(), vec![], 2,
        ).unwrap();
        let id3 = mp.publish_listing(
            "Delisted".into(), "desc".into(),
            KnowledgeCategory::General, "h3".into(),
            100, Currency::Token, "seller".into(), vec![], 3,
        ).unwrap();

        mp.update_listing(id2, "seller", None, Some(ListingStatus::Inactive), None).unwrap();
        mp.delist_listing(id3, "seller").unwrap();

        let results = mp.search(&MarketplaceFilter::default());
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Active");
    }

    #[test]
    fn test_search_min_rating_filter() {
        let mut mp = make_marketplace();
        mp.set_balance("buyer2", 5_000);

        let id1 = mp.publish_listing(
            "Good".into(), "desc".into(),
            KnowledgeCategory::General, "h1".into(),
            100, Currency::Token, "seller".into(), vec![], 1,
        ).unwrap();
        let id2 = mp.publish_listing(
            "Bad".into(), "desc".into(),
            KnowledgeCategory::General, "h2".into(),
            100, Currency::Token, "seller".into(), vec![], 2,
        ).unwrap();

        mp.purchase_listing(id1, "buyer", 5).unwrap();
        mp.rate_listing(id1, "buyer", 5, None, 10).unwrap();

        mp.purchase_listing(id2, "buyer2", 6).unwrap();
        mp.rate_listing(id2, "buyer2", 2, None, 11).unwrap();

        let filter = MarketplaceFilter {
            min_rating: Some(4.0),
            ..Default::default()
        };
        let results = mp.search(&filter);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Good");
    }

    // ── Transfer ───────────────────────────────────────────

    #[test]
    fn test_transfer_tokens() {
        let mut mp = make_marketplace();
        mp.transfer("seller", "buyer", 500, Currency::Token).unwrap();
        assert_eq!(mp.get_balance("seller"), 9_500);
        assert_eq!(mp.get_balance("buyer"), 5_500);
    }

    #[test]
    fn test_transfer_insufficient_balance() {
        let mut mp = make_marketplace();
        let result = mp.transfer("buyer", "seller", 10_000, Currency::Token);
        assert!(matches!(result, Err(MarketplaceError::InsufficientBalance { .. })));
    }

    // ── Query helpers ──────────────────────────────────────

    #[test]
    fn test_list_active() {
        let mut mp = make_marketplace();
        let id1 = publish_default_listing(&mut mp);
        let id2 = mp.publish_listing(
            "Another".into(), "desc".into(),
            KnowledgeCategory::General, "h2".into(),
            50, Currency::Token, "seller".into(), vec![], 2,
        ).unwrap();
        mp.update_listing(id2, "seller", None, Some(ListingStatus::Inactive), None).unwrap();

        let active = mp.list_active();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].id, id1);
    }

    #[test]
    fn test_buyer_purchases() {
        let mut mp = make_marketplace();
        mp.set_balance("buyer2", 5_000);
        let id = publish_default_listing(&mut mp);

        mp.purchase_listing(id, "buyer", 5).unwrap();
        mp.purchase_listing(id, "buyer2", 6).unwrap();

        let buyer_purchases = mp.buyer_purchases("buyer");
        assert_eq!(buyer_purchases.len(), 1);
        assert_eq!(buyer_purchases[0].buyer_id, "buyer");
    }

    // ── Full lifecycle ─────────────────────────────────────

    #[test]
    fn test_full_lifecycle_publish_purchase_rate() {
        let mut mp = make_marketplace();

        // Publish
        let id = mp.publish_listing(
            "Advanced Strategy".into(),
            "Win every time.".into(),
            KnowledgeCategory::Strategy,
            "hash_xyz".into(),
            500,
            Currency::Token,
            "seller".into(),
            vec!["strategy".into(), "advanced".into()],
            1,
        ).unwrap();

        // Purchase
        let record = mp.purchase_listing(id, "buyer", 10).unwrap();
        assert_eq!(mp.get_balance("seller"), 10_500);
        assert_eq!(mp.get_balance("buyer"), 4_500);

        // Rate
        mp.rate_listing(id, "buyer", 5, Some("Excellent!".into()), 15).unwrap();
        let listing = mp.get(id).unwrap();
        assert_eq!(listing.purchase_count, 1);
        assert_eq!(listing.average_rating(), 5.0);

        // Search
        let filter = MarketplaceFilter {
            category: Some(KnowledgeCategory::Strategy),
            min_rating: Some(4.0),
            sort: Some(MarketplaceSort::RatingDesc),
            ..Default::default()
        };
        let results = mp.search(&filter);
        assert_eq!(results.len(), 1);
    }

    // ── Serialization ──────────────────────────────────────

    #[test]
    fn test_listing_serialization() {
        let listing = KnowledgeListing {
            id: Uuid::new_v4(),
            title: "Test".into(),
            description: "Desc".into(),
            category: KnowledgeCategory::Economy,
            content_hash: "hash".into(),
            price: 100,
            currency: Currency::Token,
            publisher_id: "seller".into(),
            status: ListingStatus::Active,
            purchase_count: 5,
            rating_sum: 20.0,
            rating_count: 5,
            tags: vec!["test".into()],
            created_tick: 1,
        };
        let json = serde_json::to_string(&listing).unwrap();
        let back: KnowledgeListing = serde_json::from_str(&json).unwrap();
        assert_eq!(listing.id, back.id);
        assert_eq!(listing.title, back.title);
        assert_eq!(listing.category, back.category);
        assert_eq!(listing.price, back.price);
        assert_eq!(listing.status, back.status);
        assert_eq!(listing.purchase_count, back.purchase_count);
    }

    #[test]
    fn test_purchase_record_serialization() {
        let record = PurchaseRecord {
            id: Uuid::new_v4(),
            listing_id: Uuid::new_v4(),
            buyer_id: "buyer".into(),
            seller_id: "seller".into(),
            price: 100,
            currency: Currency::Token,
            tick: 5,
        };
        let json = serde_json::to_string(&record).unwrap();
        let back: PurchaseRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(record.id, back.id);
        assert_eq!(record.buyer_id, back.buyer_id);
        assert_eq!(record.price, back.price);
    }

    #[test]
    fn test_rating_serialization() {
        let rating = Rating {
            id: Uuid::new_v4(),
            listing_id: Uuid::new_v4(),
            rater_id: "buyer".into(),
            score: 4,
            review: Some("Good".into()),
            tick: 10,
        };
        let json = serde_json::to_string(&rating).unwrap();
        let back: Rating = serde_json::from_str(&json).unwrap();
        assert_eq!(rating.score, back.score);
        assert_eq!(rating.review, back.review);
    }

    #[test]
    fn test_knowledge_category_serialization() {
        for cat in KnowledgeCategory::all() {
            let json = serde_json::to_string(&cat).unwrap();
            let back: KnowledgeCategory = serde_json::from_str(&json).unwrap();
            assert_eq!(cat, back);
        }
    }

    #[test]
    fn test_listing_status_serialization() {
        for status in [ListingStatus::Active, ListingStatus::Inactive, ListingStatus::Delisted] {
            let json = serde_json::to_string(&status).unwrap();
            let back: ListingStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(status, back);
        }
    }

    #[test]
    fn test_marketplace_sort_serialization() {
        for sort in [
            MarketplaceSort::Newest, MarketplaceSort::Oldest,
            MarketplaceSort::PriceAsc, MarketplaceSort::PriceDesc,
            MarketplaceSort::RatingDesc, MarketplaceSort::PurchasesDesc,
        ] {
            let json = serde_json::to_string(&sort).unwrap();
            let back: MarketplaceSort = serde_json::from_str(&json).unwrap();
            assert_eq!(sort, back);
        }
    }

    // ── Error display ──────────────────────────────────────

    #[test]
    fn test_error_display() {
        assert!(MarketplaceError::NotFound("test".into()).to_string().contains("test"));
        assert!(MarketplaceError::SelfPurchase.to_string().contains("own"));
        assert!(MarketplaceError::InvalidRating.to_string().contains("1 and 5"));
        assert!(MarketplaceError::InvalidPrice.to_string().contains("greater than 0"));
        assert!(MarketplaceError::Unauthorized("not allowed".into()).to_string().contains("unauthorized"));
    }

    // ── Event bus integration ──────────────────────────────

    #[test]
    fn test_event_bus_publish() {
        let bus = EventBus::new(64);
        let mut rx = bus.subscribe();
        let mut mp = Marketplace::with_event_bus(bus);
        mp.set_balance("seller", 1000);

        let id = mp.publish_listing(
            "Test".into(), "desc".into(),
            KnowledgeCategory::General, "h".into(),
            100, Currency::Token, "seller".into(), vec![], 1,
        ).unwrap();

        let event = rx.try_recv().unwrap();
        match event {
            WorldEvent::KnowledgeListed { listing_id, publisher, price, currency } => {
                assert_eq!(listing_id, id.to_string());
                assert_eq!(publisher, "seller");
                assert_eq!(price, 100);
                assert_eq!(currency, Currency::Token);
            }
            _ => panic!("expected KnowledgeListed event, got {:?}", event),
        }
    }

    #[test]
    fn test_event_bus_purchase() {
        let bus = EventBus::new(64);
        let mut rx = bus.subscribe();
        let mut mp = Marketplace::with_event_bus(bus);
        mp.set_balance("seller", 1000);
        mp.set_balance("buyer", 1000);

        let id = mp.publish_listing(
            "Test".into(), "desc".into(),
            KnowledgeCategory::General, "h".into(),
            100, Currency::Token, "seller".into(), vec![], 1,
        ).unwrap();
        let _ = rx.try_recv().unwrap(); // KnowledgeListed

        mp.purchase_listing(id, "buyer", 5).unwrap();

        let event = rx.try_recv().unwrap();
        match event {
            WorldEvent::KnowledgePurchased { listing_id, buyer, seller, price, currency } => {
                assert_eq!(listing_id, id.to_string());
                assert_eq!(buyer, "buyer");
                assert_eq!(seller, "seller");
                assert_eq!(price, 100);
                assert_eq!(currency, Currency::Token);
            }
            _ => panic!("expected KnowledgePurchased event, got {:?}", event),
        }
    }

    #[test]
    fn test_event_bus_rating() {
        let bus = EventBus::new(64);
        let mut rx = bus.subscribe();
        let mut mp = Marketplace::with_event_bus(bus);
        mp.set_balance("seller", 1000);
        mp.set_balance("buyer", 1000);

        let id = mp.publish_listing(
            "Test".into(), "desc".into(),
            KnowledgeCategory::General, "h".into(),
            100, Currency::Token, "seller".into(), vec![], 1,
        ).unwrap();
        let _ = rx.try_recv().unwrap(); // KnowledgeListed

        mp.purchase_listing(id, "buyer", 5).unwrap();
        let _ = rx.try_recv().unwrap(); // KnowledgePurchased

        mp.rate_listing(id, "buyer", 4, None, 10).unwrap();
        let event = rx.try_recv().unwrap();
        match event {
            WorldEvent::KnowledgeRated { listing_id, rater, score, average_rating } => {
                assert_eq!(listing_id, id.to_string());
                assert_eq!(rater, "buyer");
                assert_eq!(score, 4);
                assert!((average_rating - 4.0).abs() < f64::EPSILON);
            }
            _ => panic!("expected KnowledgeRated event, got {:?}", event),
        }
    }
}
