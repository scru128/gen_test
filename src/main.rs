use std::env::args;
use std::io;
use std::io::prelude::*;
use std::time::{SystemTime, UNIX_EPOCH};

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

    let stdin = io::stdin();
    let mut reader = stdin.lock();
    let mut buffer = String::with_capacity(32);
    println!(
        "Reading IDs from stdin and will show stats every {} seconds. Press Ctrl-C to quit.",
        STATS_INTERVAL / 1000
    );

    let mut st = Status::default();
    let mut prev = Identifier::default();
    while reader.read_line(&mut buffer).unwrap() > 0 {
        let line = buffer
            .strip_suffix('\n')
            .map_or(buffer.as_str(), |x| x.strip_suffix('\r').unwrap_or(x));
        let opt = Identifier::new(line);
        buffer.clear();
        if opt.is_some() {
            st.n_processed += 1;
        } else {
            eprintln!("Error: invalid string representation");
            st.n_errors += 1;
            continue;
        }

        let e = opt.unwrap();
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
        if st.ts_first == 0 {
            st.ts_first = e.timestamp;
        }
        st.ts_last = e.timestamp;

        count_set_bits_by_pos(&mut st.n_ones_by_bit_per_gen_random, e.per_gen_random);

        // Triggered per millisecond
        if e.counter != prev.counter + COUNTER_INCREMENT {
            if st.ts_last_counter_update > 0 {
                st.n_counter_update += 1;
                st.sum_intervals_counter_update += e.timestamp - st.ts_last_counter_update;
            }
            st.ts_last_counter_update = e.timestamp;

            count_set_bits_by_pos(&mut st.n_ones_by_bit_counter, e.counter);
        }

        // Triggered per second
        if e.per_sec_random != prev.per_sec_random {
            if st.ts_last_per_sec_random_update > 0 {
                st.n_per_sec_random_update += 1;
                st.sum_intervals_per_sec_random_update +=
                    e.timestamp - st.ts_last_per_sec_random_update;
            }
            st.ts_last_per_sec_random_update = e.timestamp;

            count_set_bits_by_pos(&mut st.n_ones_by_bit_per_sec_random, e.per_sec_random);
        }

        // Triggered per STATS_INTERVAL seconds
        if e.timestamp > st.ts_last_stats_print + STATS_INTERVAL {
            if st.ts_last_stats_print > 0 {
                st.print().unwrap();
            }
            st.ts_last_stats_print = e.timestamp;
        }

        // Prepare for next loop
        prev = e;
    }

    if st.n_processed > 0 {
        st.print().unwrap();
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
    fn print(&self) -> Result<(), io::Error> {
        let time_elapsed = self.ts_last - self.ts_first;

        let mut buf = io::BufWriter::new(io::stdout());
        writeln!(buf)?;
        writeln!(buf, "{:<52} {:>8} {:>12}", "STAT", "EXPECTED", "ACTUAL")?;
        writeln!(
            buf,
            "{:<52} {:>8} {:>12.1}",
            "Seconds from first input ID to last (sec)",
            "NA",
            time_elapsed as f64 / 1000.0
        )?;
        writeln!(
            buf,
            "{:<52} {:>8} {:>12}",
            "Number of valid IDs processed", "NA", self.n_processed
        )?;
        writeln!(
            buf,
            "{:<52} {:>8} {:>12}",
            "Number of invalid IDs skipped", 0, self.n_errors
        )?;
        writeln!(
            buf,
            "{:<52} {:>8} {:>12.3}",
            "Mean number of IDs per millisecond",
            "NA",
            self.n_processed as f64 / time_elapsed as f64
        )?;
        writeln!(
            buf,
            "{:<52} {:>8} {:>12.3}",
            "Biased current time less timestamp in last ID (sec)",
            "~0",
            get_biased_current_time() - (self.ts_last as f64) / 1000.0
        )?;
        writeln!(
            buf,
            "{:<52} {:>8} {:>12.3}",
            "Mean interval of counter updates (msec)",
            "~1",
            self.sum_intervals_counter_update as f64 / self.n_counter_update as f64
        )?;
        writeln!(
            buf,
            "{:<52} {:>8} {:>12.3}",
            "Mean interval of per_sec_random updates (msec)",
            "~1000",
            self.sum_intervals_per_sec_random_update as f64 / self.n_per_sec_random_update as f64
        )?;

        writeln!(
            buf,
            "{:<52} {:>8} {:>12}",
            "1/0 ratio of each bit in counter at reset (min-max)",
            "~0.500",
            summarize_n_set_bits_by_pos(&self.n_ones_by_bit_counter, self.n_counter_update + 1)
        )?;
        writeln!(
            buf,
            "{:<52} {:>8} {:>12}",
            "1/0 ratio of each bit in per_sec_random (min-max)",
            "~0.500",
            summarize_n_set_bits_by_pos(
                &self.n_ones_by_bit_per_sec_random,
                self.n_per_sec_random_update + 1
            )
        )?;
        writeln!(
            buf,
            "{:<52} {:>8} {:>12}",
            "1/0 ratio of each bit in per_gen_random (min-max)",
            "~0.500",
            summarize_n_set_bits_by_pos(&self.n_ones_by_bit_per_gen_random, self.n_processed)
        )?;

        Ok(())
    }
}

/// Holds representations and internal field values of a SCRU128 ID.
#[derive(Clone, Eq, PartialEq, Hash, Debug, Default)]
struct Identifier {
    str_value: [u8; 26],
    int_value: u128,
    timestamp: u64,
    counter: u32,
    per_sec_random: u32,
    per_gen_random: u32,
}

impl Identifier {
    fn new(str_value: &str) -> Option<Self> {
        const DECODE_MAP: [u8; 256] = [
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07,
            0x08, 0x09, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e,
            0x0f, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b, 0x1c,
            0x1d, 0x1e, 0x1f, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x0a,
            0x0b, 0x0c, 0x0d, 0x0e, 0x0f, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18,
            0x19, 0x1a, 0x1b, 0x1c, 0x1d, 0x1e, 0x1f, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
            0xff, 0xff, 0xff, 0xff,
        ];

        if str_value.len() != 26 {
            return None;
        }
        let mut fixed_str = [0; 26];
        let bs = str_value.as_bytes();

        fixed_str[0] = bs[0];
        let mut int_value = DECODE_MAP[bs[0] as usize] as u128;
        if int_value > 7 {
            return None;
        }

        for i in 1..26 {
            fixed_str[i] = bs[i];
            let n = DECODE_MAP[bs[i] as usize] as u128;
            if n == 0xff {
                return None;
            }
            int_value = (int_value << 5) | n;
        }

        Some(Self {
            str_value: fixed_str,
            int_value,
            timestamp: (int_value >> 84) as u64,
            counter: ((int_value >> 56) & 0xfff_ffff) as u32,
            per_sec_random: ((int_value >> 32) & 0xff_ffff) as u32,
            per_gen_random: (int_value & 0xffff_ffff) as u32,
        })
    }
}

fn get_biased_current_time() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock may have gone backwards")
        .as_secs_f64()
        - 1577836800.0
}

/// Used to count the number of set bits by bit position in the binary representations of integers.
#[allow(unused_mut)]
fn count_set_bits_by_pos(counts: &mut [usize], mut n: u32) {
    #[cfg(any(target_pointer_width = "32", target_pointer_width = "64"))]
    let mut n: usize = n as usize;

    for i in 0..counts.len() {
        counts[counts.len() - 1 - i] += (n & 1) as usize;
        n >>= 1;
    }
}

fn summarize_n_set_bits_by_pos(counts: &[usize], n_samples: usize) -> String {
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
