//! What plants cost a frame, measured the same way every time: `bun run
//! shot --flora out/flora` stands in a wood of the default seed at a fixed
//! clock, inside it and over it, and at each preset and size draws the
//! frame many times. A GPU runs one pass while the last finishes, so a
//! pass's time is no share of the frame's: each of the foliage's draws, in
//! view, each form of it there, and in each cascade, is priced by what the
//! frame saves without it, the frames with and without taken in turn so a
//! drift of the clock falls on all alike. It says the CPU's time placing
//! the foliage, and counts what no GPU changes: the instances and triangles
//! of each pass by form, its draws, and the fragments its leaves
//! rasterize, keep and cover. A picture of each,
//! `<view>-<preset>-<W>x<H>.png`, and every number in `flora.json`; the
//! tables are printed.

use std::fmt::Write as _;
use std::path::Path;
use std::sync::OnceLock;
use std::time::Instant;

use client::{Client, Command, Effects};
use render::{Headless, Measured, Without, write_png};

use crate::shot::drawn_into;

/// Where the eye stands in a wood about a kilometre across on the default
/// seed: on the ground among the trees, and forty metres over them, the
/// camera six metres behind. The browser's measure stands at the same.
const VIEWS: [(&str, &str); 2] = [
    ("in", "0-9AR0000,360,-14,6"),
    ("over", "0-9AR0000@574,360,-12,6"),
];
/// The hour the sun stands high over the wood.
const CLOCK_S: f64 = 36_000.0;
const SIZES: [[u32; 2]; 2] = [[1280, 720], [1920, 1080]];
/// Frames drawn before any is timed, and timed.
const WARM: usize = 20;
const TIMED: usize = 120;

fn presets() -> [(&'static str, Effects); 3] {
    [
        ("high", Effects::default()),
        ("medium", Effects::medium()),
        ("light", Effects::light()),
    ]
}

/// Milliseconds since the measure began: the clock it lends the renderer.
fn now_ms() -> f64 {
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_secs_f64() * 1000.0
}

/// The draws of foliage a row leaves out, one at a time, and all at once.
fn priced() -> [(&'static str, Without); 8] {
    let cascade = |at: usize| Without {
        cascades: core::array::from_fn(|k| k == at),
        ..Without::default()
    };
    let form = |at: usize| Without {
        forms: core::array::from_fn(|k| k == at),
        ..Without::default()
    };
    [
        (
            "view",
            Without {
                view: true,
                ..Without::default()
            },
        ),
        ("near", form(0)),
        ("mid", form(1)),
        ("far", form(2)),
        ("cascade 0", cascade(0)),
        ("cascade 1", cascade(1)),
        ("cascade 2", cascade(2)),
        (
            "all",
            Without {
                view: true,
                cascades: [true; 3],
                ..Without::default()
            },
        ),
    ]
}

/// One view at one preset and size, measured.
struct Row {
    view: &'static str,
    preset: &'static str,
    size: [u32; 2],
    /// The GPU's median milliseconds of the frame, first pass to last.
    frame_ms: f64,
    /// Each pass's median milliseconds, in the order drawn.
    passes: Vec<(&'static str, f64)>,
    /// What the frame saves without each of [`priced`]'s draws, median
    /// milliseconds.
    saved: Vec<(&'static str, f64)>,
    /// The CPU's median milliseconds placing the foliage.
    gather_ms: f64,
    sort_ms: f64,
    write_ms: f64,
    measured: Measured,
}

pub fn run(client: &mut Client, headless: &mut Headless, dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let adapter = headless.adapter();
    println!("{} ({:?})", adapter.name, adapter.backend);
    headless.renderer.measure(Some(now_ms));
    let mut rows = Vec::new();
    for (view, pose) in VIEWS {
        client.go_to(pose)?;
        for (preset, effects) in presets() {
            client.command(Command::SetEffects { effects });
            client.set_clock(CLOCK_S);
            let frame = drawn_into(client, headless);
            for size in SIZES {
                headless.resize(size[0], size[1]);
                let row = measure(headless, &frame, view, preset, size);
                let pixels = headless.render(&frame);
                let name = format!("{view}-{preset}-{}x{}.png", size[0], size[1]);
                write_png(&dir.join(name), size[0], size[1], &pixels)?;
                rows.push(row);
            }
        }
    }
    let tables = tables(&rows);
    print!("{tables}");
    let out = dir.join("flora.json");
    std::fs::write(&out, json(&adapter, &rows)).map_err(|e| format!("{}: {e}", out.display()))?;
    println!("-> {}", out.display());
    Ok(())
}

fn median(mut values: Vec<f64>) -> f64 {
    values.sort_by(f64::total_cmp);
    values.get(values.len() / 2).copied().unwrap_or(0.0)
}

fn measure(
    headless: &mut Headless,
    frame: &scene::Frame,
    view: &'static str,
    preset: &'static str,
    size: [u32; 2],
) -> Row {
    let priced = priced();
    let mut whole = Vec::new();
    let mut without = vec![Vec::new(); priced.len()];
    let mut passes: Vec<(&'static str, Vec<f64>)> = Vec::new();
    let (mut gather, mut sort, mut write) = (Vec::new(), Vec::new(), Vec::new());
    for round in 0..WARM + TIMED {
        headless.renderer.measure_without(Without::default());
        headless.draw(frame);
        let measured = headless.renderer.measured().unwrap_or_default();
        let frame_ms = measured.passes.first().map_or(0.0, |(_, ms)| *ms);
        if round >= WARM {
            whole.push(frame_ms);
            for (at, (label, ms)) in measured.passes.iter().enumerate() {
                match passes.get_mut(at) {
                    Some((_, times)) => times.push(*ms),
                    None => passes.push((label, vec![*ms])),
                }
            }
            gather.push(measured.spent.gather_ms);
            sort.push(measured.spent.sort_ms);
            write.push(measured.spent.write_ms);
        }
        for ((_, leaving), times) in priced.iter().zip(&mut without) {
            headless.renderer.measure_without(*leaving);
            headless.draw(frame);
            let measured = headless.renderer.measured().unwrap_or_default();
            if round >= WARM {
                times.push(frame_ms - measured.passes.first().map_or(0.0, |(_, ms)| *ms));
            }
        }
    }
    headless.renderer.measure_without(Without::default());
    headless.renderer.count_foliage();
    headless.draw(frame);
    let measured = headless.renderer.measured().unwrap_or_default();
    Row {
        view,
        preset,
        size,
        frame_ms: median(whole),
        passes: passes
            .into_iter()
            .map(|(label, times)| (label, median(times)))
            .collect(),
        saved: priced
            .iter()
            .zip(without)
            .map(|((label, _), times)| (*label, median(times)))
            .collect(),
        gather_ms: median(gather),
        sort_ms: median(sort),
        write_ms: median(write),
        measured,
    }
}

impl Row {
    fn saved(&self, label: &str) -> f64 {
        self.saved
            .iter()
            .find(|(name, _)| *name == label)
            .map_or(0.0, |(_, ms)| *ms)
    }

    fn name(&self) -> String {
        format!(
            "{} {} {}x{}",
            self.view, self.preset, self.size[0], self.size[1]
        )
    }
}

/// The tables, as markdown.
fn tables(rows: &[Row]) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "\nGPU, ms (median of {TIMED}): the frame, first pass to last, and what it saves \
         without each draw of foliage: in view all of it, then each form\n\n\
         | view | frame | view foliage | near | mid | far | cascade 0 | cascade 1 | cascade 2 | all foliage | share |\n\
         |---|---|---|---|---|---|---|---|---|---|---|"
    );
    for row in rows {
        let all = row.saved("all");
        let _ = writeln!(
            out,
            "| {} | {:.2} | {:.2} | {:.2} | {:.2} | {:.2} | {:.2} | {:.2} | {:.2} | {:.2} | {:.0}% |",
            row.name(),
            row.frame_ms,
            row.saved("view"),
            row.saved("near"),
            row.saved("mid"),
            row.saved("far"),
            row.saved("cascade 0"),
            row.saved("cascade 1"),
            row.saved("cascade 2"),
            all,
            100.0 * all / row.frame_ms.max(f64::EPSILON),
        );
    }
    let _ = writeln!(
        out,
        "\nCPU placing the foliage, ms (median of {TIMED})\n\n\
         | view | gather | sort | write | all |\n|---|---|---|---|---|"
    );
    for row in rows.iter().filter(|row| row.size == SIZES[0]) {
        let _ = writeln!(
            out,
            "| {} {} | {:.3} | {:.3} | {:.3} | {:.3} |",
            row.view,
            row.preset,
            row.gather_ms,
            row.sort_ms,
            row.write_ms,
            row.gather_ms + row.sort_ms + row.write_ms
        );
    }
    let _ = writeln!(
        out,
        "\nCounts, the same at every size: instances and triangles by form (near, mid, \
         far), draws; fragments per pixel of the pass, rasterized (in view, by form), kept \
         past the cut, before the ground and covered, and rasterized per pixel covered\n\n\
         | view | pass | instances | triangles | draws | rasterized | by form | kept | before | covered | per covered |\n\
         |---|---|---|---|---|---|---|---|---|---|---|"
    );
    for row in rows.iter().filter(|row| row.size == SIZES[0]) {
        let passes = ["view", "cascade 0", "cascade 1", "cascade 2"];
        for (at, pass) in passes.iter().enumerate() {
            let counted = row.measured.counted[at];
            if counted.draws == 0 {
                continue;
            }
            let cells = row.measured.fragments.map(|all| all[at]).map(|f| {
                let share = |count: u64| format!("{:.2}", count as f64 / f.pixels.max(1) as f64);
                let forms = f
                    .forms
                    .map_or("-".to_owned(), |forms| forms.map(share).join(", "));
                [
                    share(f.rasterized),
                    forms,
                    share(f.kept),
                    f.before.map_or("-".to_owned(), share),
                    share(f.covered),
                    format!("{:.1}", f.rasterized as f64 / f.covered.max(1) as f64),
                ]
            });
            let [rasterized, forms, kept, before, covered, per] = cells.unwrap_or_default();
            let _ = writeln!(
                out,
                "| {} {} | {pass} | {:?} | {:?} | {} | {rasterized} | {forms} | {kept} | {before} | {covered} | {per} |",
                row.view, row.preset, counted.forms, counted.triangles, counted.draws,
            );
        }
    }
    out
}

fn json(adapter: &render::wgpu::AdapterInfo, rows: &[Row]) -> String {
    let entries = |pairs: &[(&str, f64)]| {
        pairs
            .iter()
            .map(|(label, ms)| format!("{label:?}: {ms:.4}"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let rows: Vec<String> = rows
        .iter()
        .map(|row| {
            format!(
                "    {{\"view\": {:?}, \"preset\": {:?}, \"size\": {:?}, \"frame_ms\": {:.4}, \
                 \"saved_ms\": {{{}}}, \"passes_ms\": {{{}}}, \
                 \"cpu_ms\": {{\"gather\": {:.4}, \"sort\": {:.4}, \"write\": {:.4}}}, \"counted\": {}}}",
                row.view,
                row.preset,
                row.size,
                row.frame_ms,
                entries(&row.saved),
                entries(&row.passes),
                row.gather_ms,
                row.sort_ms,
                row.write_ms,
                row.measured.json(),
            )
        })
        .collect();
    format!(
        "{{\n  \"adapter\": {:?},\n  \"backend\": \"{:?}\",\n  \"views\": {{{}}},\n  \"clock_s\": {CLOCK_S},\n  \"sizes\": {SIZES:?},\n  \"rows\": [\n{}\n  ]\n}}\n",
        adapter.name,
        adapter.backend,
        VIEWS
            .iter()
            .map(|(view, pose)| format!("{view:?}: {pose:?}"))
            .collect::<Vec<_>>()
            .join(", "),
        rows.join(",\n"),
    )
}
