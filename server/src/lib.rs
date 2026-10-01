//! REST API for baccarat-ev: exact odds of every bet (`POST /analyze`), as JSON. Shoe
//! simulation is for internal use and stays in the command-line tool, not the API.
//! The request and response shapes live in this crate, so the
//! core library stays free of outside crates. `openapi.yaml` describes the API.

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use axum::extract::rejection::JsonRejection;
use axum::extract::{DefaultBodyLimit, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use baccarat_ev::{Bet, Odds, Payouts, RebateBasis, Shoe, SuperTie, analyze};
use serde::{Deserialize, Serialize};

/// Largest request body accepted, in bytes.
pub const MAX_BODY_BYTES: usize = 64 * 1024;

/// Most decks a request can ask for: the exact analysis counts shoes of up to 31 decks.
pub const MAX_DECKS: u32 = 31;

/// Shoes whose odds are kept. When the cache is full it is emptied and starts again.
const CACHE_CAPACITY: usize = 10_000;

const OPENAPI: &str = include_str!("../openapi.yaml");

/// Shared by every request: the server's payouts and the odds of shoes already analyzed.
#[derive(Clone)]
pub struct AppState {
    payouts: Payouts,
    odds_cache: Arc<Mutex<HashMap<Shoe, Odds>>>,
}

impl AppState {
    /// `payouts` apply to every request that doesn't change them.
    pub fn new(payouts: Payouts) -> Self {
        Self { payouts, odds_cache: Arc::default() }
    }

    /// Number of shoes whose odds are cached.
    pub fn cached_shoes(&self) -> usize {
        self.cache().len()
    }

    fn cache(&self) -> MutexGuard<'_, HashMap<Shoe, Odds>> {
        self.odds_cache.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Odds of `shoe`, from the cache or worked out on a blocking thread (about 1 ms).
    async fn odds(&self, shoe: Shoe) -> Result<Odds, ApiError> {
        let cached = self.cache().get(&shoe).copied();
        if let Some(odds) = cached {
            return Ok(odds);
        }
        let odds = tokio::task::spawn_blocking(move || analyze(&shoe))
            .await
            .map_err(|error| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, format!("analysis failed: {error}")))?
            .map_err(ApiError::bad_request)?;
        let mut cache = self.cache();
        if cache.len() >= CACHE_CAPACITY {
            cache.clear();
        }
        cache.insert(shoe, odds);
        Ok(odds)
    }
}

/// The API's routes.
pub fn app(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/openapi.yaml", get(openapi))
        .route("/analyze", post(analyze_shoe))
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
        .with_state(state)
}

/// An error answer: a status code and `{ "error": "..." }`.
#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self { status, message: message.into() }
    }

    fn bad_request(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, message)
    }
}

impl From<JsonRejection> for ApiError {
    fn from(rejection: JsonRejection) -> Self {
        // Keep 413 (too large) and 415 (not JSON); any other problem with the body is a 400.
        let status = rejection.status();
        let status = if status == StatusCode::PAYLOAD_TOO_LARGE || status == StatusCode::UNSUPPORTED_MEDIA_TYPE {
            status
        } else {
            StatusCode::BAD_REQUEST
        };
        Self::new(status, rejection.body_text())
    }
}

#[derive(Serialize)]
struct ErrorBody {
    error: String,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, Json(ErrorBody { error: self.message })).into_response()
    }
}

fn check_decks(decks: u32) -> Result<(), ApiError> {
    if (1..=MAX_DECKS).contains(&decks) {
        Ok(())
    } else {
        Err(ApiError::bad_request(format!("decks must be from 1 to {MAX_DECKS}, found {decks}")))
    }
}

fn default_decks() -> u32 {
    8
}

// ---- Payouts -------------------------------------------------------------------------------

/// Payouts a request changes; anything left out keeps the server's setting.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct PayoutsRequest {
    banker_commission: Option<f64>,
    tie: Option<f64>,
    big: Option<f64>,
    small: Option<f64>,
    super_tie: Option<[f64; 3]>,
    pair: Option<f64>,
    super_six: Option<f64>,
    rebate: Option<RebateRequest>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RebateRequest {
    on: Option<RebateOn>,
    /// Rate for every bet.
    rate: Option<f64>,
    /// Rates for single bets, keyed like "banker" or "super-6", overriding `rate`.
    #[serde(default)]
    rates: BTreeMap<String, f64>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
enum RebateOn {
    All,
    Losses,
    Decided,
}

impl From<RebateOn> for RebateBasis {
    fn from(on: RebateOn) -> Self {
        match on {
            RebateOn::All => RebateBasis::AllBets,
            RebateOn::Losses => RebateBasis::Losses,
            RebateOn::Decided => RebateBasis::DecidedBets,
        }
    }
}

impl From<RebateBasis> for RebateOn {
    fn from(basis: RebateBasis) -> Self {
        match basis {
            RebateBasis::AllBets => RebateOn::All,
            RebateBasis::Losses => RebateOn::Losses,
            RebateBasis::DecidedBets => RebateOn::Decided,
        }
    }
}

impl PayoutsRequest {
    /// The server's payouts with this request's changes, checked.
    fn apply_to(self, server: &Payouts) -> Result<Payouts, ApiError> {
        let mut payouts = *server;
        payouts.banker_commission = self.banker_commission.unwrap_or(payouts.banker_commission);
        payouts.tie = self.tie.unwrap_or(payouts.tie);
        payouts.big = self.big.unwrap_or(payouts.big);
        payouts.small = self.small.unwrap_or(payouts.small);
        payouts.super_tie = self.super_tie.unwrap_or(payouts.super_tie);
        payouts.pair = self.pair.unwrap_or(payouts.pair);
        payouts.super_six = self.super_six.unwrap_or(payouts.super_six);
        if let Some(rebate) = self.rebate {
            if let Some(on) = rebate.on {
                payouts.rebate.basis = on.into();
            }
            // Giving any rate replaces the server's rates, like the --rebate option does.
            if rebate.rate.is_some() || !rebate.rates.is_empty() {
                payouts.rebate.rates = [rebate.rate.unwrap_or(0.0); Bet::ALL.len()];
                for (key, rate) in rebate.rates {
                    let bet = Bet::from_key(&key).ok_or_else(|| ApiError::bad_request(format!("unknown bet {key:?} in rebate.rates")))?;
                    payouts.rebate.rates[bet as usize] = rate;
                }
            }
        }
        payouts.validate().map_err(ApiError::bad_request)?;
        Ok(payouts)
    }
}

/// The payouts a request was settled with.
#[derive(Debug, Serialize)]
struct PayoutsUsed {
    banker_commission: f64,
    tie: f64,
    big: f64,
    small: f64,
    super_tie: [f64; 3],
    pair: f64,
    super_six: f64,
    rebate: RebateUsed,
}

#[derive(Debug, Serialize)]
struct RebateUsed {
    on: RebateOn,
    /// Every bet's rate, keyed like "banker".
    rates: BTreeMap<String, f64>,
}

impl From<&Payouts> for PayoutsUsed {
    fn from(payouts: &Payouts) -> Self {
        Self {
            banker_commission: payouts.banker_commission,
            tie: payouts.tie,
            big: payouts.big,
            small: payouts.small,
            super_tie: payouts.super_tie,
            pair: payouts.pair,
            super_six: payouts.super_six,
            rebate: RebateUsed {
                on: payouts.rebate.basis.into(),
                rates: Bet::ALL.into_iter().map(|bet| (bet.key(), payouts.rebate.rates[bet as usize])).collect(),
            },
        }
    }
}

// ---- GET /health, GET /openapi.yaml --------------------------------------------------------

#[derive(Serialize)]
struct Health {
    status: &'static str,
}

async fn health() -> Json<Health> {
    Json(Health { status: "ok" })
}

async fn openapi() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "application/yaml")], OPENAPI)
}

// ---- POST /analyze -------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AnalyzeRequest {
    #[serde(default = "default_decks")]
    decks: u32,
    /// Cards already dealt, by rank: "A", "2"-"9", "10" or "T", "J", "Q", "K".
    #[serde(default)]
    dealt: Vec<String>,
    #[serde(default)]
    payouts: PayoutsRequest,
}

#[derive(Serialize)]
struct AnalyzeResponse {
    shoe: ShoeSummary,
    payouts: PayoutsUsed,
    bets: Vec<BetOdds>,
    super_tie: SuperTieChances,
}

#[derive(Serialize)]
struct ShoeSummary {
    decks: u32,
    dealt: usize,
    cards_left: u32,
}

#[derive(Serialize)]
struct BetOdds {
    bet: String,
    name: &'static str,
    win: f64,
    lose: f64,
    push: f64,
    ev: f64,
    rebate: f64,
    ev_with_rebate: f64,
    /// Only for bets that can push (Banker, Player).
    win_rate_excluding_ties: Option<f64>,
}

#[derive(Serialize)]
struct SuperTieChances {
    same_color: f64,
    mixed_color: f64,
    other: f64,
}

async fn analyze_shoe(
    State(state): State<AppState>,
    request: Result<Json<AnalyzeRequest>, JsonRejection>,
) -> Result<Json<AnalyzeResponse>, ApiError> {
    let Json(request) = request?;
    check_decks(request.decks)?;
    let payouts = request.payouts.apply_to(&state.payouts)?;
    let mut shoe = Shoe::new(request.decks);
    for card in &request.dealt {
        shoe.remove(card).map_err(ApiError::bad_request)?;
    }
    let odds = state.odds(shoe).await?;

    let bets = Bet::ALL
        .into_iter()
        .map(|bet| {
            let stats = odds.bet_stats(bet, &payouts);
            let decided = stats.win + stats.lose;
            BetOdds {
                bet: bet.key(),
                name: bet.name(),
                win: stats.win,
                lose: stats.lose,
                push: stats.push,
                ev: stats.ev,
                rebate: stats.rebate,
                ev_with_rebate: stats.ev_with_rebate(),
                win_rate_excluding_ties: (stats.push > 0.0 && decided > 0.0).then(|| stats.win / decided),
            }
        })
        .collect();
    let results = odds.results();
    let chance = |category| results.iter().filter(|(result, _)| result.super_tie == Some(category)).map(|(_, probability)| probability).sum();
    Ok(Json(AnalyzeResponse {
        shoe: ShoeSummary { decks: request.decks, dealt: request.dealt.len(), cards_left: shoe.cards_left() },
        payouts: (&payouts).into(),
        bets,
        super_tie: SuperTieChances {
            same_color: chance(SuperTie::SameColorSixCard),
            mixed_color: chance(SuperTie::MixedColorSixCard),
            other: chance(SuperTie::OtherTie),
        },
    }))
}
