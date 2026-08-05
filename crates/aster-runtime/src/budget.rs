use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Fixed ASTER 0.1 resource dimensions.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BudgetDimension {
    ModelCalls,
    ModelTokens,
    ExternalReads,
    ExternalWrites,
    Approvals,
    MoneyMicrounits,
}

/// One outstanding deterministic maximum reservation.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Reservation {
    id: u64,
    dimension: BudgetDimension,
    maximum: u64,
}

/// Remaining and reserved resource ledger.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Budget {
    remaining: BTreeMap<BudgetDimension, u64>,
    reservations: BTreeMap<u64, Reservation>,
    next_id: u64,
}

impl Budget {
    /// Creates a budget; omitted dimensions have zero capacity.
    #[must_use]
    pub fn new(limits: BTreeMap<BudgetDimension, u64>) -> Self {
        Self {
            remaining: limits,
            reservations: BTreeMap::new(),
            next_id: 0,
        }
    }

    /// Reserves the maximum resource use before an external effect.
    ///
    /// # Errors
    ///
    /// Returns exhaustion without mutating the ledger when capacity is absent.
    pub fn reserve(
        &mut self,
        dimension: BudgetDimension,
        maximum: u64,
    ) -> Result<Reservation, BudgetError> {
        let remaining = self.remaining.get(&dimension).copied().unwrap_or(0);
        let Some(after) = remaining.checked_sub(maximum) else {
            return Err(BudgetError::Exhausted(dimension));
        };
        let reservation = Reservation {
            id: self.next_id,
            dimension,
            maximum,
        };
        self.next_id = self.next_id.saturating_add(1);
        self.remaining.insert(dimension, after);
        self.reservations.insert(reservation.id, reservation);
        Ok(reservation)
    }

    /// Settles actual use and releases unused capacity.
    ///
    /// # Errors
    ///
    /// Rejects unknown reservations and usage over the declared maximum.
    pub fn settle(&mut self, reservation: Reservation, actual: u64) -> Result<(), BudgetError> {
        if actual > reservation.maximum {
            return Err(BudgetError::ActualExceedsReservation);
        }
        if self.reservations.remove(&reservation.id) != Some(reservation) {
            return Err(BudgetError::UnknownReservation);
        }
        let released = reservation.maximum - actual;
        let remaining = self.remaining.entry(reservation.dimension).or_default();
        *remaining = remaining.saturating_add(released);
        Ok(())
    }

    /// Returns remaining capacity for one dimension.
    #[must_use]
    pub fn remaining(&self, dimension: BudgetDimension) -> u64 {
        self.remaining.get(&dimension).copied().unwrap_or(0)
    }
}

/// Deterministic budget admission or settlement failure.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum BudgetError {
    /// Maximum reservation exceeds remaining capacity.
    #[error("budget exhausted for {0:?}")]
    Exhausted(BudgetDimension),
    /// Driver reported usage above its declared maximum.
    #[error("actual usage exceeds reservation")]
    ActualExceedsReservation,
    /// Reservation is stale, forged, or already settled.
    #[error("unknown budget reservation")]
    UnknownReservation,
}
