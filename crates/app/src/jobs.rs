#![allow(dead_code)] // Scheduler con deadlines: infraestructura para streaming futuro.
//! Job system con deadlines: ningún job bloquea el frame.
//! Cancelación cooperativa vía AtomicBool (chequeo cada N iters).

use rivaren_core::FrameId;
use std::collections::{BinaryHeap, HashMap};
use std::sync::atomic::AtomicBool;

type JobFn = Box<dyn FnOnce(&AtomicBool) + Send>;

struct Job {
    run_at: FrameId,
    id: u64,
    job: Option<JobFn>,
}

impl PartialEq for Job {
    fn eq(&self, o: &Self) -> bool {
        self.run_at == o.run_at && self.id == o.id
    }
}
impl Eq for Job {}
impl PartialOrd for Job {
    fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(o))
    }
}
impl Ord for Job {
    fn cmp(&self, o: &Self) -> std::cmp::Ordering {
        // min-heap por run_at
        o.run_at.cmp(&self.run_at).then(o.id.cmp(&self.id))
    }
}

pub struct JobSystem {
    queue: BinaryHeap<Job>,
    deadlines: HashMap<u64, FrameId>,
    next_id: u64,
}

impl JobSystem {
    pub fn new() -> Self {
        Self { queue: BinaryHeap::new(), deadlines: HashMap::new(), next_id: 0 }
    }
    pub fn submit(&mut self, run_at: FrameId, deadline: FrameId, job: JobFn) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.queue.push(Job { run_at, id, job: Some(job) });
        self.deadlines.insert(id, deadline);
        id
    }
    /// Ejecuta jobs con run_at <= frame; cancela vencidos (deadline < frame).
    pub fn tick(&mut self, frame: FrameId, cancel: &AtomicBool) {
        // 1. purga vencidos (sin ejecutar): se reprograman en el llamador.
        self.deadlines.retain(|_, d| *d >= frame);
        // 2. ejecuta vencidos de run_at (con límite: máx 4 por tick para no
        //    exceder el cpu budget del 40%).
        let mut ran = 0;
        while ran < 4 {
            let ready = self.queue.peek().map(|j| j.run_at <= frame).unwrap_or(false);
            if !ready {
                break;
            }
            let mut job = self.queue.pop().unwrap();
            if self.deadlines.remove(&job.id).is_none() {
                continue; // vencido → skip
            }
            if let Some(f) = job.job.take() {
                f(cancel);
                ran += 1;
            }
        }
    }
}

impl Default for JobSystem {
    fn default() -> Self {
        Self::new()
    }
}
