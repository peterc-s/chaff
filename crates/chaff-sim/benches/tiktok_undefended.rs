//! A benchmark which tests simulation performance on a few machines with
//! the Tik-Tok unprocessed dataset.

#![expect(clippy::unwrap_used)]

use ::chaff::{
    action::IntegratorAction,
    distr::{Distr, DistrKind},
    event::Event,
    machine,
};
use chaff_cli::subcommands::simulate;
use chaff_datasets::parsers::chaff;
use chaff_machines::{constant, wtf_pad_lite};
use criterion::{Criterion, criterion_group, criterion_main};
use std::{hint::black_box, path::Path, time::Duration};

fn machines(c: &mut Criterion) {
    let workspace_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let dataset_dir = workspace_dir
        .parent()
        .and_then(Path::parent)
        .unwrap()
        .join("data/tik_tok_undefended.chaff");
    let dataset = chaff::try_parse(dataset_dir).unwrap();

    let delay_distr: Distr = DistrKind::Normal {
        mean: 0.015,
        std_dev: 0.05,
    }
    .try_into()
    .unwrap();
    let timeout: Distr = Duration::from_secs_f64(0.4).try_into().unwrap();
    let machine = wtf_pad_lite::construct(delay_distr, timeout);

    c.bench_function("tiktok undefended wtf_pad_lite normal", |b| {
        b.iter(|| simulate::run_dataset(black_box(&dataset), &None, black_box(&machine)));
    });

    let machine = constant::construct();

    c.bench_function("tiktok undefended const", |b| {
        b.iter(|| simulate::run_dataset(black_box(&dataset), &None, black_box(&machine)));
    });

    let machine = machine! {
        queues: [],
        state double {
            actions: [IntegratorAction::SendDecoy],
            transitions: [
                Event::SendNormal => double,
            ],
        },
    }
    .unwrap();

    c.bench_function("tiktok undefended double", |b| {
        b.iter(|| simulate::run_dataset(black_box(&dataset), &None, black_box(&machine)));
    });

    let machine = machine! {
        queues: [],
        state init {},
    }
    .unwrap();

    c.bench_function("tiktok undefended no_op", |b| {
        b.iter(|| simulate::run_dataset(black_box(&dataset), &None, black_box(&machine)));
    });
}

criterion_group!(
    name = tiktok_undefended;
    config = Criterion::default().sample_size(20);
    targets = machines
);
criterion_main!(tiktok_undefended);
