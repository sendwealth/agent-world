use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::world::enums::Currency;
use crate::world::event::WorldEvent;
use crate::world::state::EventBus;

// ── Tool Category ─────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolCategory {
    Computation,
    Communication,
    Analysis,
    Storage,
    Automation,
    Defense,
    Production,
    Utility,
}

impl ToolCategory {
    pub fn all() -> Vec<ToolCategory> {
        vec![
            ToolCategory::Computation,
            ToolCategory::Communication,
            ToolCategory::Analysis,
            ToolCategory::Storage,
            ToolCategory::Automation,
            ToolCategory::Defense,
            ToolCategory::Production,
            ToolCategory::Utility,
        ]
    }
}

// ── Tool Listing Mode ─────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolListingMode {
    /// Available for outright purchase.
    Sale,
    /// Available for time-limited rental.
    Rent,
    /// Available for both purchase and rental.
    Both,
}

// ── Tool Listing Status ───────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolListingStatus {
    Active,
    Inactive,
    Delisted,
}

// ── Tool Listing ──────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolListing {
    pub id: Uuid,
    pub name: String,
    pub description: String,
    pub category: ToolCategory,
    pub owner_id: String,
    /// Purchase price (0 = not for sale).
    pub purchase_price: u64,
    /// Rental price per tick (0 = not for rent).
    pub rental_price_per_tick: u64,
    pub currency: Currency,
    pub listing_mode: ToolListingMode,
    pub status: ToolListingStatus,
    /// Total times this tool has been purchased.
    pub total_purchases: u64,
    /// Total times this tool has been rented.
    pub total_rentals: u64,
    /// Sum of all ratings.
    pub rating_sum: f64,
    /// Number of ratings.
    pub rating_count: u64,
    /// Tags for search/filter.
    pub tags: Vec<String>,
    pub created_tick: u64,
}

impl ToolListing {
    pub fn average_rating(&self) -> f64 {
        if self.rating_count == 0 {
            0.0
        } else {
            self.rating_sum / self.rating_count as f64
        }
    }
}

// ── Rental Record ─────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RentalRecord {
    pub id: Uuid,
    pub tool_id: Uuid,
    pub renter_id: String,
    pub owner_id: String,
    pub price_per_tick: u64,
    pub currency: Currency,
    /// Tick when the rental starts.
    pub start_tick: u64,
    /// Tick when the rental expires.
    pub end_tick: u64,
    pub status: RentalStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RentalStatus {
    Active,
    Expired,
    Cancelled,
}

// ── Purchase Record ───────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolPurchaseRecord {
    pub id: Uuid,
    pub tool_id: Uuid,
    pub buyer_id: String,
    pub seller_id: String,
    pub price: u64,
    pub currency: Currency,
    pub tick: u64,
}

// ── Rating ────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolRating {
    pub id: Uuid,
    pub tool_id: Uuid,
    pub rater_id: String,
    pub score: u8,
    pub review: Option<String>,
    pub tick: u64,
}

// ── Errors ────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolMarketplaceError {
    NotFound(String),
    ListingInactive,
    ListingDelisted,
    InsufficientBalance { required: u64, available: u64 },
    SelfPurchase,
    SelfRent,
    NotForSale,
    NotForRent,
    InvalidPrice,
    InvalidRentalDuration,
    AlreadyOwned { tool_id: Uuid, agent_id: String },
    RentalNotFound(String),
    RentalNotActive,
    InvalidRating,
    AlreadyRated,
    NotEligibleToRate,
    Unauthorized(String),
}

impl std::fmt::Display for ToolMarketplaceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ToolMarketplaceError::NotFound(id) => write!(f, "tool listing not found: {}", id),
            ToolMarketplaceError::ListingInactive => write!(f, "tool listing is not active"),
            ToolMarketplaceError::ListingDelisted => write!(f, "tool listing has been delisted"),
            ToolMarketplaceError::InsufficientBalance {
                required,
                available,
            } => {
                write!(
                    f,
                    "insufficient balance: required {}, available {}",
                    required, available
                )
            }
            ToolMarketplaceError::SelfPurchase => write!(f, "cannot purchase your own tool"),
            ToolMarketplaceError::SelfRent => write!(f, "cannot rent your own tool"),
            ToolMarketplaceError::NotForSale => write!(f, "tool is not available for purchase"),
            ToolMarketplaceError::NotForRent => write!(f, "tool is not available for rental"),
            ToolMarketplaceError::InvalidPrice => write!(f, "price must be greater than 0"),
            ToolMarketplaceError::InvalidRentalDuration => {
                write!(f, "rental duration must be at least 1 tick")
            }
            ToolMarketplaceError::AlreadyOwned { tool_id, agent_id } => {
                write!(f, "agent {} already owns tool {}", agent_id, tool_id)
            }
            ToolMarketplaceError::RentalNotFound(id) => write!(f, "rental not found: {}", id),
            ToolMarketplaceError::RentalNotActive => write!(f, "rental is not active"),
            ToolMarketplaceError::InvalidRating => write!(f, "rating must be between 1 and 5"),
            ToolMarketplaceError::AlreadyRated => write!(f, "already rated this tool"),
            ToolMarketplaceError::NotEligibleToRate => {
                write!(f, "must purchase or rent before rating")
            }
            ToolMarketplaceError::Unauthorized(msg) => write!(f, "unauthorized: {}", msg),
        }
    }
}

impl std::error::Error for ToolMarketplaceError {}

// ── Search / Filter ───────────────────────────────────────

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ToolMarketplaceFilter {
    pub category: Option<ToolCategory>,
    pub owner_id: Option<String>,
    pub listing_mode: Option<ToolListingMode>,
    pub min_price: Option<u64>,
    pub max_price: Option<u64>,
    pub tag: Option<String>,
    pub query: Option<String>,
    pub min_rating: Option<f64>,
    pub sort: Option<ToolMarketplaceSort>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolMarketplaceSort {
    #[default]
    Newest,
    Oldest,
    PriceAsc,
    PriceDesc,
    RatingDesc,
    MostPurchased,
    MostRented,
}

// ── Tool Marketplace ──────────────────────────────────────

pub struct ToolMarketplace {
    listings: HashMap<Uuid, ToolListing>,
    purchases: Vec<ToolPurchaseRecord>,
    rentals: HashMap<Uuid, RentalRecord>,
    ratings: HashMap<Uuid, Vec<ToolRating>>,
    balances: HashMap<String, u64>,
    /// Track which agents own which tools (purchased).
    ownership_index: HashSet<(String, Uuid)>,
    /// Track which agents have rented which tools.
    rental_index: HashSet<(String, Uuid)>,
    /// Track which agents have purchased or rented for rating eligibility.
    interaction_index: HashSet<(String, Uuid)>,
    event_bus: Option<Arc<EventBus>>,
}

impl ToolMarketplace {
    pub fn new() -> Self {
        Self {
            listings: HashMap::new(),
            purchases: Vec::new(),
            rentals: HashMap::new(),
            ratings: HashMap::new(),
            balances: HashMap::new(),
            ownership_index: HashSet::new(),
            rental_index: HashSet::new(),
            interaction_index: HashSet::new(),
            event_bus: None,
        }
    }

    pub fn with_shared_event_bus(event_bus: Arc<EventBus>) -> Self {
        Self {
            listings: HashMap::new(),
            purchases: Vec::new(),
            rentals: HashMap::new(),
            ratings: HashMap::new(),
            balances: HashMap::new(),
            ownership_index: HashSet::new(),
            rental_index: HashSet::new(),
            interaction_index: HashSet::new(),
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

    pub fn get(&self, id: Uuid) -> Option<&ToolListing> {
        self.listings.get(&id)
    }

    pub fn list_active(&self) -> Vec<&ToolListing> {
        self.listings
            .values()
            .filter(|l| l.status == ToolListingStatus::Active)
            .collect()
    }

    pub fn list_all(&self) -> Vec<&ToolListing> {
        self.listings.values().collect()
    }

    /// Search/filter tool listings with optional sorting.
    pub fn search(&self, filter: &ToolMarketplaceFilter) -> Vec<&ToolListing> {
        let mut results: Vec<&ToolListing> = self
            .listings
            .values()
            .filter(|l| l.status == ToolListingStatus::Active)
            .filter(|l| filter.category.is_none_or(|c| l.category == c))
            .filter(|l| filter.owner_id.as_ref().is_none_or(|id| l.owner_id == *id))
            .filter(|l| filter.listing_mode.is_none_or(|m| l.listing_mode == m))
            .filter(|l| {
                filter
                    .min_price
                    .is_none_or(|min| l.purchase_price >= min || l.rental_price_per_tick >= min)
            })
            .filter(|l| {
                filter
                    .max_price
                    .is_none_or(|max| l.purchase_price <= max || l.rental_price_per_tick <= max)
            })
            .filter(|l| {
                filter
                    .tag
                    .as_ref()
                    .is_none_or(|tag| l.tags.iter().any(|t| t.eq_ignore_ascii_case(tag)))
            })
            .filter(|l| {
                filter
                    .query
                    .as_ref()
                    .is_none_or(|q| l.name.to_lowercase().contains(&q.to_lowercase()))
            })
            .filter(|l| {
                filter
                    .min_rating
                    .is_none_or(|min| l.average_rating() >= min)
            })
            .collect();

        let sort = filter.sort.unwrap_or_default();
        results.sort_by(|a, b| match sort {
            ToolMarketplaceSort::Newest => b.created_tick.cmp(&a.created_tick),
            ToolMarketplaceSort::Oldest => a.created_tick.cmp(&b.created_tick),
            ToolMarketplaceSort::PriceAsc => a.purchase_price.cmp(&b.purchase_price),
            ToolMarketplaceSort::PriceDesc => b.purchase_price.cmp(&a.purchase_price),
            ToolMarketplaceSort::RatingDesc => b
                .average_rating()
                .partial_cmp(&a.average_rating())
                .unwrap_or(std::cmp::Ordering::Equal),
            ToolMarketplaceSort::MostPurchased => b.total_purchases.cmp(&a.total_purchases),
            ToolMarketplaceSort::MostRented => b.total_rentals.cmp(&a.total_rentals),
        });

        results
    }

    /// Get rental by ID.
    pub fn get_rental(&self, id: Uuid) -> Option<&RentalRecord> {
        self.rentals.get(&id)
    }

    /// List active rentals for an agent.
    pub fn list_active_rentals(&self, renter_id: &str) -> Vec<&RentalRecord> {
        self.rentals
            .values()
            .filter(|r| r.renter_id == renter_id && r.status == RentalStatus::Active)
            .collect()
    }

    /// List all purchases for a tool.
    pub fn tool_purchases(&self, tool_id: Uuid) -> Vec<&ToolPurchaseRecord> {
        self.purchases
            .iter()
            .filter(|p| p.tool_id == tool_id)
            .collect()
    }

    /// List ratings for a tool.
    pub fn tool_ratings(&self, tool_id: Uuid) -> Vec<&ToolRating> {
        self.ratings
            .get(&tool_id)
            .map(|v| v.iter().collect())
            .unwrap_or_default()
    }

    /// Check if agent owns (purchased) a tool.
    pub fn owns_tool(&self, agent_id: &str, tool_id: Uuid) -> bool {
        self.ownership_index
            .contains(&(agent_id.to_string(), tool_id))
    }

    /// Check if agent has an active rental for a tool.
    pub fn has_active_rental(&self, agent_id: &str, tool_id: Uuid) -> bool {
        self.rental_index.contains(&(agent_id.to_string(), tool_id))
    }

    // ── CRUD ──────────────────────────────────────────────

    /// List a new tool on the marketplace.
    #[allow(clippy::too_many_arguments)]
    pub fn list_tool(
        &mut self,
        name: String,
        description: String,
        category: ToolCategory,
        owner_id: String,
        purchase_price: u64,
        rental_price_per_tick: u64,
        currency: Currency,
        listing_mode: ToolListingMode,
        tags: Vec<String>,
        created_tick: u64,
    ) -> Result<Uuid, ToolMarketplaceError> {
        if name.is_empty() {
            return Err(ToolMarketplaceError::NotFound("name is required".into()));
        }

        match listing_mode {
            ToolListingMode::Sale if purchase_price == 0 => {
                return Err(ToolMarketplaceError::InvalidPrice);
            }
            ToolListingMode::Rent if rental_price_per_tick == 0 => {
                return Err(ToolMarketplaceError::InvalidPrice);
            }
            ToolListingMode::Both if purchase_price == 0 && rental_price_per_tick == 0 => {
                return Err(ToolMarketplaceError::InvalidPrice);
            }
            _ => {}
        }

        let id = Uuid::new_v4();
        let listing = ToolListing {
            id,
            name,
            description,
            category,
            owner_id: owner_id.clone(),
            purchase_price,
            rental_price_per_tick,
            currency,
            listing_mode,
            status: ToolListingStatus::Active,
            total_purchases: 0,
            total_rentals: 0,
            rating_sum: 0.0,
            rating_count: 0,
            tags,
            created_tick,
        };
        self.listings.insert(id, listing);

        self.emit(WorldEvent::ToolListed {
            tool_id: id.to_string(),
            owner_id,
            purchase_price,
            rental_price_per_tick,
            currency,
        });

        Ok(id)
    }

    /// Update a tool listing.
    pub fn update_tool(
        &mut self,
        tool_id: Uuid,
        owner_id: &str,
        purchase_price: Option<u64>,
        rental_price_per_tick: Option<u64>,
        status: Option<ToolListingStatus>,
        tags: Option<Vec<String>>,
    ) -> Result<(), ToolMarketplaceError> {
        let listing = self
            .listings
            .get_mut(&tool_id)
            .ok_or_else(|| ToolMarketplaceError::NotFound(tool_id.to_string()))?;

        if listing.owner_id != owner_id {
            return Err(ToolMarketplaceError::Unauthorized(
                "only the owner can update".into(),
            ));
        }

        if let Some(p) = purchase_price {
            listing.purchase_price = p;
        }
        if let Some(r) = rental_price_per_tick {
            listing.rental_price_per_tick = r;
        }
        if let Some(s) = status {
            listing.status = s;
        }
        if let Some(t) = tags {
            listing.tags = t;
        }

        Ok(())
    }

    /// Delist a tool (permanent removal from marketplace).
    pub fn delist_tool(
        &mut self,
        tool_id: Uuid,
        owner_id: &str,
    ) -> Result<(), ToolMarketplaceError> {
        let listing = self
            .listings
            .get_mut(&tool_id)
            .ok_or_else(|| ToolMarketplaceError::NotFound(tool_id.to_string()))?;

        if listing.owner_id != owner_id {
            return Err(ToolMarketplaceError::Unauthorized(
                "only the owner can delist".into(),
            ));
        }

        listing.status = ToolListingStatus::Delisted;

        self.emit(WorldEvent::ToolDelisted {
            tool_id: tool_id.to_string(),
        });

        Ok(())
    }

    // ── Purchase ──────────────────────────────────────────

    /// Purchase a tool outright. Transfers funds and grants permanent ownership.
    pub fn purchase_tool(
        &mut self,
        tool_id: Uuid,
        buyer_id: &str,
        tick: u64,
    ) -> Result<ToolPurchaseRecord, ToolMarketplaceError> {
        let (price, currency, owner_id) = {
            let listing = self
                .listings
                .get(&tool_id)
                .ok_or_else(|| ToolMarketplaceError::NotFound(tool_id.to_string()))?;
            match listing.status {
                ToolListingStatus::Active => {}
                ToolListingStatus::Inactive => return Err(ToolMarketplaceError::ListingInactive),
                ToolListingStatus::Delisted => return Err(ToolMarketplaceError::ListingDelisted),
            }
            if listing.owner_id == buyer_id {
                return Err(ToolMarketplaceError::SelfPurchase);
            }
            if !matches!(
                listing.listing_mode,
                ToolListingMode::Sale | ToolListingMode::Both
            ) {
                return Err(ToolMarketplaceError::NotForSale);
            }
            if listing.purchase_price == 0 {
                return Err(ToolMarketplaceError::NotForSale);
            }
            (
                listing.purchase_price,
                listing.currency,
                listing.owner_id.clone(),
            )
        };

        // Check if already owned
        if self.owns_tool(buyer_id, tool_id) {
            return Err(ToolMarketplaceError::AlreadyOwned {
                tool_id,
                agent_id: buyer_id.to_string(),
            });
        }

        // Check balance
        let buyer_balance = self.get_balance(buyer_id);
        if buyer_balance < price {
            return Err(ToolMarketplaceError::InsufficientBalance {
                required: price,
                available: buyer_balance,
            });
        }

        // Transfer funds
        self.balances
            .insert(buyer_id.to_string(), buyer_balance - price);
        let seller_balance = self.get_balance(&owner_id);
        self.balances
            .insert(owner_id.clone(), seller_balance + price);

        // Update listing stats
        let listing = self.listings.get_mut(&tool_id).unwrap();
        listing.total_purchases += 1;

        // Record purchase
        let record = ToolPurchaseRecord {
            id: Uuid::new_v4(),
            tool_id,
            buyer_id: buyer_id.to_string(),
            seller_id: owner_id.clone(),
            price,
            currency,
            tick,
        };

        self.ownership_index.insert((buyer_id.to_string(), tool_id));
        self.interaction_index
            .insert((buyer_id.to_string(), tool_id));
        self.purchases.push(record.clone());

        self.emit(WorldEvent::ToolPurchased {
            tool_id: tool_id.to_string(),
            buyer_id: buyer_id.to_string(),
            seller_id: owner_id,
            price,
            currency,
        });

        Ok(record)
    }

    // ── Rental ────────────────────────────────────────────

    /// Rent a tool for a specified number of ticks.
    pub fn rent_tool(
        &mut self,
        tool_id: Uuid,
        renter_id: &str,
        duration_ticks: u64,
        current_tick: u64,
    ) -> Result<RentalRecord, ToolMarketplaceError> {
        if duration_ticks == 0 {
            return Err(ToolMarketplaceError::InvalidRentalDuration);
        }

        let (price_per_tick, currency, owner_id) = {
            let listing = self
                .listings
                .get(&tool_id)
                .ok_or_else(|| ToolMarketplaceError::NotFound(tool_id.to_string()))?;
            match listing.status {
                ToolListingStatus::Active => {}
                ToolListingStatus::Inactive => return Err(ToolMarketplaceError::ListingInactive),
                ToolListingStatus::Delisted => return Err(ToolMarketplaceError::ListingDelisted),
            }
            if listing.owner_id == renter_id {
                return Err(ToolMarketplaceError::SelfRent);
            }
            if !matches!(
                listing.listing_mode,
                ToolListingMode::Rent | ToolListingMode::Both
            ) {
                return Err(ToolMarketplaceError::NotForRent);
            }
            if listing.rental_price_per_tick == 0 {
                return Err(ToolMarketplaceError::NotForRent);
            }
            (
                listing.rental_price_per_tick,
                listing.currency,
                listing.owner_id.clone(),
            )
        };

        let total_cost = price_per_tick * duration_ticks;

        // Check balance
        let renter_balance = self.get_balance(renter_id);
        if renter_balance < total_cost {
            return Err(ToolMarketplaceError::InsufficientBalance {
                required: total_cost,
                available: renter_balance,
            });
        }

        // Transfer funds
        self.balances
            .insert(renter_id.to_string(), renter_balance - total_cost);
        let owner_balance = self.get_balance(&owner_id);
        self.balances
            .insert(owner_id.clone(), owner_balance + total_cost);

        // Update listing stats
        let listing = self.listings.get_mut(&tool_id).unwrap();
        listing.total_rentals += 1;

        // Create rental record
        let rental_id = Uuid::new_v4();
        let record = RentalRecord {
            id: rental_id,
            tool_id,
            renter_id: renter_id.to_string(),
            owner_id: owner_id.clone(),
            price_per_tick,
            currency,
            start_tick: current_tick,
            end_tick: current_tick + duration_ticks,
            status: RentalStatus::Active,
        };

        self.rental_index.insert((renter_id.to_string(), tool_id));
        self.interaction_index
            .insert((renter_id.to_string(), tool_id));
        self.rentals.insert(rental_id, record.clone());

        self.emit(WorldEvent::ToolRented {
            rental_id: rental_id.to_string(),
            tool_id: tool_id.to_string(),
            renter_id: renter_id.to_string(),
            owner_id,
            price_per_tick,
            duration_ticks,
            total_cost,
            currency,
        });

        Ok(record)
    }

    /// Cancel an active rental. No refund.
    pub fn cancel_rental(
        &mut self,
        rental_id: Uuid,
        renter_id: &str,
    ) -> Result<(), ToolMarketplaceError> {
        let rental = self
            .rentals
            .get_mut(&rental_id)
            .ok_or_else(|| ToolMarketplaceError::RentalNotFound(rental_id.to_string()))?;

        if rental.renter_id != renter_id {
            return Err(ToolMarketplaceError::Unauthorized(
                "only the renter can cancel".into(),
            ));
        }
        if rental.status != RentalStatus::Active {
            return Err(ToolMarketplaceError::RentalNotActive);
        }

        rental.status = RentalStatus::Cancelled;
        self.rental_index
            .remove(&(renter_id.to_string(), rental.tool_id));

        Ok(())
    }

    /// Process rental expirations. Expires all rentals whose end_tick <= current_tick.
    pub fn process_rental_expiry(&mut self, current_tick: u64) -> Vec<Uuid> {
        let expired: Vec<Uuid> = self
            .rentals
            .iter()
            .filter(|(_, r)| r.status == RentalStatus::Active && r.end_tick <= current_tick)
            .map(|(id, _)| *id)
            .collect();

        for id in &expired {
            if let Some(rental) = self.rentals.get_mut(id) {
                rental.status = RentalStatus::Expired;
                self.rental_index
                    .remove(&(rental.renter_id.clone(), rental.tool_id));
            }
        }

        expired
    }

    // ── Rating ────────────────────────────────────────────

    /// Rate a tool. The rater must have purchased or rented it.
    pub fn rate_tool(
        &mut self,
        tool_id: Uuid,
        rater_id: &str,
        score: u8,
        review: Option<String>,
        tick: u64,
    ) -> Result<Uuid, ToolMarketplaceError> {
        if !(1..=5).contains(&score) {
            return Err(ToolMarketplaceError::InvalidRating);
        }

        if !self
            .interaction_index
            .contains(&(rater_id.to_string(), tool_id))
        {
            return Err(ToolMarketplaceError::NotEligibleToRate);
        }

        // Check not already rated
        if self
            .ratings
            .get(&tool_id)
            .is_some_and(|rs| rs.iter().any(|r| r.rater_id == rater_id))
        {
            return Err(ToolMarketplaceError::AlreadyRated);
        }

        if !self.listings.contains_key(&tool_id) {
            return Err(ToolMarketplaceError::NotFound(tool_id.to_string()));
        }

        let rating_id = Uuid::new_v4();
        let rating = ToolRating {
            id: rating_id,
            tool_id,
            rater_id: rater_id.to_string(),
            score,
            review,
            tick,
        };

        let listing = self.listings.get_mut(&tool_id).unwrap();
        listing.rating_sum += score as f64;
        listing.rating_count += 1;

        self.ratings.entry(tool_id).or_default().push(rating);

        Ok(rating_id)
    }

    // ── Helpers ────────────────────────────────────────────

    fn emit(&self, event: WorldEvent) {
        if let Some(ref bus) = self.event_bus {
            bus.emit(event);
        }
    }
}

impl Default for ToolMarketplace {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ─────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_mp() -> ToolMarketplace {
        let mut mp = ToolMarketplace::new();
        mp.set_balance("owner", 10_000);
        mp.set_balance("buyer", 5_000);
        mp.set_balance("renter", 5_000);
        mp
    }

    fn list_sale_tool(mp: &mut ToolMarketplace) -> Uuid {
        mp.list_tool(
            "Computation Engine".into(),
            "A powerful computation tool.".into(),
            ToolCategory::Computation,
            "owner".into(),
            500,
            0,
            Currency::Token,
            ToolListingMode::Sale,
            vec!["compute".into()],
            1,
        )
        .unwrap()
    }

    fn list_rent_tool(mp: &mut ToolMarketplace) -> Uuid {
        mp.list_tool(
            "Storage Vault".into(),
            "Secure storage for your data.".into(),
            ToolCategory::Storage,
            "owner".into(),
            0,
            50,
            Currency::Token,
            ToolListingMode::Rent,
            vec!["storage".into()],
            1,
        )
        .unwrap()
    }

    fn list_both_tool(mp: &mut ToolMarketplace) -> Uuid {
        mp.list_tool(
            "Analysis Suite".into(),
            "Data analysis toolkit.".into(),
            ToolCategory::Analysis,
            "owner".into(),
            300,
            20,
            Currency::Token,
            ToolListingMode::Both,
            vec!["analysis".into()],
            1,
        )
        .unwrap()
    }

    // ── List Tool ──────────────────────────────────────────

    #[test]
    fn test_list_sale_tool() {
        let mut mp = make_mp();
        let id = list_sale_tool(&mut mp);
        let tool = mp.get(id).unwrap();
        assert_eq!(tool.name, "Computation Engine");
        assert_eq!(tool.purchase_price, 500);
        assert_eq!(tool.rental_price_per_tick, 0);
        assert_eq!(tool.listing_mode, ToolListingMode::Sale);
        assert_eq!(tool.status, ToolListingStatus::Active);
    }

    #[test]
    fn test_list_rent_tool() {
        let mut mp = make_mp();
        let id = list_rent_tool(&mut mp);
        let tool = mp.get(id).unwrap();
        assert_eq!(tool.rental_price_per_tick, 50);
        assert_eq!(tool.listing_mode, ToolListingMode::Rent);
    }

    #[test]
    fn test_list_both_tool() {
        let mut mp = make_mp();
        let id = list_both_tool(&mut mp);
        let tool = mp.get(id).unwrap();
        assert_eq!(tool.purchase_price, 300);
        assert_eq!(tool.rental_price_per_tick, 20);
        assert_eq!(tool.listing_mode, ToolListingMode::Both);
    }

    #[test]
    fn test_list_tool_empty_name_fails() {
        let mut mp = make_mp();
        let result = mp.list_tool(
            "".into(),
            "desc".into(),
            ToolCategory::Utility,
            "owner".into(),
            100,
            0,
            Currency::Token,
            ToolListingMode::Sale,
            vec![],
            1,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_list_sale_tool_zero_price_fails() {
        let mut mp = make_mp();
        let result = mp.list_tool(
            "Tool".into(),
            "desc".into(),
            ToolCategory::Utility,
            "owner".into(),
            0,
            0,
            Currency::Token,
            ToolListingMode::Sale,
            vec![],
            1,
        );
        assert!(matches!(result, Err(ToolMarketplaceError::InvalidPrice)));
    }

    #[test]
    fn test_list_rent_tool_zero_price_fails() {
        let mut mp = make_mp();
        let result = mp.list_tool(
            "Tool".into(),
            "desc".into(),
            ToolCategory::Utility,
            "owner".into(),
            0,
            0,
            Currency::Token,
            ToolListingMode::Rent,
            vec![],
            1,
        );
        assert!(matches!(result, Err(ToolMarketplaceError::InvalidPrice)));
    }

    // ── Purchase ───────────────────────────────────────────

    #[test]
    fn test_purchase_tool() {
        let mut mp = make_mp();
        let id = list_sale_tool(&mut mp);

        let record = mp.purchase_tool(id, "buyer", 5).unwrap();
        assert_eq!(record.price, 500);
        assert_eq!(record.buyer_id, "buyer");
        assert_eq!(record.seller_id, "owner");

        assert_eq!(mp.get_balance("buyer"), 4_500);
        assert_eq!(mp.get_balance("owner"), 10_500);
        assert_eq!(mp.get(id).unwrap().total_purchases, 1);
        assert!(mp.owns_tool("buyer", id));
    }

    #[test]
    fn test_purchase_self_fails() {
        let mut mp = make_mp();
        let id = list_sale_tool(&mut mp);
        let result = mp.purchase_tool(id, "owner", 5);
        assert!(matches!(result, Err(ToolMarketplaceError::SelfPurchase)));
    }

    #[test]
    fn test_purchase_insufficient_balance() {
        let mut mp = make_mp();
        let id = list_sale_tool(&mut mp);
        mp.set_balance("poor", 100);
        let result = mp.purchase_tool(id, "poor", 5);
        assert!(matches!(
            result,
            Err(ToolMarketplaceError::InsufficientBalance { .. })
        ));
    }

    #[test]
    fn test_purchase_rent_only_tool_fails() {
        let mut mp = make_mp();
        let id = list_rent_tool(&mut mp);
        let result = mp.purchase_tool(id, "buyer", 5);
        assert!(matches!(result, Err(ToolMarketplaceError::NotForSale)));
    }

    #[test]
    fn test_purchase_already_owned_fails() {
        let mut mp = make_mp();
        let id = list_sale_tool(&mut mp);
        mp.purchase_tool(id, "buyer", 5).unwrap();
        mp.set_balance("buyer", 10_000);
        let result = mp.purchase_tool(id, "buyer", 10);
        assert!(matches!(
            result,
            Err(ToolMarketplaceError::AlreadyOwned { .. })
        ));
    }

    // ── Rental ─────────────────────────────────────────────

    #[test]
    fn test_rent_tool() {
        let mut mp = make_mp();
        let id = list_rent_tool(&mut mp);

        let rental = mp.rent_tool(id, "renter", 10, 1).unwrap();
        assert_eq!(rental.price_per_tick, 50);
        assert_eq!(rental.start_tick, 1);
        assert_eq!(rental.end_tick, 11);
        assert_eq!(rental.status, RentalStatus::Active);

        // Total cost = 50 * 10 = 500
        assert_eq!(mp.get_balance("renter"), 4_500);
        assert_eq!(mp.get_balance("owner"), 10_500);
        assert_eq!(mp.get(id).unwrap().total_rentals, 1);
        assert!(mp.has_active_rental("renter", id));
    }

    #[test]
    fn test_rent_self_fails() {
        let mut mp = make_mp();
        let id = list_rent_tool(&mut mp);
        let result = mp.rent_tool(id, "owner", 10, 1);
        assert!(matches!(result, Err(ToolMarketplaceError::SelfRent)));
    }

    #[test]
    fn test_rent_sale_only_tool_fails() {
        let mut mp = make_mp();
        let id = list_sale_tool(&mut mp);
        let result = mp.rent_tool(id, "renter", 10, 1);
        assert!(matches!(result, Err(ToolMarketplaceError::NotForRent)));
    }

    #[test]
    fn test_rent_zero_duration_fails() {
        let mut mp = make_mp();
        let id = list_rent_tool(&mut mp);
        let result = mp.rent_tool(id, "renter", 0, 1);
        assert!(matches!(
            result,
            Err(ToolMarketplaceError::InvalidRentalDuration)
        ));
    }

    #[test]
    fn test_rent_insufficient_balance() {
        let mut mp = make_mp();
        let id = list_rent_tool(&mut mp);
        mp.set_balance("poor", 10);
        let result = mp.rent_tool(id, "poor", 100, 1);
        assert!(matches!(
            result,
            Err(ToolMarketplaceError::InsufficientBalance { .. })
        ));
    }

    #[test]
    fn test_rent_then_purchase_both_mode() {
        let mut mp = make_mp();
        let id = list_both_tool(&mut mp);

        // Rent first
        mp.rent_tool(id, "renter", 5, 1).unwrap();
        assert_eq!(mp.get_balance("renter"), 4_900); // 5000 - 100

        // Also can purchase
        mp.set_balance("renter", 10_000);
        let result = mp.purchase_tool(id, "renter", 10);
        assert!(result.is_ok());
    }

    // ── Cancel Rental ──────────────────────────────────────

    #[test]
    fn test_cancel_rental() {
        let mut mp = make_mp();
        let id = list_rent_tool(&mut mp);
        let rental = mp.rent_tool(id, "renter", 10, 1).unwrap();

        mp.cancel_rental(rental.id, "renter").unwrap();
        assert_eq!(
            mp.get_rental(rental.id).unwrap().status,
            RentalStatus::Cancelled
        );
        assert!(!mp.has_active_rental("renter", id));
    }

    // ── Rental Expiry ──────────────────────────────────────

    #[test]
    fn test_rental_expiry() {
        let mut mp = make_mp();
        let id = list_rent_tool(&mut mp);
        let rental = mp.rent_tool(id, "renter", 5, 1).unwrap();
        // Rental ends at tick 6

        let expired = mp.process_rental_expiry(6);
        assert_eq!(expired.len(), 1);
        assert_eq!(
            mp.get_rental(rental.id).unwrap().status,
            RentalStatus::Expired
        );
        assert!(!mp.has_active_rental("renter", id));
    }

    #[test]
    fn test_rental_not_yet_expired() {
        let mut mp = make_mp();
        let id = list_rent_tool(&mut mp);
        let rental = mp.rent_tool(id, "renter", 5, 1).unwrap();

        let expired = mp.process_rental_expiry(3);
        assert!(expired.is_empty());
        assert_eq!(
            mp.get_rental(rental.id).unwrap().status,
            RentalStatus::Active
        );
    }

    // ── Rating ─────────────────────────────────────────────

    #[test]
    fn test_rate_after_purchase() {
        let mut mp = make_mp();
        let id = list_sale_tool(&mut mp);
        mp.purchase_tool(id, "buyer", 5).unwrap();

        let rating_id = mp
            .rate_tool(id, "buyer", 5, Some("Excellent!".into()), 10)
            .unwrap();
        assert_ne!(rating_id, Uuid::nil());

        let tool = mp.get(id).unwrap();
        assert_eq!(tool.rating_count, 1);
        assert_eq!(tool.average_rating(), 5.0);
    }

    #[test]
    fn test_rate_after_rental() {
        let mut mp = make_mp();
        let id = list_rent_tool(&mut mp);
        mp.rent_tool(id, "renter", 5, 1).unwrap();

        let result = mp.rate_tool(id, "renter", 4, None, 10);
        assert!(result.is_ok());

        let tool = mp.get(id).unwrap();
        assert_eq!(tool.average_rating(), 4.0);
    }

    #[test]
    fn test_rate_without_interaction_fails() {
        let mut mp = make_mp();
        let id = list_sale_tool(&mut mp);
        let result = mp.rate_tool(id, "buyer", 5, None, 10);
        assert!(matches!(
            result,
            Err(ToolMarketplaceError::NotEligibleToRate)
        ));
    }

    #[test]
    fn test_rate_twice_fails() {
        let mut mp = make_mp();
        let id = list_sale_tool(&mut mp);
        mp.purchase_tool(id, "buyer", 5).unwrap();
        mp.rate_tool(id, "buyer", 4, None, 10).unwrap();
        let result = mp.rate_tool(id, "buyer", 5, None, 11);
        assert!(matches!(result, Err(ToolMarketplaceError::AlreadyRated)));
    }

    // ── Update / Delist ────────────────────────────────────

    #[test]
    fn test_update_tool_price() {
        let mut mp = make_mp();
        let id = list_sale_tool(&mut mp);
        mp.update_tool(id, "owner", Some(600), None, None, None)
            .unwrap();
        assert_eq!(mp.get(id).unwrap().purchase_price, 600);
    }

    #[test]
    fn test_update_tool_wrong_owner_fails() {
        let mut mp = make_mp();
        let id = list_sale_tool(&mut mp);
        let result = mp.update_tool(id, "imposter", Some(600), None, None, None);
        assert!(matches!(result, Err(ToolMarketplaceError::Unauthorized(_))));
    }

    #[test]
    fn test_delist_tool() {
        let mut mp = make_mp();
        let id = list_sale_tool(&mut mp);
        mp.delist_tool(id, "owner").unwrap();
        assert_eq!(mp.get(id).unwrap().status, ToolListingStatus::Delisted);
        assert!(mp.list_active().is_empty());
    }

    // ── Search ─────────────────────────────────────────────

    #[test]
    fn test_search_all_active() {
        let mut mp = make_mp();
        list_sale_tool(&mut mp);
        list_rent_tool(&mut mp);
        list_both_tool(&mut mp);
        let results = mp.search(&ToolMarketplaceFilter::default());
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn test_search_by_category() {
        let mut mp = make_mp();
        list_sale_tool(&mut mp); // Computation
        list_rent_tool(&mut mp); // Storage

        let filter = ToolMarketplaceFilter {
            category: Some(ToolCategory::Storage),
            ..Default::default()
        };
        let results = mp.search(&filter);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "Storage Vault");
    }

    #[test]
    fn test_search_by_listing_mode() {
        let mut mp = make_mp();
        list_sale_tool(&mut mp);
        list_both_tool(&mut mp);

        let filter = ToolMarketplaceFilter {
            listing_mode: Some(ToolListingMode::Both),
            ..Default::default()
        };
        let results = mp.search(&filter);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "Analysis Suite");
    }

    #[test]
    fn test_search_excludes_delisted() {
        let mut mp = make_mp();
        let id = list_sale_tool(&mut mp);
        list_rent_tool(&mut mp);
        mp.delist_tool(id, "owner").unwrap();

        let results = mp.search(&ToolMarketplaceFilter::default());
        assert_eq!(results.len(), 1);
    }

    // ── Serialization ──────────────────────────────────────

    #[test]
    fn test_tool_category_serialization() {
        for cat in ToolCategory::all() {
            let json = serde_json::to_string(&cat).unwrap();
            let back: ToolCategory = serde_json::from_str(&json).unwrap();
            assert_eq!(cat, back);
        }
    }

    #[test]
    fn test_tool_listing_mode_serialization() {
        for mode in [
            ToolListingMode::Sale,
            ToolListingMode::Rent,
            ToolListingMode::Both,
        ] {
            let json = serde_json::to_string(&mode).unwrap();
            let back: ToolListingMode = serde_json::from_str(&json).unwrap();
            assert_eq!(mode, back);
        }
    }

    #[test]
    fn test_rental_status_serialization() {
        for status in [
            RentalStatus::Active,
            RentalStatus::Expired,
            RentalStatus::Cancelled,
        ] {
            let json = serde_json::to_string(&status).unwrap();
            let back: RentalStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(status, back);
        }
    }

    // ── Error display ──────────────────────────────────────

    #[test]
    fn test_error_display() {
        assert!(ToolMarketplaceError::NotFound("t1".into())
            .to_string()
            .contains("t1"));
        assert!(ToolMarketplaceError::SelfPurchase
            .to_string()
            .contains("own tool"));
        assert!(ToolMarketplaceError::NotForSale
            .to_string()
            .contains("purchase"));
        assert!(ToolMarketplaceError::NotForRent
            .to_string()
            .contains("rental"));
        assert!(ToolMarketplaceError::InvalidRating
            .to_string()
            .contains("1 and 5"));
    }
}
