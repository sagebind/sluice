#![feature(async_await)]

#[macro_use]
extern crate criterion;

use criterion::Criterion;
use futures::prelude::*;
use futures::executor::block_on;
use std::io;

fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("pipe_read_write", |b| {
        let data = [1; 0x1000];

        b.iter(move || {
            let (mut reader, mut writer) = sluice::pipe::chunked_pipe();

            let producer = async {
                for _ in 0..0x10 {
                    writer.write_all(&data).await.unwrap();
                }
                writer.close().await.unwrap();
            };

            let consumer = async {
                let mut sink = io::sink();
                reader.copy_into(&mut sink).await.unwrap();
            };

            block_on(future::join(producer, consumer));
        })
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
