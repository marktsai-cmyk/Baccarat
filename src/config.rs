//! Reads payouts and the rebate from a small TOML file (see `baccarat.toml`).
//!
//! Only the part of TOML the file needs is understood: `key = value` lines, `#` comments,
//! one `[rebate]` section, and values that are numbers, "strings" or [lists, of, numbers].

use std::collections::HashSet;
use std::path::Path;

use crate::bets::{Bet, Payouts, RebateBasis};

/// File read when no `--config` is given, if it exists.
pub const DEFAULT_CONFIG: &str = "baccarat.toml";

enum Value {
    Number(f64),
    Text(String),
    Numbers(Vec<f64>),
}

pub fn load_payouts(path: &Path) -> Result<Payouts, String> {
    let text = std::fs::read_to_string(path).map_err(|error| format!("can't read {}: {error}", path.display()))?;
    parse_payouts(&text).map_err(|error| format!("{}: {error}", path.display()))
}

/// Payouts from the file's text; anything it leaves out keeps its default.
pub fn parse_payouts(text: &str) -> Result<Payouts, String> {
    let mut payouts = Payouts::default();
    let mut rebate_rate = 0.0;
    let mut rebate_by_bet: Vec<(Bet, f64)> = Vec::new();
    let mut section = String::new();
    let mut seen = HashSet::new();

    for (line_number, line) in (1..).zip(text.lines()) {
        let at_line = |message: String| format!("line {line_number}: {message}");
        let line = line.split('#').next().unwrap_or_default().trim();
        if line.is_empty() {
            continue;
        }
        if let Some(name) = line.strip_prefix('[').and_then(|rest| rest.strip_suffix(']')) {
            section = name.trim().to_string();
            if section != "rebate" {
                return Err(at_line(format!("unknown section [{section}]")));
            }
            continue;
        }
        let (key, raw) = line.split_once('=').ok_or_else(|| at_line(format!("expected `key = value`, found {line:?}")))?;
        let (key, raw) = (key.trim(), raw.trim());
        if !seen.insert(format!("{section}.{key}")) {
            return Err(at_line(format!("{key} is set twice")));
        }
        let value = parse_value(raw).ok_or_else(|| at_line(format!("invalid value {raw:?} for {key}")))?;

        let rate = || match value {
            Value::Number(rate) if (0.0..1.0).contains(&rate) => Ok(rate),
            _ => Err(at_line(format!("{key} must be a number from 0 to under 1, found {raw}"))),
        };
        let payout = || match value {
            Value::Number(payout) if payout >= 0.0 => Ok(payout),
            _ => Err(at_line(format!("{key} must be a number from 0 up, found {raw}"))),
        };
        match (section.as_str(), key) {
            ("", "banker_commission") => payouts.banker_commission = rate()?,
            ("", "tie") => payouts.tie = payout()?,
            ("", "big") => payouts.big = payout()?,
            ("", "small") => payouts.small = payout()?,
            ("", "pair") => payouts.pair = payout()?,
            ("", "super_six") => payouts.super_six = payout()?,
            ("", "super_tie") => {
                payouts.super_tie = match &value {
                    Value::Numbers(pays) if pays.iter().all(|&pay| pay >= 0.0) => pays.as_slice().try_into().ok(),
                    _ => None,
                }
                .ok_or_else(|| at_line(format!("super_tie must be 3 payouts, e.g. [100, 25, 7], found {raw}")))?;
            }
            ("rebate", "on") => {
                payouts.rebate.basis = match &value {
                    Value::Text(basis) if basis == "all" => RebateBasis::AllBets,
                    Value::Text(basis) if basis == "losses" => RebateBasis::Losses,
                    Value::Text(basis) if basis == "decided" => RebateBasis::DecidedBets,
                    _ => return Err(at_line(format!("on must be \"all\", \"losses\" or \"decided\", found {raw}"))),
                };
            }
            ("rebate", "rate") => rebate_rate = rate()?,
            ("rebate", bet_key) => {
                let bet = Bet::from_key(bet_key).ok_or_else(|| at_line(format!("unknown rebate setting {bet_key:?}")))?;
                rebate_by_bet.push((bet, rate()?));
            }
            _ => return Err(at_line(format!("unknown setting {key:?}"))),
        }
    }

    payouts.rebate.rates = [rebate_rate; Bet::ALL.len()];
    for (bet, rate) in rebate_by_bet {
        payouts.rebate.rates[bet as usize] = rate;
    }
    Ok(payouts)
}

fn parse_value(raw: &str) -> Option<Value> {
    let finite = |text: &str| text.trim().parse::<f64>().ok().filter(|number| number.is_finite());
    if let Some(text) = raw.strip_prefix('"').and_then(|rest| rest.strip_suffix('"')) {
        return Some(Value::Text(text.to_string()));
    }
    if let Some(items) = raw.strip_prefix('[').and_then(|rest| rest.strip_suffix(']')) {
        return items.split(',').filter(|item| !item.trim().is_empty()).map(finite).collect::<Option<_>>().map(Value::Numbers);
    }
    finite(raw).map(Value::Number)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shipped_file_holds_the_defaults() {
        assert_eq!(parse_payouts(include_str!("../baccarat.toml")), Ok(Payouts::default()));
    }

    #[test]
    fn reads_payouts_and_rebate() {
        let text = "
            tie = 9            # a 9:1 table
            super_tie = [100, 20, 7]
            super_six = 15

            [rebate]
            on = \"decided\"
            rate = 0.008
            super-6 = 0
        ";
        let payouts = parse_payouts(text).unwrap();
        assert_eq!((payouts.tie, payouts.super_tie, payouts.super_six), (9.0, [100.0, 20.0, 7.0], 15.0));
        assert_eq!((payouts.big, payouts.pair), (0.54, 11.0));
        assert_eq!(payouts.rebate.basis, RebateBasis::DecidedBets);
        assert_eq!(payouts.rebate.rates, [0.008, 0.008, 0.008, 0.008, 0.008, 0.008, 0.008, 0.008, 0.0]);
    }

    #[test]
    fn rejects_mistakes_with_the_line_number() {
        for (text, expected) in [
            ("tie_pays = 8", "line 1: unknown setting \"tie_pays\""),
            ("tie = 8\ntie = 9", "line 2: tie is set twice"),
            ("tie = eight", "line 1: invalid value \"eight\" for tie"),
            ("banker_commission = 5", "line 1: banker_commission must be a number from 0 to under 1, found 5"),
            ("super_tie = [100, 25]", "line 1: super_tie must be 3 payouts, e.g. [100, 25, 7], found [100, 25]"),
            ("[rebate]\nbank = 0.01", "line 2: unknown rebate setting \"bank\""),
            ("[rebate]\non = \"sometimes\"", "line 2: on must be \"all\", \"losses\" or \"decided\", found \"sometimes\""),
            ("[payouts]", "line 1: unknown section [payouts]"),
        ] {
            assert_eq!(parse_payouts(text), Err(expected.to_string()), "{text:?}");
        }
    }
}
