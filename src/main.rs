use std::hash::{BuildHasher, RandomState};
use std::process::ExitCode;
use std::str::FromStr;
use std::time::SystemTime;

use baccarat_ev::{Bet, Card, DEFAULT_CUT, Outcome, Payouts, Shoe, SuperTie, analyze, simulate_shoe};

const USAGE: &str = "\
Baccarat (Punto Banco) odds: exact win rates and EV of each bet, or a simulated shoe.

Usage:
  baccarat-ev [analyze] [--decks N] [--dealt CARD...] [PAYOUTS]
  baccarat-ev simulate [--decks N] [--cut N] [--seed N] [PAYOUTS]

  --decks N        decks in the shoe (default: 8)
  --dealt CARD...  cards already dealt from the shoe, e.g. --dealt K 5 A 9 10
  --cut N          cut card position, in cards from the end (default: 14)
  --seed N         seed that replays a simulated shoe (default: random)

Payouts:
  --commission RATE       Banker commission (default: 0.05)
  --tie-pays N            Tie pays N:1 (default: 8)
  --super-tie-pays S,M,O  Super Tie pays S:1 on a same-color six-card tie of 6-9, M:1 on a
                          mixed-color one and O:1 on any other tie (default: 100,25,7)

Big pays 0.54:1 when 5 or 6 cards are dealt, Small 1.5:1 when only 4 are.";

struct Options {
    decks: u32,
    dealt: Vec<String>,
    cut: Option<usize>,
    seed: Option<u64>,
    payouts: Payouts,
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
    let mut options = Options { decks: 8, dealt: Vec::new(), cut: None, seed: None, payouts: Payouts::default() };
    let mut args = args.into_iter().peekable();
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--decks" => options.decks = parse_value(&flag, args.next())?,
            "--cut" => options.cut = Some(parse_value(&flag, args.next())?),
            "--seed" => options.seed = Some(parse_value(&flag, args.next())?),
            "--commission" => options.payouts.banker_commission = parse_value(&flag, args.next())?,
            "--tie-pays" => options.payouts.tie = parse_value(&flag, args.next())?,
            "--super-tie-pays" => options.payouts.super_tie = parse_super_tie_pays(&flag, args.next())?,
            "--dealt" => {
                while let Some(card) = args.next_if(|arg| !arg.starts_with("--")) {
                    options.dealt.push(card);
                }
            }
            _ => return Err(format!("unknown argument {flag:?}")),
        }
    }
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

fn run_analysis(options: &Options) -> Result<(), String> {
    if options.cut.is_some() || options.seed.is_some() {
        return Err("--cut and --seed only apply to simulate".to_string());
    }
    let mut shoe = Shoe::new(options.decks);
    for card in &options.dealt {
        shoe.remove(card)?;
    }
    let odds = analyze(&shoe)?;

    println!("Shoe: {} cards left ({} decks, {} dealt)\n", shoe.cards_left(), options.decks, options.dealt.len());
    println!("{:<11}{:>10}{:>10}{:>10}{:>12}{:>22}", "Bet", "Win", "Lose", "Push", "EV / unit", "Win rate excl. ties");
    for bet in Bet::ALL {
        let stats = odds.bet_stats(bet, &options.payouts);
        let decided = stats.win + stats.lose;
        let excluding_ties = if stats.push > 0.0 && decided > 0.0 { percent(stats.win / decided) } else { "-".to_string() };
        println!(
            "{:<11}{:>10}{:>10}{:>10}{:>12}{excluding_ties:>22}",
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
    let seed = options.seed.unwrap_or_else(|| RandomState::new().hash_one(SystemTime::now()));
    let run = simulate_shoe(options.decks, cut, seed)?;

    println!("Shoe: {} decks, seed {seed}, cut card {cut} cards from the end", options.decks);
    println!("Burned {} cards: exposed {}\n", run.burned.len(), run.burned[0]);
    println!("{:>4}  {:<16}{:<16}{:<8}{:<9}Super Tie", "#", "Player", "Banker", "Result", "Cards");

    let mut outcomes = [0usize; 3];
    let mut small = 0;
    let mut super_ties = [0usize; 3];
    let mut net = [0.0; Bet::ALL.len()];
    for (number, coup) in run.coups.iter().enumerate() {
        let result = coup.result();
        outcomes[result.outcome as usize] += 1;
        if result.cards == 4 {
            small += 1;
        }
        if let Some(category) = result.super_tie {
            super_ties[category as usize] += 1;
        }
        for (units, bet) in net.iter_mut().zip(Bet::ALL) {
            *units += options.payouts.net(bet, &result);
        }
        let size = format!("{} {}", result.cards, if result.cards == 4 { "Small" } else { "Big" });
        let super_tie = match result.super_tie {
            Some(SuperTie::SameColorSixCard) => "same color",
            Some(SuperTie::MixedColorSixCard) => "mixed color",
            Some(SuperTie::OtherTie) => "other tie",
            None => "",
        };
        println!(
            "{:>4}  {:<16}{:<16}{:<8}{size:<9}{super_tie}",
            number + 1,
            hand(&coup.player, coup.player_total()),
            hand(&coup.banker, coup.banker_total()),
            outcome_name(result.outcome),
        );
    }

    let coups = run.coups.len();
    let share = |count: usize| format!("{count} ({:.1}%)", 100.0 * count as f64 / coups as f64);
    println!("\n{coups} coups, {} cards left behind the last one", run.cards_left);
    println!("Banker {}, Player {}, Tie {}", share(outcomes[0]), share(outcomes[1]), share(outcomes[2]));
    println!("Big {}, Small {}", share(coups - small), share(small));
    println!("Super Tie wins: {} same color, {} mixed color, {} other ties", super_ties[0], super_ties[1], super_ties[2]);
    println!("\nNet units from a 1-unit bet on every coup:");
    for (bet, units) in Bet::ALL.into_iter().zip(net) {
        println!("  {:<11}{units:>+9.2}", bet.name());
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
