//! A benchmark which tests simulation performance on the [`chaff_machines::constant`] machine with
//! the Tik-Tok unprocessed dataset.

#![expect(clippy::unwrap_used)]

use ::chaff::framework::Framework;
use chaff_capture::trace::Trace;
use chaff_datasets::{dataset::DatasetBuilder, parsers::chaff};
use chaff_machines::constant;
use chaff_sim::{Simulator, SimulatorOverheads};
use criterion::{Criterion, criterion_group, criterion_main};
use std::{hint::black_box, path::Path};

fn criterion_benchmark(c: &mut Criterion) {
    let workspace_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let dataset_dir = workspace_dir
        .parent()
        .and_then(Path::parent)
        .unwrap()
        .join("data/tik_tok_undefended.chaff");
    let dataset = chaff::try_parse(dataset_dir).unwrap();

    let data = dataset.get_dataset();
    let mut output_dataset_builder = DatasetBuilder::new(dataset.get_pad_to());

    let machine = constant::construct();
    let framework = Framework::new(machine, rand::rng());
    let mut sim = Simulator::with(framework, Trace::default(), rand::rng());

    c.bench_function("tiktok undefended const", |b| {
        b.iter(|| {
            let mut overheads = Vec::with_capacity(data.len());

            for (class, traces) in data {
                for trace in traces {
                    sim.replace_trace(black_box(trace.clone()));
                    let (trace, overhead) = sim.run();
                    output_dataset_builder.push_to_class(class, trace);
                    overheads.push(overhead);
                }
            }

            let overheads = SimulatorOverheads::total_from(overheads);
            black_box(overheads);
        });
    });
}

criterion_group!(
    name = benches;
    config = Criterion::default().sample_size(10);
    targets = criterion_benchmark
);
criterion_main!(benches);
