# perflab

A Rust command-line tool that measures how a program uses the CPU and memory. It runs a command several times, reads hardware counters from Linux `perf`, measures peak memory and wall time, and reports the median of each metric with a bootstrap confidence interval and a noise flag.

## Requirements

- Linux, with `perf` installed and hardware counters available to your user
- `taskset` (part of util-linux), needed only for pinned runs
- Rust stable

Check that counters work before running:

```fish
perf stat -e cycles -- sleep 0.1
```

## Usage

```fish
cargo run -- --warmup 2 --runs 14 --core 2 --out results/pinned -- ./my_program --input data.bin
```

Options come before `--`. Everything after `--` is the program to measure.

| Option | Meaning | Default |
|---|---|---|
| `--warmup` | Runs discarded before measurement | 2 |
| `--runs` | Measured runs | 14 |
| `--core` | CPU core to pin to (omit for no pinning) | none |
| `--out` | Folder for `run.json` and `run.md` | `results` |

The output folder is created if it doesn't exist. Each run overwrites the files in its folder, so give every configuration and session its own folder if you want to keep it.

## What it measures

For each run:

- **cycles, instructions, cache misses, branch misses** from `perf stat`
- **peak resident memory** from `wait4`, in kilobytes
- **wall time** around the `perf` call, in milliseconds

Peak memory comes from a separate execution of the same command. This keeps the memory call out of the timed section, at the cost of running the program twice per iteration.

## Method

- **Median.** Each metric is summarized by its median across the measured runs. Medians resist single outliers, which matters for wall time.
- **Bootstrap interval.** The 95% interval for the median comes from 10,000 resamples with a fixed seed, so the same data always gives the same interval. It estimates how precisely the median is known, not how much single runs vary.
- **Spread and noise flag.** Spread is `(max − min) / |median|`. A metric whose spread exceeds 5% is marked `!`, meaning its median varies too much across runs for small comparisons. The flag doesn't mean the value is wrong, only that differences smaller than the spread can't be resolved.
- **Pinning.** With `--core`, the command runs under `taskset`, so the scheduler can't move it between cores.

## Design choices

- **`None` instead of zero.** A counter that perf doesn't report is recorded as missing, so it can't be mistaken for a real zero.
- **Median over mean.** A single slow run changes a mean much more than a median.
- **Separate memory run.** Counters and memory come from different executions, so the memory call doesn't affect the timing.
- **Plain CSV parsing of `perf stat -x,`.** It's simple and easy to test, at the cost of depending on perf's output format.
- **Errors propagate.** A failing command, a missing program, or an impossible core stops the run and writes no files.

## Known limits

- **Inherited memory floor.** A child process starts with its parent's peak memory as its own high-water mark, so tiny programs report about 3.7 MB, the size of the measuring process, not their own footprint.
- **No isolated cores or fixed frequency.** Pinning stops the scheduler from moving the process, but not interrupts, other processes on the same core, or frequency scaling. The `isolcpus` boot option and the `performance` governor would address those.
- **Cycles are noisier than instructions.** Instructions depend only on the program's work. Cycles also depend on the machine's state.
- **Few runs give weak intervals.** With three runs, the bootstrap has few distinct values to draw from. Use the default of 14 or more.
- **Medians drift between sessions.** The same configuration gave different medians on different days, because the machine's state changes. Compare configurations within the same session.
- **Command display is joined by spaces.** Arguments containing spaces or shell characters are shown without quotes, so the displayed command may not match what was run.
- **Writes are not atomic.** If the process stops while writing, a partial file can remain.

## Results

Each configuration ran the same CPU-bound command, `sh -c 'i=0; while [ $i -lt 2000000 ]; do i=$((i+1)); done'`, with 2 warm-up runs and 14 measured runs. Two sessions were recorded, each with one pinned and one unpinned run:

- `results/session-1/pinned` and `results/session-1/unpinned`
- `results/session-2/pinned` and `results/session-2/unpinned`

Pinning to core 2 was verified during a run with `ps -eo pid,psr,comm`, which showed the measured process on that core.

Spread is `(max − min) / median`, shown as a percentage. The ranges below cover both sessions:

| Metric | Pinned (2 sessions) | Unpinned (2 sessions) | Flagged |
|---|---|---|---|
| cycles | 1.3% to 2.2% | 2.9% to 5.6% | unpinned in session 2 only |
| wall time | 1.2% to 1.4% | 1.5% to 5.8% | unpinned in session 2 only |
| instructions | ≈0% | ≈0% | neither |
| cache misses | 9% to 13% | 66% to 90% | all four flagged, pinned in both sessions |
| branch misses | flagged | flagged | all four |

What these runs show:

- **Cache misses** had the clearest and most repeatable difference. Unpinned medians were about 120,000 to 131,000 misses, against about 27,000 to 28,000 pinned, and the gap appeared in both sessions. This is consistent with the process moving between cores and losing its cache contents. Pinning reduced the spread a lot, but the pinned values were still flagged, so the remaining variation is worth investigating.
- **Cycles** had a lower spread when pinned in both sessions, but the difference is small. Unpinned exceeded the 5% threshold once, and pinned never did.
- **Wall time** followed the same pattern as cycles, with the unpinned spread flagged once.
- **Instructions** matched across all configurations, as expected for the same program.
- **Branch misses** were flagged in every configuration, so pinning didn't change that metric.

The honest reading is that pinning reduced cache-miss variance strongly and reduced cycle and wall-time variance modestly on this workload, on this machine, across two sessions. Two sessions are a small sample, so treat these as indications from one experiment, not general results.

## Reproducing

Run each session with its own folders, close together in time:

```fish
cargo run -- --warmup 2 --runs 14 --core 2 --out results/session-1/pinned -- sh -c 'i=0; while [ $i -lt 2000000 ]; do i=$((i+1)); done'
cargo run -- --warmup 2 --runs 14 --out results/session-1/unpinned -- sh -c 'i=0; while [ $i -lt 2000000 ]; do i=$((i+1)); done'
```

Use `session-2` in the paths for a second session. Results vary between machines and between sessions, so compare the spread and the flags within one session rather than the absolute numbers.

## Testing

```fish
cargo test                 # logic and error paths, runs anywhere
cargo test -- --ignored    # needs hardware perf counters, run locally
```

The test suite runs the same checks in CI. Tests that need counters are marked `#[ignore]` and run only locally.
