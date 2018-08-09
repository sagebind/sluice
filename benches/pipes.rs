#[macro_use]
extern crate criterion;
extern crate ringtail;

use criterion::Criterion;
use std::io::{self, Write};
use std::thread;

fn pipe_read_write_benchmark(c: &mut Criterion) {
    c.bench_function("pipe_read_write", |b| {
        let data = [1; 0x100];

        b.iter(move || {
            let (mut r, mut w) = ringtail::io::pipe();

            let guard = thread::spawn(move || {
                for _ in 0..0x10 {
                    w.write_all(&data).unwrap();
                }
            });

            io::copy(&mut r, &mut io::sink()).unwrap();

            guard.join().unwrap();
        })
    });
}

criterion_group!(benches, pipe_read_write_benchmark);
criterion_main!(benches);
