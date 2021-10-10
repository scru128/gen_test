use std::env::args;
use std::io;
use std::io::prelude::*;

const STATS_INTERVAL: u64 = 10 * 1000;

const COUNTER_INCREMENT: u32 = 1;

fn main() {
    if let Some(arg) = args().nth(1) {
        let usage = "Usage: any-command-that-prints-identifiers-infinitely | scru128-test";
        if arg == "-h" || arg == "--help" {
            println!("{}", usage);
        } else {
            eprintln!("{}", usage);
            eprintln!("Error: unknown argument: {}", arg);
        }
        return;
    }

    println!(
        "Reading IDs from stdin and will show stats every {} seconds. Press Ctrl-C to quit.",
        STATS_INTERVAL / 1000
    );

    let mut st = Status::default();
    let mut prev = Identifier::default();
    for line in io::stdin().lock().lines() {
        let str_value = line.unwrap();
        let mut cs = str_value.chars();
        if str_value.len() == 26
            && cs.next().map_or(false, |c| c.is_digit(8))
            && cs.all(|c| c.is_digit(32))
        {
            st.n_processed += 1;
        } else {
            eprintln!("Error: invalid string representation");
            st.n_errors += 1;
            continue;
        }

        let e = Identifier::new(&str_value);
        if e.str_value <= prev.str_value {
            eprintln!("Error: string representation not monotonically ordered");
            st.n_errors += 1;
            continue;
        }
        if e.int_value <= prev.int_value {
            eprintln!("Error: integer representation not monotonically ordered");
            st.n_errors += 1;
            continue;
        }
        if e.timestamp < prev.timestamp {
            eprintln!("Error: clock went backwards");
            st.n_errors += 1;
            continue;
        } else if e.timestamp == prev.timestamp && e.counter <= prev.counter {
            eprintln!("Error: counter not monotonically ordered within same timestamp");
            st.n_errors += 1;
            continue;
        }

        // Triggered per line
        count_ones_by_bit(&mut st.n_ones_by_bit_per_gen_random, e.per_gen_random);

        // Triggered per millisecond
        if e.counter != prev.counter + COUNTER_INCREMENT {
            if st.ts_last_counter_update > 0 {
                st.n_counter_update += 1;
                st.sum_intervals_counter_update += e.timestamp - st.ts_last_counter_update;
            }
            st.ts_last_counter_update = e.timestamp;

            count_ones_by_bit(&mut st.n_ones_by_bit_counter, e.counter);
        }

        // Triggered per second
        if e.per_sec_random != prev.per_sec_random {
            if st.ts_last_per_sec_random_update > 0 {
                st.n_per_sec_random_update += 1;
                st.sum_intervals_per_sec_random_update +=
                    e.timestamp - st.ts_last_per_sec_random_update;
            }
            st.ts_last_per_sec_random_update = e.timestamp;

            count_ones_by_bit(&mut st.n_ones_by_bit_per_sec_random, e.per_sec_random);
        }

        // Triggered per STATS_INTERVAL seconds
        if e.timestamp > st.ts_last_stats_print + STATS_INTERVAL {
            if st.ts_last_stats_print > 0 {
                st.print();
            }
            st.ts_last_stats_print = e.timestamp;
        }

        // Prepare for next loop
        if st.ts_first == 0 {
            st.ts_first = e.timestamp;
        }
        st.ts_last = e.timestamp;
        prev = e;
    }

    if st.n_processed > 0 {
        st.print();
    } else {
        eprintln!("Error: no valid ID processed");
    }
}

#[derive(Debug, Default)]
struct Status {
    n_processed: usize,
    n_errors: usize,
    ts_first: u64,
    ts_last: u64,

    n_ones_by_bit_per_gen_random: [usize; 32],

    n_counter_update: usize,
    ts_last_counter_update: u64,
    sum_intervals_counter_update: u64,
    n_ones_by_bit_counter: [usize; 28],

    n_per_sec_random_update: usize,
    ts_last_per_sec_random_update: u64,
    sum_intervals_per_sec_random_update: u64,
    n_ones_by_bit_per_sec_random: [usize; 24],

    ts_last_stats_print: u64,
}

impl Status {
    fn print(&self) {
        let time_elapsed = self.ts_last - self.ts_first;

        let mut buf = io::BufWriter::new(io::stdout());
        writeln!(buf).unwrap();
        writeln!(buf, "{:<52} {:>8} {:>12}", "STAT", "EXPECTED", "ACTUAL").unwrap();
        writeln!(
            buf,
            "{:<52} {:>8} {:>12.1}",
            "Seconds from first input ID to last (sec)",
            "NA",
            time_elapsed as f64 / 1000.0
        )
        .unwrap();
        writeln!(
            buf,
            "{:<52} {:>8} {:>12}",
            "Number of valid IDs processed", "NA", self.n_processed
        )
        .unwrap();
        writeln!(
            buf,
            "{:<52} {:>8} {:>12}",
            "Number of invalid IDs skipped", 0, self.n_errors
        )
        .unwrap();
        writeln!(
            buf,
            "{:<52} {:>8} {:>12.1}",
            "Mean number of IDs per millisecond",
            "NA",
            self.n_processed as f64 / time_elapsed as f64
        )
        .unwrap();
        writeln!(
            buf,
            "{:<52} {:>8} {:>12.3}",
            "Mean interval of counter updates (msec)",
            "~1",
            self.sum_intervals_counter_update as f64 / self.n_counter_update as f64
        )
        .unwrap();
        writeln!(
            buf,
            "{:<52} {:>8} {:>12.3}",
            "Mean interval of per_sec_random updates (msec)",
            "~1000",
            self.sum_intervals_per_sec_random_update as f64 / self.n_per_sec_random_update as f64
        )
        .unwrap();

        writeln!(
            buf,
            "{:<52} {:>8} {:>12}",
            "1/0 ratio of each bit in counter at reset (min-max)",
            "~0.500",
            summarize_n_ones_by_bit(&self.n_ones_by_bit_counter, self.n_counter_update + 1)
        )
        .unwrap();
        writeln!(
            buf,
            "{:<52} {:>8} {:>12}",
            "1/0 ratio of each bit in per_sec_random (min-max)",
            "~0.500",
            summarize_n_ones_by_bit(
                &self.n_ones_by_bit_per_sec_random,
                self.n_per_sec_random_update + 1
            )
        )
        .unwrap();
        writeln!(
            buf,
            "{:<52} {:>8} {:>12}",
            "1/0 ratio of each bit in per_gen_random (min-max)",
            "~0.500",
            summarize_n_ones_by_bit(&self.n_ones_by_bit_per_gen_random, self.n_processed)
        )
        .unwrap();
    }
}

/// Holds representations and internal field values of a SCRU128 ID.
#[derive(Clone, Eq, PartialEq, Hash, Debug, Default)]
struct Identifier {
    str_value: String,
    int_value: u128,
    timestamp: u64,
    counter: u32,
    per_sec_random: u32,
    per_gen_random: u32,
}

impl Identifier {
    fn new(str_value: &str) -> Self {
        let int_value = u128::from_str_radix(str_value, 32).unwrap();
        Self {
            str_value: str_value.into(),
            int_value,
            timestamp: (int_value >> 84) as u64,
            counter: ((int_value >> 56) & 0xfff_ffff) as u32,
            per_sec_random: ((int_value >> 32) & 0xff_ffff) as u32,
            per_gen_random: (int_value & 0xffff_ffff) as u32,
        }
    }
}

/// Used to count the number of ones by bit number in the binary representations of integers.
fn count_ones_by_bit(counts: &mut [usize], bits: u32) {
    let mut n = bits;
    for i in 0..counts.len() {
        if n & 1 == 1 {
            counts[counts.len() - 1 - i] += 1;
        }
        n >>= 1;
    }
}

fn summarize_n_ones_by_bit(counts: &[usize], n_samples: usize) -> String {
    let mut min = 1.0;
    let mut max = 0.0;

    for e in counts {
        let p = *e as f64 / n_samples as f64;
        if p < min {
            min = p;
        }
        if p > max {
            max = p;
        }
    }

    format!("{:.3}-{:.3}", min, max)
}
