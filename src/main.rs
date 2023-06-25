use std::io::prelude::*;
use std::process::ExitCode;
use std::{env, io, mem, time};

const STATS_INTERVAL: u64 = 10 * 1000;

fn main() -> io::Result<ExitCode> {
    let mut args = env::args();
    let program = args.next();
    if let Some(arg) = args.next() {
        let usage = format!(
            "Usage: any-command-that-prints-identifiers-infinitely | {}",
            program.as_deref().unwrap_or("scru128-test")
        );
        return if arg == "-h" || arg == "--help" {
            println!("{usage}");
            Ok(ExitCode::SUCCESS)
        } else {
            eprintln!("Error: unknown argument: {arg}");
            eprintln!("{usage}");
            Ok(ExitCode::FAILURE)
        };
    }

    let mut reader = io::stdin().lock();
    let mut buffer = Vec::with_capacity(32);
    println!(
        "Reading IDs from stdin and will show stats every {} seconds. Press Ctrl-C to quit.",
        STATS_INTERVAL / 1000
    );

    let mut st = Status::default();
    let mut prev = Identifier::default();
    while {
        buffer.clear();
        reader.read_until(b'\n', &mut buffer)? > 0
    } {
        let line = match buffer.strip_suffix(b"\n") {
            Some(s) => s.strip_suffix(b"\r").unwrap_or(s),
            None => &buffer,
        };

        let Some(e) = Identifier::new(line) else {
            eprintln!("Error: invalid string representation");
            st.n_errors += 1;
            continue;
        };

        st.n_processed += 1;
        if e.str_bytes <= prev.str_bytes {
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
        } else if e.timestamp == prev.timestamp && e.counter_hi < prev.counter_hi {
            eprintln!("Error: counter_hi went backwards within same timestamp");
            st.n_errors += 1;
            continue;
        } else if e.timestamp == prev.timestamp
            && e.counter_hi == prev.counter_hi
            && e.counter_lo <= prev.counter_lo
        {
            eprintln!(
                "Error: counter_lo not monotonically ordered within same timestamp and counter_hi"
            );
            st.n_errors += 1;
            continue;
        }

        // Triggered per line
        if st.ts_first == 0 {
            st.ts_first = e.timestamp;
        }
        st.ts_last = e.timestamp;

        count_set_bits_by_pos(&mut st.n_ones_by_bit_entropy, e.entropy);

        // Triggered per millisecond
        if e.counter_lo != prev.counter_lo + 1
            && !(e.counter_lo == 0 && prev.counter_lo == 0xff_ffff)
        {
            if st.ts_last_counter_lo_update > 0 {
                st.n_counter_lo_update += 1;
                st.sum_intervals_counter_lo_update += e.timestamp - st.ts_last_counter_lo_update;
            }
            st.ts_last_counter_lo_update = e.timestamp;

            count_set_bits_by_pos(&mut st.n_ones_by_bit_counter_lo, e.counter_lo);
        }

        // Triggered per second or at counter_hi increment
        if e.counter_hi == prev.counter_hi + 1
            && e.timestamp == prev.timestamp
            && e.counter_lo == 0
            && prev.counter_lo == 0xff_ffff
        {
            st.n_counter_hi_increment += 1;
        } else if e.counter_hi != prev.counter_hi {
            if st.ts_last_counter_hi_update > 0 {
                st.n_counter_hi_update += 1;
                st.sum_intervals_counter_hi_update += e.timestamp - st.ts_last_counter_hi_update;
            }
            st.ts_last_counter_hi_update = e.timestamp;

            count_set_bits_by_pos(&mut st.n_ones_by_bit_counter_hi, e.counter_hi);
        }

        // Triggered per STATS_INTERVAL seconds
        if e.timestamp > st.ts_last_stats_print + STATS_INTERVAL {
            if st.ts_last_stats_print > 0 {
                st.print()?;
            }
            st.ts_last_stats_print = e.timestamp;
        }

        // Prepare for next loop
        prev = e;
    }

    if st.n_processed > 0 {
        st.print()?;
    } else {
        eprintln!("Error: no valid ID processed");
        return Ok(ExitCode::FAILURE);
    }

    if st.n_errors == 0 {
        Ok(ExitCode::SUCCESS)
    } else {
        Ok(ExitCode::FAILURE)
    }
}

#[derive(Debug, Default)]
struct Status {
    n_processed: usize,
    n_errors: usize,
    ts_first: u64,
    ts_last: u64,

    n_ones_by_bit_entropy: [usize; 32],

    n_counter_lo_update: usize,
    ts_last_counter_lo_update: u64,
    sum_intervals_counter_lo_update: u64,
    n_ones_by_bit_counter_lo: [usize; 24],

    n_counter_hi_increment: usize,
    n_counter_hi_update: usize,
    ts_last_counter_hi_update: u64,
    sum_intervals_counter_hi_update: u64,
    n_ones_by_bit_counter_hi: [usize; 24],

    ts_last_stats_print: u64,
}

impl Status {
    fn print(&self) -> io::Result<()> {
        let time_elapsed = self.ts_last - self.ts_first;

        let mut buf = io::stdout().lock();
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
            "Current time less timestamp of last ID (sec)",
            "~0",
            get_current_time() - (self.ts_last as f64) / 1000.0
        )?;
        writeln!(
            buf,
            "{:<52} {:>8} {:>12}",
            "Number of counter_hi increments", "Few", self.n_counter_hi_increment
        )?;
        writeln!(
            buf,
            "{:<52} {:>8} {:>12.3}",
            "Mean interval of counter_hi updates (msec)",
            "~1000",
            self.sum_intervals_counter_hi_update as f64 / self.n_counter_hi_update as f64
        )?;
        writeln!(
            buf,
            "{:<52} {:>8} {:>12.3}",
            "Mean interval of counter_lo updates (msec)",
            "~1",
            self.sum_intervals_counter_lo_update as f64 / self.n_counter_lo_update as f64
        )?;
        writeln!(
            buf,
            "{:<52} {:>8} {:>12}",
            "1/0 ratio by bit of counter_hi at reset (min-max)",
            "~0.500",
            summarize_n_set_bits_by_pos(
                &self.n_ones_by_bit_counter_hi,
                self.n_counter_hi_update + 1
            )
        )?;
        writeln!(
            buf,
            "{:<52} {:>8} {:>12}",
            "1/0 ratio by bit of counter_lo at reset (min-max)",
            "~0.500",
            summarize_n_set_bits_by_pos(
                &self.n_ones_by_bit_counter_lo,
                self.n_counter_lo_update + 1
            )
        )?;
        writeln!(
            buf,
            "{:<52} {:>8} {:>12}",
            "1/0 ratio by bit of entropy (min-max)",
            "~0.500",
            summarize_n_set_bits_by_pos(&self.n_ones_by_bit_entropy, self.n_processed)
        )?;

        Ok(())
    }
}

/// Holds representations and internal field values of a SCRU128 ID.
#[derive(Clone, Eq, PartialEq, Hash, Debug, Default)]
struct Identifier {
    str_bytes: [u8; 25],
    int_value: u128,
    timestamp: u64,
    counter_hi: u32,
    counter_lo: u32,
    entropy: u32,
}

impl Identifier {
    fn new(str_bytes: &[u8]) -> Option<Self> {
        const DECODE_MAP: [u8; 256] = [
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07,
            0x08, 0x09, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e,
            0x0f, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b, 0x1c,
            0x1d, 0x1e, 0x1f, 0x20, 0x21, 0x22, 0x23, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x0a,
            0x0b, 0x0c, 0x0d, 0x0e, 0x0f, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18,
            0x19, 0x1a, 0x1b, 0x1c, 0x1d, 0x1e, 0x1f, 0x20, 0x21, 0x22, 0x23, 0xff, 0xff, 0xff,
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

        let str_bytes = <[u8; 25]>::try_from(str_bytes).ok()?;
        let mut int_value = 0u128;

        let mut buffer = 0u64;
        let mut i = 0;
        while i < 25 {
            let n = DECODE_MAP[str_bytes[i] as usize] as u64;
            if n == 0xff {
                return None;
            }

            buffer = buffer * 36 + n;
            int_value = match i {
                7 | 15 => int_value
                    .checked_mul(36u128.pow(8))?
                    .checked_add(mem::replace(&mut buffer, 0).into())?,
                24 => int_value
                    .checked_mul(36u128.pow(9))?
                    .checked_add(mem::replace(&mut buffer, 0).into())?,
                _ => int_value,
            };

            i += 1;
        }

        Some(Self {
            str_bytes,
            int_value,
            timestamp: (int_value >> 80) as u64,
            counter_hi: ((int_value >> 56) & 0xff_ffff) as u32,
            counter_lo: ((int_value >> 32) & 0xff_ffff) as u32,
            entropy: (int_value & 0xffff_ffff) as u32,
        })
    }
}

fn get_current_time() -> f64 {
    time::SystemTime::now()
        .duration_since(time::UNIX_EPOCH)
        .expect("clock may have gone backwards")
        .as_secs_f64()
}

/// Used to count the number of set bits by bit position in the binary representations of integers.
#[allow(unused_mut)]
fn count_set_bits_by_pos(counts: &mut [usize], mut n: u32) {
    #[cfg(any(target_pointer_width = "32", target_pointer_width = "64"))]
    let mut n: usize = n as usize;

    for e in counts.iter_mut().rev() {
        *e += (n & 1) as usize;
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

    format!("{min:.3}-{max:.3}")
}
