//! Bounded asynchronous whole-source digest orchestration.
#![allow(clippy::redundant_pub_crate)]

use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender, TrySendError},
    },
    thread::{self, JoinHandle},
};

use strata_core::{AnalysisRequestId, DomainError, SourceGeneration, SourceId};
use strata_source::DigestState;

use crate::AttachedSource;

/// Sealed identity of one immutable source generation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceDigestArtifact {
    /// Source identity captured before hashing began.
    pub source_id: SourceId,
    /// Source generation captured before hashing began.
    pub generation: SourceGeneration,
    /// Stable logical source length.
    pub byte_length: u64,
    /// Canonical lowercase whole-source SHA-256.
    pub sha256: String,
}

/// One progressive digest request.
pub struct RuntimeDigestRequest {
    /// Caller-owned identity used for cancellation and event correlation.
    pub request_id: AnalysisRequestId,
    /// Immutable source generation to seal.
    pub source: AttachedSource,
}

/// Observable digest lifecycle events.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DigestRuntimeEvent {
    /// A worker began hashing the source.
    Started {
        /// Request being serviced.
        request_id: AnalysisRequestId,
        /// Stable total byte count.
        total_bytes: u64,
    },
    /// A bounded step completed without sealing the digest.
    Progress {
        /// Request being serviced.
        request_id: AnalysisRequestId,
        /// Bytes incorporated into the canonical digest.
        bytes_hashed: u64,
        /// Stable total byte count.
        total_bytes: u64,
    },
    /// The whole immutable source was sealed.
    Completed {
        /// Request that produced the digest.
        request_id: AnalysisRequestId,
        /// Exact whole-source fingerprint.
        artifact: Arc<SourceDigestArtifact>,
    },
    /// Cancellation prevented digest publication.
    Cancelled {
        /// Cancelled request.
        request_id: AnalysisRequestId,
    },
    /// A newer or different generation suppressed publication.
    Stale {
        /// Suppressed request.
        request_id: AnalysisRequestId,
    },
    /// Hashing failed locally without terminating the runtime.
    Failed {
        /// Failed request.
        request_id: AnalysisRequestId,
        /// Bounded domain error.
        error: DomainError,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DigestRuntimeConfig {
    pub(crate) queue_capacity: usize,
    pub(crate) step_bytes: u64,
    pub(crate) progress_interval_bytes: u64,
}

impl DigestRuntimeConfig {
    fn validate(self) -> Result<(), DomainError> {
        if self.queue_capacity == 0 || self.step_bytes == 0 || self.progress_interval_bytes == 0 {
            return Err(DomainError::ResourceLimit(
                "all digest runtime limits must be greater than zero".to_owned(),
            ));
        }
        Ok(())
    }
}

struct ActiveDigest {
    source_id: SourceId,
    generation: SourceGeneration,
    cancelled: Arc<AtomicBool>,
}

struct DigestJob {
    request: RuntimeDigestRequest,
    source_id: SourceId,
    generation: SourceGeneration,
    total_bytes: u64,
    cancelled: Arc<AtomicBool>,
}

pub(crate) struct DigestRuntime {
    commands: Mutex<Option<SyncSender<DigestJob>>>,
    events: Mutex<Receiver<DigestRuntimeEvent>>,
    active: Arc<Mutex<HashMap<AnalysisRequestId, ActiveDigest>>>,
    latest_generations: Arc<Mutex<HashMap<SourceId, SourceGeneration>>>,
    worker: Option<JoinHandle<()>>,
}

impl DigestRuntime {
    pub(crate) fn new(config: DigestRuntimeConfig) -> Result<Self, DomainError> {
        config.validate()?;
        let (command_sender, command_receiver) = mpsc::sync_channel(config.queue_capacity);
        let (event_sender, event_receiver) = mpsc::channel();
        let active = Arc::new(Mutex::new(HashMap::new()));
        let latest_generations = Arc::new(Mutex::new(HashMap::new()));
        let worker_active = Arc::clone(&active);
        let worker_generations = Arc::clone(&latest_generations);
        let worker = thread::Builder::new()
            .name("strata-digest".to_owned())
            .spawn(move || {
                worker_loop(
                    &command_receiver,
                    &event_sender,
                    &worker_active,
                    &worker_generations,
                    config,
                );
            })
            .map_err(|error| {
                DomainError::Internal(format!("failed to start digest worker: {error}"))
            })?;
        Ok(Self {
            commands: Mutex::new(Some(command_sender)),
            events: Mutex::new(event_receiver),
            active,
            latest_generations,
            worker: Some(worker),
        })
    }

    pub(crate) fn submit(&self, request: RuntimeDigestRequest) -> Result<(), DomainError> {
        let descriptor = request.source.descriptor();
        let total_bytes = descriptor.length.ok_or_else(|| {
            DomainError::UnsupportedCapability("source length is unknown".to_owned())
        })?;
        {
            let mut generations = lock_recover(&self.latest_generations);
            match generations.get(&descriptor.id).copied() {
                Some(current) if descriptor.generation < current => {
                    return Err(DomainError::StaleGeneration);
                }
                Some(current) if descriptor.generation > current => {
                    generations.insert(descriptor.id, descriptor.generation);
                    cancel_older(&self.active, descriptor.id, descriptor.generation);
                }
                None => {
                    generations.insert(descriptor.id, descriptor.generation);
                }
                Some(_) => {}
            }
        }

        let cancelled = Arc::new(AtomicBool::new(false));
        {
            let mut active = lock_recover(&self.active);
            if active.contains_key(&request.request_id) {
                return Err(DomainError::InvalidTransform(
                    "digest request ID is already active".to_owned(),
                ));
            }
            active.insert(
                request.request_id,
                ActiveDigest {
                    source_id: descriptor.id,
                    generation: descriptor.generation,
                    cancelled: Arc::clone(&cancelled),
                },
            );
        }

        let request_id = request.request_id;
        let command = DigestJob {
            request,
            source_id: descriptor.id,
            generation: descriptor.generation,
            total_bytes,
            cancelled,
        };
        let sender = lock_recover(&self.commands).clone();
        let Some(sender) = sender else {
            remove_active(&self.active, request_id);
            return Err(DomainError::Cancelled);
        };
        match sender.try_send(command) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(_)) => {
                remove_active(&self.active, request_id);
                Err(DomainError::ResourceLimit(
                    "digest queue is full".to_owned(),
                ))
            }
            Err(TrySendError::Disconnected(_)) => {
                remove_active(&self.active, request_id);
                Err(DomainError::Internal(
                    "digest worker is unavailable".to_owned(),
                ))
            }
        }
    }

    pub(crate) fn cancel(&self, request_id: AnalysisRequestId) -> bool {
        let cancelled = lock_recover(&self.active)
            .get(&request_id)
            .map(|request| Arc::clone(&request.cancelled));
        let Some(cancelled) = cancelled else {
            return false;
        };
        cancelled.store(true, Ordering::Release);
        true
    }

    pub(crate) fn poll_event(&self) -> Option<DigestRuntimeEvent> {
        lock_recover(&self.events).try_recv().ok()
    }
}

impl Drop for DigestRuntime {
    fn drop(&mut self) {
        for request in lock_recover(&self.active).values() {
            request.cancelled.store(true, Ordering::Release);
        }
        let _ = lock_recover(&self.commands).take();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn worker_loop(
    commands: &Receiver<DigestJob>,
    events: &mpsc::Sender<DigestRuntimeEvent>,
    active: &Mutex<HashMap<AnalysisRequestId, ActiveDigest>>,
    latest_generations: &Mutex<HashMap<SourceId, SourceGeneration>>,
    config: DigestRuntimeConfig,
) {
    while let Ok(job) = commands.recv() {
        let request_id = job.request.request_id;
        let _ = events.send(DigestRuntimeEvent::Started {
            request_id,
            total_bytes: job.total_bytes,
        });
        let terminal = match execute_job(&job, events, latest_generations, config) {
            Ok(artifact) => DigestRuntimeEvent::Completed {
                request_id,
                artifact: Arc::new(artifact),
            },
            Err(DomainError::Cancelled) => DigestRuntimeEvent::Cancelled { request_id },
            Err(DomainError::StaleGeneration) => DigestRuntimeEvent::Stale { request_id },
            Err(error) => DigestRuntimeEvent::Failed { request_id, error },
        };
        remove_active(active, request_id);
        let _ = events.send(terminal);
    }
}

fn execute_job(
    job: &DigestJob,
    events: &mpsc::Sender<DigestRuntimeEvent>,
    latest_generations: &Mutex<HashMap<SourceId, SourceGeneration>>,
    config: DigestRuntimeConfig,
) -> Result<SourceDigestArtifact, DomainError> {
    let mut last_reported = 0_u64;
    loop {
        check_current(job, latest_generations)?;
        let progress = job.request.source.advance_digest(config.step_bytes)?;
        check_current(job, latest_generations)?;
        if progress.state == DigestState::Sealed {
            let sha256 = progress.content_digest.ok_or_else(|| {
                DomainError::Internal("sealed source digest is missing".to_owned())
            })?;
            return Ok(SourceDigestArtifact {
                source_id: job.source_id,
                generation: job.generation,
                byte_length: job.total_bytes,
                sha256,
            });
        }
        if progress.bytes_hashed.saturating_sub(last_reported) >= config.progress_interval_bytes {
            last_reported = progress.bytes_hashed;
            let _ = events.send(DigestRuntimeEvent::Progress {
                request_id: job.request.request_id,
                bytes_hashed: progress.bytes_hashed,
                total_bytes: progress.total_bytes,
            });
        }
    }
}

fn check_current(
    job: &DigestJob,
    latest_generations: &Mutex<HashMap<SourceId, SourceGeneration>>,
) -> Result<(), DomainError> {
    if job.cancelled.load(Ordering::Acquire) {
        return Err(DomainError::Cancelled);
    }
    let descriptor = job.request.source.descriptor();
    let latest = lock_recover(latest_generations)
        .get(&job.source_id)
        .copied();
    if descriptor.id != job.source_id {
        return Err(DomainError::StaleGeneration);
    }
    if descriptor.generation != job.generation || latest != Some(job.generation) {
        return Err(DomainError::StaleGeneration);
    }
    Ok(())
}

fn cancel_older(
    active: &Mutex<HashMap<AnalysisRequestId, ActiveDigest>>,
    source_id: SourceId,
    generation: SourceGeneration,
) {
    for request in lock_recover(active).values() {
        if request.source_id == source_id && request.generation < generation {
            request.cancelled.store(true, Ordering::Release);
        }
    }
}

fn remove_active(
    active: &Mutex<HashMap<AnalysisRequestId, ActiveDigest>>,
    request_id: AnalysisRequestId,
) {
    lock_recover(active).remove(&request_id);
}

fn lock_recover<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    match mutex.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

#[cfg(test)]
mod tests {
    use std::{
        error::Error,
        fs::OpenOptions,
        io::Write,
        sync::atomic::{AtomicU64, Ordering},
        time::{Duration, Instant},
    };

    use super::*;
    use strata_core::{SourceGeneration, SourceId};

    static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn retained_source_digest_completes_without_copying_source_bytes() -> Result<(), DomainError> {
        let runtime = DigestRuntime::new(DigestRuntimeConfig {
            queue_capacity: 1,
            step_bytes: 64,
            progress_interval_bytes: 64,
        })?;
        let source = AttachedSource::retained(
            SourceId(9),
            SourceGeneration(3),
            "digest fixture",
            Arc::<[u8]>::from(b"bounded digest fixture".repeat(16)),
        )?;
        runtime.submit(RuntimeDigestRequest {
            request_id: AnalysisRequestId(44),
            source,
        })?;
        let started = Instant::now();
        loop {
            if started.elapsed() > Duration::from_secs(2) {
                return Err(DomainError::Internal(
                    "digest test exceeded deadline".to_owned(),
                ));
            }
            match runtime.poll_event() {
                Some(DigestRuntimeEvent::Completed { artifact, .. }) => {
                    assert_eq!(artifact.byte_length, 352);
                    assert_eq!(artifact.sha256.len(), 64);
                    return Ok(());
                }
                Some(DigestRuntimeEvent::Failed { error, .. }) => return Err(error),
                Some(_) | None => thread::park_timeout(Duration::from_millis(1)),
            }
        }
    }

    #[test]
    fn local_digest_reports_progress_then_seals_descriptor() -> Result<(), Box<dyn Error>> {
        let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "strata-runtime-digest-{}-{counter}.bin",
            std::process::id()
        ));
        let result = (|| -> Result<(), Box<dyn Error>> {
            let mut file = OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&path)?;
            file.write_all(&vec![0x5a; 512 * 1024])?;
            file.sync_all()?;
            drop(file);
            let runtime = DigestRuntime::new(DigestRuntimeConfig {
                queue_capacity: 1,
                step_bytes: 16 * 1024,
                progress_interval_bytes: 32 * 1024,
            })?;
            let source = AttachedSource::open_local(&path, SourceId(10), SourceGeneration(7))?;
            runtime.submit(RuntimeDigestRequest {
                request_id: AnalysisRequestId(45),
                source: source.clone(),
            })?;
            let started = Instant::now();
            let mut saw_progress = false;
            loop {
                if started.elapsed() > Duration::from_secs(2) {
                    return Err(Box::new(std::io::Error::other(
                        "local digest test exceeded deadline",
                    )));
                }
                match runtime.poll_event() {
                    Some(DigestRuntimeEvent::Progress { .. }) => saw_progress = true,
                    Some(DigestRuntimeEvent::Completed { artifact, .. }) => {
                        assert!(saw_progress);
                        assert_eq!(artifact.byte_length, 512 * 1024);
                        assert_eq!(
                            source.descriptor().content_digest,
                            Some(artifact.sha256.clone())
                        );
                        break;
                    }
                    Some(DigestRuntimeEvent::Failed { error, .. }) => {
                        return Err(Box::new(error));
                    }
                    Some(_) | None => thread::park_timeout(Duration::from_millis(1)),
                }
            }
            Ok(())
        })();
        let _ = std::fs::remove_file(path);
        result
    }
}
