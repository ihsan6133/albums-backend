use std::{collections::HashMap, fmt::Display, time::{Duration, Instant}};


pub struct Perf {
    pub totals: HashMap<String, Duration>,
    instants  : HashMap<String, Instant>
}

impl Perf {
    pub fn new() -> Perf {
        Perf { totals: HashMap::new(), instants: HashMap::new() }
    }

    pub fn add(&mut self, perf :&Perf) {
        for (k, v) in &perf.totals {
            *self.totals.entry(k.to_string()).or_insert(Duration::ZERO) += v.to_owned();
        }
    }

    pub fn time<F, T>(&mut self, label: &str, f: F) -> T 
    where 
        F: FnOnce() -> T,
    {
        let start = Instant::now();
        let result = f();
        let elapsed = start.elapsed();

        *self.totals.entry(label.to_string()).or_insert(Duration::ZERO) += elapsed;

        result
    }

    pub fn start(&mut self, label: String) {
        let start = Instant::now();

        self.instants.insert(label, start);
    }

    pub fn end(&mut self, label: String) -> Duration {
        let i = self.instants.get(&label).expect(format!("Called timeEnd for label '{}' which was never started!", label).as_str());
        let elapsed = i.elapsed();

        *self.totals.entry(label.to_string()).or_insert(Duration::ZERO) += elapsed;

        elapsed
    }

    pub fn insert(&mut self, label: String, dur: Duration) -> Option<Duration> {
        self.totals.insert(label, dur)
    }
}

impl Display for Perf {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "performance times")?;

        for (k, v) in &self.totals {
            writeln!(f, "{}: {:.3}ms", k, v.as_secs_f64() * 1000.0f64)?;
        }

        Ok(())
    }
}