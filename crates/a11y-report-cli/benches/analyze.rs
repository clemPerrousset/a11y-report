//! Spec target: 100 screens analysed in less than 2 s in release mode.

use a11y_report_cli::config::EngineConfig;
use a11y_report_cli::engine::{AnalyzeOptions, analyze};
use a11y_report_cli::synth;
use a11y_report_core::Lang;
use criterion::{Criterion, criterion_group, criterion_main};
use std::time::Duration;

fn bench_analyze(c: &mut Criterion) {
    let dir = tempfile::tempdir().expect("tempdir");
    synth::write_bench_run(dir.path(), 100).expect("write bench run");
    let mut group = c.benchmark_group("analyze");
    group
        .sample_size(10)
        .measurement_time(Duration::from_secs(20));
    group.bench_function("100 screens, json + html", |b| {
        b.iter(|| {
            let rules = a11y_report_rules::all_rules();
            let config = EngineConfig::defaults(&rules);
            let analysis = analyze(&AnalyzeOptions {
                run_dir: dir.path().to_path_buf(),
                config,
                rules,
                baseline: None,
                lang: Lang::En,
                project_root: None,
                thumbnails: true,
            })
            .expect("analyze");
            let json = a11y_report_render::render_json(&analysis.report).expect("json");
            let html = a11y_report_render::render_html(&analysis.report, &analysis.thumbnails)
                .expect("html");
            (json.len(), html.len())
        });
    });
    group.finish();
}

criterion_group!(benches, bench_analyze);
criterion_main!(benches);
