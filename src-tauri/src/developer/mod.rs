use serde::Serialize;
use std::collections::{HashSet, VecDeque};
use sysinfo::{Pid, ProcessesToUpdate, System};
#[derive(Clone, Serialize)]
pub struct Sample {
    pub time: String,
    pub cpu_percent: Option<f32>,
    pub memory_bytes: u64,
    pub processes: usize,
}
pub struct Metrics {
    system: System,
    pub samples: VecDeque<Sample>,
    warmed: bool,
}
impl Default for Metrics {
    fn default() -> Self {
        Self {
            system: System::new(),
            samples: VecDeque::new(),
            warmed: false,
        }
    }
}
impl Metrics {
    pub fn sample(&mut self) -> Sample {
        self.system.refresh_processes(ProcessesToUpdate::All, true);
        let root = Pid::from_u32(std::process::id());
        let mut ids = HashSet::from([root]);
        loop {
            let before = ids.len();
            for (id, p) in self.system.processes() {
                if p.parent().is_some_and(|p| ids.contains(&p)) {
                    ids.insert(*id);
                }
            }
            if before == ids.len() {
                break;
            }
        }
        let mut memory = 0;
        let mut cpu = 0.;
        let mut count = 0;
        for id in ids {
            if let Some(p) = self.system.process(id) {
                memory += p.memory();
                cpu += p.cpu_usage();
                count += 1;
            }
        }
        let cores = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1) as f32;
        let result = Sample {
            time: chrono::Utc::now().to_rfc3339(),
            cpu_percent: if self.warmed { Some(cpu / cores) } else { None },
            memory_bytes: memory,
            processes: count,
        };
        self.warmed = true;
        self.samples.push_back(result.clone());
        while self.samples.len() > 120 {
            self.samples.pop_front();
        }
        result
    }
}
