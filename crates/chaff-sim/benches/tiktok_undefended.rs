//! A benchmark which tests simulation performance on a few machines with
//! the Tik-Tok unprocessed dataset.

#![expect(clippy::unwrap_used)]

use ::chaff::{action::IntegratorAction, event::Event, machine};
use chaff_cli::subcommands::simulate;
use chaff_datasets::parsers::chaff;
use chaff_machines::constant;
use criterion::{Criterion, criterion_group, criterion_main};
use std::{hint::black_box, path::Path};

fn constant(c: &mut Criterion) {
    let workspace_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let dataset_dir = workspace_dir
        .parent()
        .and_then(Path::parent)
        .unwrap()
        .join("data/tik_tok_undefended.chaff");
    let dataset = chaff::try_parse(dataset_dir).unwrap();
    let machine = constant::construct();

    c.bench_function("tiktok undefended const", |b| {
        b.iter(|| simulate::run_dataset(black_box(&dataset), &None, black_box(&machine)));
    });
}

fn no_op(c: &mut Criterion) {
    let workspace_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let dataset_dir = workspace_dir
        .parent()
        .and_then(Path::parent)
        .unwrap()
        .join("data/tik_tok_undefended.chaff");
    let dataset = chaff::try_parse(dataset_dir).unwrap();
    let machine = machine! {
        queues: [],
        state init {},
    }
    .unwrap();

    c.bench_function("tiktok undefended no_op", |b| {
        b.iter(|| simulate::run_dataset(black_box(&dataset), &None, black_box(&machine)));
    });
}

fn double(c: &mut Criterion) {
    let workspace_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let dataset_dir = workspace_dir
        .parent()
        .and_then(Path::parent)
        .unwrap()
        .join("data/tik_tok_undefended.chaff");
    let dataset = chaff::try_parse(dataset_dir).unwrap();
    let machine = machine! {
        queues: [],
        state double {
            action: IntegratorAction::SendDecoy,
            transitions: [
                Event::SendNormal => double,
            ],
        },
    }
    .unwrap();

    c.bench_function("tiktok undefended double", |b| {
        b.iter(|| simulate::run_dataset(black_box(&dataset), &None, black_box(&machine)));
    });
}

criterion_group!(
    name = tiktok_undefended;
    config = Criterion::default().sample_size(10);
    targets = constant, no_op, double
);
criterion_main!(tiktok_undefended);
