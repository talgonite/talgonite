//! Small helpers shared across the workspace.

/// Runs `task(0..job_count)` across `worker_count` threads, returning results
/// keyed by index. Workers pull the next index from a shared counter, so the
/// work stays balanced even when individual jobs take different amounts of
/// time.
pub fn parallel_indexed<T, F>(job_count: usize, worker_count: usize, task: F) -> Vec<(usize, T)>
where
    T: Send,
    F: Fn(usize) -> T + Sync,
{
    let task = &task;

    std::thread::scope(|scope| {
        let next_index = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let mut jobs = Vec::with_capacity(worker_count);

        for _ in 0..worker_count {
            let next_index = next_index.clone();
            jobs.push(scope.spawn(move || {
                let mut local_results = Vec::new();

                loop {
                    let index = next_index.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    if index >= job_count {
                        break;
                    }

                    local_results.push((index, task(index)));
                }

                local_results
            }));
        }

        let mut results = Vec::with_capacity(job_count);
        for job in jobs {
            results.extend(job.join().expect("parallel worker thread panicked"));
        }

        results
    })
}
