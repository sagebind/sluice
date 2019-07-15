#![cfg(feature = "nightly")]
#![feature(async_await)]

use criterion::*;

fn benchmark(c: &mut Criterion) {
    c.bench_function("write 100 1K chunks", |b| {
        use futures::executor::ThreadPool;
        use futures::prelude::*;

        let mut pool = ThreadPool::new().unwrap();
        let data = [1; 1024];

        b.iter_batched(
            sluice::pipe::pipe,
            |(mut reader, mut writer)| {
                let producer = async {
                    for _ in 0..100 {
                        writer.write_all(&data).await.unwrap();
                    }
                    writer.close().await.unwrap();
                };

                let consumer = async {
                    let mut sink = std::io::sink();
                    reader.copy_into(&mut sink).await.unwrap();
                };

                pool.run(future::join(producer, consumer));
            },
            BatchSize::SmallInput,
        )
    });
}

criterion_group!(benches, benchmark);
criterion_main!(benches);
