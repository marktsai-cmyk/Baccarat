use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use baccarat_ev::{Bet, Payouts};
use baccarat_server::{AppState, MAX_BODY_BYTES, app};
use serde_json::{Value, json};
use tower::ServiceExt;

async fn send(state: &AppState, request: Request<Body>) -> (StatusCode, String) {
    let response = app(state.clone()).oneshot(request).await.unwrap();
    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    (status, String::from_utf8(body.to_vec()).unwrap())
}

async fn post(state: &AppState, path: &str, body: impl Into<String>) -> (StatusCode, Value) {
    let request = Request::post(path).header(header::CONTENT_TYPE, "application/json").body(Body::from(body.into())).unwrap();
    let (status, body) = send(state, request).await;
    (status, serde_json::from_str(&body).unwrap_or_else(|_| panic!("not JSON: {body}")))
}

fn bet<'a>(answer: &'a Value, key: &str) -> &'a Value {
    answer["bets"].as_array().unwrap().iter().find(|bet| bet["bet"] == key).unwrap()
}

fn assert_close(actual: &Value, expected: f64, tolerance: f64) {
    let actual = actual.as_f64().unwrap_or_else(|| panic!("not a number: {actual}"));
    assert!((actual - expected).abs() <= tolerance, "{actual} is not within {tolerance} of {expected}");
}

fn default_state() -> AppState {
    AppState::new(Payouts::default())
}

#[tokio::test]
async fn health_says_ok() {
    let (status, body) = send(&default_state(), Request::get("/health").body(Body::empty()).unwrap()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(serde_json::from_str::<Value>(&body).unwrap(), json!({ "status": "ok" }));
}

#[tokio::test]
async fn serves_the_openapi_description() {
    let response = app(default_state()).oneshot(Request::get("/openapi.yaml").body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::CONTENT_TYPE], "application/yaml");
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let text = String::from_utf8(body.to_vec()).unwrap();
    for path in ["/health:", "/openapi.yaml:", "/analyze:"] {
        assert!(text.contains(path), "openapi.yaml has no {path}");
    }
    assert!(!text.contains("/simulate"), "simulation is internal, so the API description must not offer it");
    for bet in Bet::ALL {
        assert!(text.contains(&bet.key()), "openapi.yaml doesn't list the bet {}", bet.key());
    }
}

#[tokio::test]
async fn analyzes_a_fresh_shoe_with_the_server_payouts() {
    let (status, answer) = post(&default_state(), "/analyze", "{}").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(answer["shoe"], json!({ "decks": 8, "dealt": 0, "cards_left": 416 }));
    assert_eq!(answer["payouts"]["tie"], 8.0);
    assert_eq!(answer["bets"].as_array().unwrap().len(), Bet::ALL.len());

    let banker = bet(&answer, "banker");
    assert_close(&banker["win"], 0.458597, 5e-7);
    assert_close(&banker["ev"], -0.010579, 5e-7);
    assert_close(&banker["win_rate_excluding_ties"], 0.506825, 5e-7);
    assert_eq!(bet(&answer, "tie")["win_rate_excluding_ties"], Value::Null);
    assert_close(&bet(&answer, "player-pair")["win"], 31.0 / 415.0, 1e-12);
    assert_close(&bet(&answer, "super-6")["ev"], -0.2998, 5e-5);
    assert_close(&answer["super_tie"]["same_color"], 0.00026543, 5e-9);
}

#[tokio::test]
async fn request_payouts_change_the_server_payouts() {
    let server = AppState::new(Payouts { tie: 9.0, ..Payouts::default() });
    let body = json!({ "payouts": { "super_six": 15, "rebate": { "on": "decided", "rates": { "banker": 0.008 } } } });
    let (status, answer) = post(&server, "/analyze", body.to_string()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(answer["payouts"]["tie"], 9.0, "the server's payout is kept");
    assert_eq!(answer["payouts"]["rebate"]["on"], "decided");
    assert_close(&bet(&answer, "tie")["ev"], -0.04844, 5e-6);
    assert_close(&bet(&answer, "super-6")["ev"], -0.138181, 5e-6);
    assert_close(&bet(&answer, "banker")["ev_with_rebate"], -0.003340, 5e-7);
    assert_eq!(bet(&answer, "player")["rebate"], 0.0);
}

#[tokio::test]
async fn caches_the_odds_of_each_shoe() {
    let state = default_state();
    let dealt = json!({ "dealt": ["5", "5", "5", "5", "6", "6", "7", "4", "4", "5", "6", "5", "6", "7", "5"] }).to_string();
    let (status, answer) = post(&state, "/analyze", dealt.clone()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(answer["shoe"]["cards_left"], 401);
    assert_close(&bet(&answer, "player")["ev"], -0.011332, 5e-7);
    assert_eq!(state.cached_shoes(), 1);

    post(&state, "/analyze", dealt).await;
    assert_eq!(state.cached_shoes(), 1, "the same shoe again comes from the cache");
    post(&state, "/analyze", json!({ "payouts": { "tie": 9 } }).to_string()).await;
    post(&state, "/analyze", "{}").await;
    assert_eq!(state.cached_shoes(), 2, "payouts don't change which shoe is cached");
}

#[tokio::test]
async fn does_not_offer_simulation() {
    // Simulation is for internal use; it stays in the command-line tool.
    let request = Request::post("/simulate").header(header::CONTENT_TYPE, "application/json").body(Body::from("{}")).unwrap();
    let (status, _) = send(&default_state(), request).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn rejects_bad_requests_with_a_json_error() {
    let cases = [
        ("/analyze", r#"{ "dealt": ["X"] }"#, "unknown card \"X\""),
        ("/analyze", r#"{ "decks": 40 }"#, "decks must be from 1 to 31, found 40"),
        ("/analyze", r#"{ "deck": 8 }"#, "unknown field `deck`"),
        ("/analyze", r#"{ "decks": 8"#, "EOF while parsing"),
        ("/analyze", r#"{ "payouts": { "banker_commission": 2 } }"#, "banker_commission must be from 0 to under 1"),
        ("/analyze", r#"{ "payouts": { "super_tie": [100, 25] } }"#, "invalid length 2"),
        ("/analyze", r#"{ "payouts": { "rebate": { "rates": { "bank": 0.01 } } } }"#, "unknown bet \"bank\" in rebate.rates"),
        ("/analyze", r#"{ "payouts": { "rebate": { "on": "sometimes" } } }"#, "unknown variant `sometimes`"),
        ("/analyze", r#"{ "decks": 0 }"#, "decks must be from 1 to 31, found 0"),
        ("/analyze", r#"{ "seed": 2026 }"#, "unknown field `seed`"),
    ];
    for (path, body, expected) in cases {
        let (status, answer) = post(&default_state(), path, body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{path} {body}");
        let error = answer["error"].as_str().unwrap();
        assert!(error.contains(expected), "{path} {body}: {error:?} doesn't mention {expected:?}");
    }
}

#[tokio::test]
async fn rejects_bodies_that_are_not_json_or_too_large() {
    let not_json = Request::post("/analyze").body(Body::from("{}")).unwrap();
    let (status, body) = send(&default_state(), not_json).await;
    assert_eq!(status, StatusCode::UNSUPPORTED_MEDIA_TYPE);
    assert!(serde_json::from_str::<Value>(&body).unwrap()["error"].is_string());

    let huge = format!(r#"{{ "dealt": [{}"K"] }}"#, r#""K", "#.repeat(MAX_BODY_BYTES / 4));
    let (status, answer) = post(&default_state(), "/analyze", huge).await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
    assert!(answer["error"].is_string());
}
