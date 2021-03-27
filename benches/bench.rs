#![feature(test)]

// Basic microbenchmark.
//
// Usage:
//
//     $ cargo bench --bench bench -- --use-index [...]
//

extern crate docopt;
extern crate shortcut;
extern crate test;
extern crate time;

use std::borrow::Cow;

use docopt::Docopt;
use shortcut::cmp;
use shortcut::idx;
use shortcut::Store;
use time::PreciseTime;

const USAGE: &'static str = "
Benchmark shortcut.

Usage:
  bench [--rounds=N --use-index --bench]

Options:
  --rounds=N               Number of rounds to run. [default: 1000000]
  --use-index              Install a hash index for fast lookups.
  --bench                  Appease `cargo bench`. No effect.
";

fn main() {
    let args = Docopt::new(USAGE)
        .and_then(|dopt| dopt.parse())
        .unwrap_or_else(|e| e.exit());

    let mut store = Store::new(2);

    let rounds: u32 = args.get_str("--rounds").parse().unwrap();

    if args.get_bool("--use-index") {
        store.index(0, idx::HashIndex::new());
    }

    let t0 = PreciseTime::now();

    // Put.
    for i in 0..rounds {
        let istr = format!("{}", i);
        store.insert(vec![istr.clone(), istr])
    }

    let t1 = PreciseTime::now();

    // Get.
    for i in 0..rounds {
        let cmp = [cmp::Condition {
            column: 0,
            cmp: cmp::Comparison::Equal(cmp::Value::Const(Cow::Owned(format!("{}", i)))),
        }];

        let rows = store.find(&cmp);

        for row in rows {
            test::black_box(row);
        }
    }

    let t2 = PreciseTime::now();

    println!(
        "put time: {:.2}ms ({:.2} puts/sec)",
        t0.to(t1).num_milliseconds(),
        ops_per_sec(rounds, t0, t1)
    );

    println!(
        "get time: {:.2}ms ({:.2} gets/sec)",
        t1.to(t2).num_milliseconds(),
        ops_per_sec(rounds, t1, t2)
    );
}

fn ops_per_sec(rounds: u32, start: PreciseTime, end: PreciseTime) -> f64 {
    1000.0 * (rounds as f64) / (start.to(end).num_milliseconds() as f64)
}
