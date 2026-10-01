use std::path::Path;
use std::process::ExitCode;
use std::str::FromStr;

use baccarat_ev::{
    Bet, Card, DEFAULT_CONFIG, DEFAULT_CUT, Outcome, Payouts, RebateBasis, Shoe, SuperTie, analyze, load_payouts, random_seed,
    simulate_shoe,
};

const USAGE: &str = "\
Baccarat (Punto Banco) odds: exact win rates and EV of each bet, or a simulated shoe.

Usage:
  baccarat-ev [analyze] [--decks N] [--dealt CARD...] [PAYOUTS]
  baccarat-ev simulate [--decks N] [--cut N] [--seed N] [PAYOUTS]

  --decks N        decks in the shoe (default: 8)
  --dealt CARD...  cards already dealt from the shoe, e.g. --dealt K 5 A 9 10
  --cut N          cut card position, in cards from the end (default: 14)
  --seed N         seed that replays a simulated shoe (default: random)

Payouts (read from baccarat.toml in this folder if it exists; these options override it):
  --config FILE           read payouts and the rebate from FILE instead
  --commission RATE      Banker commission (default: 0.05)
  --tie-pays N            Tie pays N:1 (default: 8)
  --super-tie-pays S,M,O  Super Tie pays S:1 on a same-color six-card tie of 6-9, M:1 on a
                          mixed-color one and O:1 on any other tie (default: 100,25,7)
  --pair-pays N           Player Pair and Banker Pair pay N:1 (default: 11)
  --super-6-pays N        Super 6 pays N:1 when Banker wins with 6 (default: 12)

Rebate:
  --rebate RATE           pay back RATE of every bet, e.g. 0.008 for 0.8% (default: 0)
  --rebate BET=RATE,...   a rate per bet, others get none, e.g. banker=0.008,player=0.008
                          (bets: banker, player, tie, big, small, super-tie, player-pair,
                          banker-pair, super-6)
  --rebate-on all|losses|decided
                          which bets earn it: every bet, only losing bets, or every bet
                          that doesn't push (default: all)

Big pays 0.54:1 when 5 or 6 cards are dealt, Small 1.5:1 when only 4 are. A pair is two
cards of the same rank: K-K is a pair, K-Q is not.";

struct Options {
    decks: u32,
    dealt: Vec<String>,
    cut: Option<usize>,
    seed: Option<u64>,
    payouts: Payouts,
    /// Config file the payouts were read from, if any.
    config: Option<String>,
}

impl Options {
    fn payouts_source(&self) -> String {
        match &self.config {
            Some(path) => format!("Payouts: {path} (plus any payout options)"),
            None => "Payouts: built-in defaults (plus any payout options)".to_string(),
        }
    }
}

fn main() -> ExitCode {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "-h" || arg == "--help") {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    let simulate = args.first().is_some_and(|arg| arg == "simulate");
    if simulate || args.first().is_some_and(|arg| arg == "analyze") {
        args.remove(0);
    }
    let result = parse_args(args).and_then(|options| if simulate { run_simulation(&options) } else { run_analysis(&options) });
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("error: {message}\n\n{USAGE}");
            ExitCode::FAILURE
        }
    }
}

fn parse_args(args: Vec<String>) -> Result<Options, String> {
    // Payouts come from the config file first, so the options below can override them.
    let config = match args.iter().position(|arg| arg == "--config") {
        Some(index) => Some(args.get(index + 1).ok_or("--config needs a file")?.clone()),
        None => Path::new(DEFAULT_CONFIG).exists().then(|| DEFAULT_CONFIG.to_string()),
    };
    let payouts = match &config {
        Some(path) => load_payouts(Path::new(path))?,
        None => Payouts::default(),
    };
    let mut options = Options { decks: 8, dealt: Vec::new(), cut: None, seed: None, payouts, config };
    let mut args = args.into_iter().peekable();
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--config" => {
                args.next(); // already loaded above
            }
            "--decks" => options.decks = parse_value(&flag, args.next())?,
            "--cut" => options.cut = Some(parse_value(&flag, args.next())?),
            "--seed" => options.seed = Some(parse_value(&flag, args.next())?),
            "--commission" => options.payouts.banker_commission = parse_value(&flag, args.next())?,
            "--tie-pays" => options.payouts.tie = parse_value(&flag, args.next())?,
            "--super-tie-pays" => options.payouts.super_tie = parse_super_tie_pays(&flag, args.next())?,
            "--pair-pays" => options.payouts.pair = parse_value(&flag, args.next())?,
            "--super-6-pays" => options.payouts.super_six = parse_value(&flag, args.next())?,
            "--rebate" => options.payouts.rebate.rates = parse_rebate_rates(&flag, args.next())?,
            "--rebate-on" => options.payouts.rebate.basis = parse_rebate_basis(&flag, args.next())?,
            "--dealt" => {
                while let Some(card) = args.next_if(|arg| !arg.starts_with("--")) {
                    options.dealt.push(card);
                }
            }
            _ => return Err(format!("unknown argument {flag:?}")),
        }
    }
    options.payouts.validate()?;
    Ok(options)
}

fn parse_value<T: FromStr>(flag: &str, value: Option<String>) -> Result<T, String> {
    let value = value.ok_or_else(|| format!("{flag} needs a value"))?;
    value.parse().map_err(|_| format!("invalid value {value:?} for {flag}"))
}

fn parse_super_tie_pays(flag: &str, value: Option<String>) -> Result<[f64; 3], String> {
    let value = value.ok_or_else(|| format!("{flag} needs a value"))?;
    let pays: Vec<f64> = value
        .split(',')
        .map(|pay| pay.trim().parse())
        .collect::<Result<_, _>>()
        .map_err(|_| format!("invalid value {value:?} for {flag}"))?;
    pays.try_into().map_err(|_| format!("{flag} needs 3 payouts, e.g. 100,25,7"))
}

/// Parses one rate for every bet ("0.008") or a rate per bet ("banker=0.008,player=0.008").
fn parse_rebate_rates(flag: &str, value: Option<String>) -> Result<[f64; Bet::ALL.len()], String> {
    let value = value.ok_or_else(|| format!("{flag} needs a value"))?;
    let parse_rate = |rate: &str| -> Result<f64, String> {
        rate.trim()
            .parse()
            .ok()
            .filter(|rate| (0.0..1.0).contains(rate))
            .ok_or_else(|| format!("invalid rebate rate {rate:?} for {flag}: use a fraction from 0 to under 1, e.g. 0.008"))
    };
    if !value.contains('=') {
        return Ok([parse_rate(&value)?; Bet::ALL.len()]);
    }
    let mut rates = [0.0; Bet::ALL.len()];
    for entry in value.split(',') {
        let (name, rate) = entry.split_once('=').ok_or_else(|| format!("invalid rebate {entry:?} for {flag}, e.g. banker=0.008"))?;
        let bet = Bet::from_key(name).ok_or_else(|| format!("unknown bet {:?} for {flag}", name.trim()))?;
        rates[bet as usize] = parse_rate(rate)?;
    }
    Ok(rates)
}

fn parse_rebate_basis(flag: &str, value: Option<String>) -> Result<RebateBasis, String> {
    match value.as_deref() {
        Some("all") => Ok(RebateBasis::AllBets),
        Some("losses") => Ok(RebateBasis::Losses),
        Some("decided") => Ok(RebateBasis::DecidedBets),
        _ => Err(format!("{flag} needs one of: all, losses, decided")),
    }
}

fn run_analysis(options: &Options) -> Result<(), String> {
    if options.cut.is_some() || options.seed.is_some() {
        return Err("--cut and --seed only apply to simulate".to_string());
    }
    let mut shoe = Shoe::new(options.decks);
    for card in &options.dealt {
        shoe.remove(card)?;
    }
    let odds = analyze(&shoe)?;

    let has_rebate = !options.payouts.rebate.is_none();
    println!("Shoe: {} cards left ({} decks, {} dealt)", shoe.cards_left(), options.decks, options.dealt.len());
    println!("{}\n", options.payouts_source());
    let rebate_header = if has_rebate { format!("{:>10}{:>14}", "Rebate", "EV + rebate") } else { String::new() };
    println!("{:<13}{:>10}{:>10}{:>10}{:>12}{rebate_header}{:>22}", "Bet", "Win", "Lose", "Push", "EV / unit", "Win rate excl. ties");
    for bet in Bet::ALL {
        let stats = odds.bet_stats(bet, &options.payouts);
        let decided = stats.win + stats.lose;
        let excluding_ties = if stats.push > 0.0 && decided > 0.0 { percent(stats.win / decided) } else { "-".to_string() };
        let rebate = if has_rebate { format!("{:>10}{:>14}", percent(stats.rebate), percent(stats.ev_with_rebate())) } else { String::new() };
        println!(
            "{:<13}{:>10}{:>10}{:>10}{:>12}{rebate}{excluding_ties:>22}",
            bet.name(),
            percent(stats.win),
            percent(stats.lose),
            percent(stats.push),
            percent(stats.ev),
        );
    }

    let chance = |category| -> f64 {
        odds.results().into_iter().filter(|(result, _)| result.super_tie == Some(category)).map(|(_, probability)| probability).sum()
    };
    println!(
        "\nSuper Tie wins: {} same-color six-card tie of 6-9, {} mixed-color one, {} other ties",
        percent(chance(SuperTie::SameColorSixCard)),
        percent(chance(SuperTie::MixedColorSixCard)),
        percent(chance(SuperTie::OtherTie)),
    );
    Ok(())
}

fn run_simulation(options: &Options) -> Result<(), String> {
    if !options.dealt.is_empty() {
        return Err("--dealt only applies to analyze".to_string());
    }
    let cut = options.cut.unwrap_or(DEFAULT_CUT);
    let seed = options.seed.unwrap_or_else(random_seed);
    let run = simulate_shoe(options.decks, cut, seed)?;

    println!("Shoe: {} decks, seed {seed}, cut card {cut} cards from the end", options.decks);
    println!("{}", options.payouts_source());
    println!("Burned {} cards: exposed {}\n", run.burned.len(), run.burned[0]);
    println!("{:>4}  {:<16}{:<16}{:<8}{:<9}Side bets won", "#", "Player", "Banker", "Result", "Cards");

    let mut outcomes = [0usize; 3];
    let mut small = 0;
    let mut super_ties = [0usize; 3];
    let (mut player_pairs, mut banker_pairs, mut super_sixes) = (0, 0, 0);
    let mut net = [0.0; Bet::ALL.len()];
    let mut rebates = [0.0; Bet::ALL.len()];
    for (number, coup) in run.coups.iter().enumerate() {
        let result = coup.result();
        outcomes[result.outcome as usize] += 1;
        if result.cards == 4 {
            small += 1;
        }
        if let Some(category) = result.super_tie {
            super_ties[category as usize] += 1;
        }
        for (bet, (units, rebate)) in Bet::ALL.into_iter().zip(net.iter_mut().zip(&mut rebates)) {
            let before_rebate = options.payouts.net_before_rebate(bet, &result);
            *units += before_rebate;
            *rebate += options.payouts.rebate(bet, before_rebate);
        }
        player_pairs += usize::from(result.player_pair);
        banker_pairs += usize::from(result.banker_pair);
        super_sixes += usize::from(result.banker_six);
        let size = format!("{} {}", result.cards, if result.cards == 4 { "Small" } else { "Big" });
        let super_tie = result.super_tie.map(|category| match category {
            SuperTie::SameColorSixCard => "Super Tie (same color)",
            SuperTie::MixedColorSixCard => "Super Tie (mixed color)",
            SuperTie::OtherTie => "Super Tie",
        });
        let side_bets: Vec<&str> = [
            super_tie,
            result.player_pair.then_some("Player Pair"),
            result.banker_pair.then_some("Banker Pair"),
            result.banker_six.then_some("Super 6"),
        ]
        .into_iter()
        .flatten()
        .collect();
        println!(
            "{:>4}  {:<16}{:<16}{:<8}{size:<9}{}",
            number + 1,
            hand(&coup.player, coup.player_total()),
            hand(&coup.banker, coup.banker_total()),
            outcome_name(result.outcome),
            side_bets.join(", "),
        );
    }

    let coups = run.coups.len();
    let share = |count: usize| format!("{count} ({:.1}%)", 100.0 * count as f64 / coups as f64);
    println!("\n{coups} coups, {} cards left behind the last one", run.cards_left);
    println!("Banker {}, Player {}, Tie {}", share(outcomes[0]), share(outcomes[1]), share(outcomes[2]));
    println!("Big {}, Small {}", share(coups - small), share(small));
    println!("Super Tie wins: {} same color, {} mixed color, {} other ties", super_ties[0], super_ties[1], super_ties[2]);
    println!("Player Pair {player_pairs}, Banker Pair {banker_pairs}, Super 6 (Banker wins with 6) {super_sixes}");
    println!("\nNet units from a 1-unit bet on every coup:");
    if options.payouts.rebate.is_none() {
        for (bet, units) in Bet::ALL.into_iter().zip(net) {
            println!("  {:<13}{units:>+9.2}", bet.name());
        }
    } else {
        println!("  {:<13}{:>9}{:>9}{:>9}", "", "Bet", "Rebate", "Total");
        for (bet, (units, rebate)) in Bet::ALL.into_iter().zip(net.into_iter().zip(rebates)) {
            println!("  {:<13}{units:>+9.2}{rebate:>+9.2}{:>+9.2}", bet.name(), units + rebate);
        }
    }
    Ok(())
}

fn hand(cards: &[Card], total: usize) -> String {
    let cards: Vec<String> = cards.iter().map(Card::to_string).collect();
    format!("{:<12}{total}", cards.join(" "))
}

fn outcome_name(outcome: Outcome) -> &'static str {
    match outcome {
        Outcome::Banker => "Banker",
        Outcome::Player => "Player",
        Outcome::Tie => "Tie",
    }
}

fn percent(fraction: f64) -> String {
    format!("{:.4}%", 100.0 * fraction)
}
