//! Small production analysis runtime for source-backed structure artifacts.

use std::{
    collections::{HashMap, VecDeque},
    future::Future,
    pin::Pin,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender, TrySendError},
    },
    task::{Context, Poll, Wake, Waker},
    thread::{self, JoinHandle},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use strata_core::{
    AnalysisRequestId, ByteRange, ByteRangeSet, DomainError, Priority, SourceGeneration, SourceId,
};
use strata_source::{ByteSource, ReadRequest};

use crate::poc::{ByteClass, EntropyBlock, block_shannon_entropy, classify_byte};

/// Semantic identity of the combined structure and entropy artifact.
pub const STRUCTURE_ENTROPY_SEMANTICS: &str = "strata.structure-entropy/v1";

/// Deterministic parameters shared by GUI and CLI analysis clients.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StructureEntropyPreset {
    /// Requested row width for the eventual byte atlas.
    pub atlas_width: u32,
    /// Exact, non-overlapping entropy block width.
    pub entropy_block_size: u32,
}

impl Default for StructureEntropyPreset {
    fn default() -> Self {
        Self {
            atlas_width: 256,
            entropy_block_size: 256,
        }
    }
}

impl StructureEntropyPreset {
    /// Rejects zero or unreasonably large dimensions before work is queued.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError::InvalidTransform`] when either dimension is zero
    /// or exceeds its documented bound.
    pub fn validate(self) -> Result<(), DomainError> {
        if self.atlas_width == 0 || self.atlas_width > 16_384 {
            return Err(DomainError::InvalidTransform(
                "atlas_width must be in 1..=16384".to_owned(),
            ));
        }
        if self.entropy_block_size == 0 || self.entropy_block_size > 1024 * 1024 {
            return Err(DomainError::InvalidTransform(
                "entropy_block_size must be in 1..=1048576".to_owned(),
            ));
        }
        Ok(())
    }
}

/// Per-range byte classifications retaining exact source coverage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassifiedRange {
    /// Exact source range represented by `classes`.
    pub range: ByteRange,
    /// One deterministic class for each byte in `range`.
    pub classes: Vec<ByteClass>,
}

/// Immutable artifact consumed by Structure and its entropy track.
#[derive(Debug, Clone, PartialEq)]
pub struct StructureEntropyArtifact {
    /// Runtime source identity used for provenance, not artifact hashing.
    pub source_id: SourceId,
    /// Source generation used for every read in this artifact.
    pub generation: SourceGeneration,
    /// Exact normalized source coverage.
    pub covered_ranges: ByteRangeSet,
    /// Parameters that define the artifact semantics.
    pub preset: StructureEntropyPreset,
    /// Byte classes grouped by exact source range.
    pub classified_ranges: Vec<ClassifiedRange>,
    /// Exact entropy blocks with absolute source offsets.
    pub entropy_blocks: Vec<EntropyBlock>,
    /// SHA-256 of canonical, source-independent artifact content.
    pub artifact_digest: String,
}

impl StructureEntropyArtifact {
    /// Approximate retained payload bytes used for cache budgeting.
    #[must_use]
    pub fn retained_bytes(&self) -> usize {
        let classes = self
            .classified_ranges
            .iter()
            .map(|range| range.classes.len())
            .sum::<usize>();
        classes.saturating_add(
            self.entropy_blocks
                .len()
                .saturating_mul(std::mem::size_of::<EntropyBlock>()),
        )
    }
}

/// One asynchronous source-backed structure request.
pub struct StructureEntropyRequest {
    /// Caller-owned identifier used for cancellation and publication.
    pub request_id: AnalysisRequestId,
    /// Immutable byte source.
    pub source: Arc<dyn ByteSource>,
    /// Exact source ranges to analyze.
    pub ranges: ByteRangeSet,
    /// Shared GUI/CLI preset.
    pub preset: StructureEntropyPreset,
    /// Scheduling priority forwarded to every bounded source read.
    pub priority: Priority,
}

/// Bounded runtime configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProductionRuntimeConfig {
    /// Maximum queued requests before submit reports backpressure.
    pub queue_capacity: usize,
    /// Maximum bytes read for one request.
    pub maximum_job_bytes: u64,
    /// Maximum bytes in each cancellable source read.
    pub read_chunk_bytes: u64,
    /// Maximum number of retained artifacts.
    pub cache_max_entries: usize,
    /// Maximum approximate artifact payload bytes.
    pub cache_max_bytes: usize,
}

impl Default for ProductionRuntimeConfig {
    fn default() -> Self {
        Self {
            queue_capacity: 8,
            maximum_job_bytes: 64 * 1024 * 1024,
            read_chunk_bytes: 1024 * 1024,
            cache_max_entries: 4,
            cache_max_bytes: 32 * 1024 * 1024,
        }
    }
}

impl ProductionRuntimeConfig {
    fn validate(self) -> Result<(), DomainError> {
        if self.queue_capacity == 0
            || self.maximum_job_bytes == 0
            || self.read_chunk_bytes == 0
            || self.cache_max_entries == 0
            || self.cache_max_bytes == 0
        {
            return Err(DomainError::ResourceLimit(
                "all production runtime limits must be greater than zero".to_owned(),
            ));
        }
        Ok(())
    }
}

/// Result-channel events. Only `Completed` publishes an artifact.
#[derive(Debug, Clone)]
pub enum ProductionRuntimeEvent {
    /// A worker began servicing the request.
    Started {
        /// Request being serviced.
        request_id: AnalysisRequestId,
    },
    /// A current, non-cancelled artifact is safe to attach to a view.
    Completed {
        /// Request that produced the artifact.
        request_id: AnalysisRequestId,
        /// Immutable result.
        artifact: Arc<StructureEntropyArtifact>,
        /// Whether analyzer execution and source reads were avoided.
        cache_hit: bool,
    },
    /// Cancellation prevented artifact publication.
    Cancelled {
        /// Cancelled request.
        request_id: AnalysisRequestId,
    },
    /// A newer source generation prevented stale publication.
    Stale {
        /// Suppressed request.
        request_id: AnalysisRequestId,
    },
    /// The request failed locally without terminating the runtime.
    Failed {
        /// Failed request.
        request_id: AnalysisRequestId,
        /// Bounded domain error.
        error: DomainError,
    },
}

/// Current in-memory cache occupancy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArtifactCacheStats {
    /// Number of retained artifacts.
    pub entries: usize,
    /// Approximate retained payload bytes.
    pub bytes: usize,
    /// Configured entry ceiling.
    pub maximum_entries: usize,
    /// Configured byte ceiling.
    pub maximum_bytes: usize,
}

struct ActiveRequest {
    source_id: SourceId,
    generation: SourceGeneration,
    cancelled: Arc<AtomicBool>,
}

struct RuntimeJob {
    request: StructureEntropyRequest,
    source_id: SourceId,
    generation: SourceGeneration,
    cache_key: String,
    cancelled: Arc<AtomicBool>,
}

enum WorkerCommand {
    Analyze(RuntimeJob),
    Shutdown,
}

struct ArtifactCache {
    entries: HashMap<String, (Arc<StructureEntropyArtifact>, usize)>,
    order: VecDeque<String>,
    bytes: usize,
    maximum_entries: usize,
    maximum_bytes: usize,
}

impl ArtifactCache {
    fn new(maximum_entries: usize, maximum_bytes: usize) -> Self {
        Self {
            entries: HashMap::new(),
            order: VecDeque::new(),
            bytes: 0,
            maximum_entries,
            maximum_bytes,
        }
    }

    fn get(&mut self, key: &str) -> Option<Arc<StructureEntropyArtifact>> {
        let artifact = self
            .entries
            .get(key)
            .map(|(artifact, _)| Arc::clone(artifact));
        if artifact.is_some() {
            self.order.retain(|candidate| candidate != key);
            self.order.push_back(key.to_owned());
        }
        artifact
    }

    fn insert(&mut self, key: String, artifact: Arc<StructureEntropyArtifact>) {
        let size = artifact.retained_bytes();
        if size > self.maximum_bytes {
            return;
        }
        if let Some((_, prior_size)) = self.entries.remove(&key) {
            self.bytes = self.bytes.saturating_sub(prior_size);
            self.order.retain(|candidate| candidate != &key);
        }
        self.bytes = self.bytes.saturating_add(size);
        self.order.push_back(key.clone());
        self.entries.insert(key, (artifact, size));

        while self.entries.len() > self.maximum_entries || self.bytes > self.maximum_bytes {
            let Some(oldest) = self.order.pop_front() else {
                break;
            };
            if let Some((_, removed_size)) = self.entries.remove(&oldest) {
                self.bytes = self.bytes.saturating_sub(removed_size);
            }
        }
    }

    fn stats(&self) -> ArtifactCacheStats {
        ArtifactCacheStats {
            entries: self.entries.len(),
            bytes: self.bytes,
            maximum_entries: self.maximum_entries,
            maximum_bytes: self.maximum_bytes,
        }
    }
}

/// One-worker bounded runtime with cancellation, stale suppression, and LRU cache.
pub struct ProductionAnalysisRuntime {
    commands: Mutex<Option<SyncSender<WorkerCommand>>>,
    events: Mutex<Receiver<ProductionRuntimeEvent>>,
    active: Arc<Mutex<HashMap<AnalysisRequestId, ActiveRequest>>>,
    latest_generations: Arc<Mutex<HashMap<SourceId, SourceGeneration>>>,
    cache: Arc<Mutex<ArtifactCache>>,
    worker: Option<JoinHandle<()>>,
    config: ProductionRuntimeConfig,
}

impl ProductionAnalysisRuntime {
    /// Starts the background worker after validating every resource limit.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError`] when configuration validation fails or the
    /// worker thread cannot be started.
    pub fn new(config: ProductionRuntimeConfig) -> Result<Self, DomainError> {
        config.validate()?;
        let (command_sender, command_receiver) = mpsc::sync_channel(config.queue_capacity);
        let (event_sender, event_receiver) = mpsc::channel();
        let active = Arc::new(Mutex::new(HashMap::new()));
        let latest_generations = Arc::new(Mutex::new(HashMap::new()));
        let cache = Arc::new(Mutex::new(ArtifactCache::new(
            config.cache_max_entries,
            config.cache_max_bytes,
        )));

        let worker_active = Arc::clone(&active);
        let worker_generations = Arc::clone(&latest_generations);
        let worker_cache = Arc::clone(&cache);
        let worker = thread::Builder::new()
            .name("strata-analysis".to_owned())
            .spawn(move || {
                worker_loop(
                    &command_receiver,
                    &event_sender,
                    &worker_active,
                    &worker_generations,
                    &worker_cache,
                    &config,
                );
            })
            .map_err(|error| {
                DomainError::Internal(format!("failed to start analysis worker: {error}"))
            })?;

        Ok(Self {
            commands: Mutex::new(Some(command_sender)),
            events: Mutex::new(event_receiver),
            active,
            latest_generations,
            cache,
            worker: Some(worker),
            config,
        })
    }

    /// Queues a request without blocking when the bounded queue is full.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError`] when the request is invalid, stale, duplicated,
    /// above the job bound, or cannot enter the bounded queue.
    pub fn submit(&self, request: StructureEntropyRequest) -> Result<(), DomainError> {
        request.preset.validate()?;
        let descriptor = request.source.descriptor();
        if request
            .ranges
            .total_len()
            .ok_or(DomainError::RangeOverflow)?
            > self.config.maximum_job_bytes
        {
            return Err(DomainError::ResourceLimit(format!(
                "analysis exceeds {} byte job limit",
                self.config.maximum_job_bytes
            )));
        }

        {
            let mut generations = lock_recover(&self.latest_generations);
            match generations.get(&descriptor.id) {
                Some(current) if descriptor.generation < *current => {
                    return Err(DomainError::StaleGeneration);
                }
                Some(current) if descriptor.generation > *current => {
                    *generations.get_mut(&descriptor.id).ok_or_else(|| {
                        DomainError::Internal("missing source generation".to_owned())
                    })? = descriptor.generation;
                    cancel_older_requests(&self.active, descriptor.id, descriptor.generation);
                }
                None => {
                    generations.insert(descriptor.id, descriptor.generation);
                }
                _ => {}
            }
        }

        let cancelled = Arc::new(AtomicBool::new(false));
        {
            let mut active = lock_recover(&self.active);
            if active.contains_key(&request.request_id) {
                return Err(DomainError::InvalidTransform(
                    "analysis request ID is already active".to_owned(),
                ));
            }
            active.insert(
                request.request_id,
                ActiveRequest {
                    source_id: descriptor.id,
                    generation: descriptor.generation,
                    cancelled: Arc::clone(&cancelled),
                },
            );
        }

        let request_id = request.request_id;
        let cache_key = cache_key(&descriptor, &request.ranges, request.preset);
        let command = WorkerCommand::Analyze(RuntimeJob {
            request,
            source_id: descriptor.id,
            generation: descriptor.generation,
            cache_key,
            cancelled,
        });
        let sender = {
            let commands = lock_recover(&self.commands);
            commands.clone()
        };
        let Some(sender) = sender else {
            remove_active(&self.active, request_id);
            return Err(DomainError::Cancelled);
        };
        let send_result = sender.try_send(command);
        match send_result {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(_)) => {
                remove_active(&self.active, request_id);
                Err(DomainError::ResourceLimit(
                    "analysis queue is full".to_owned(),
                ))
            }
            Err(TrySendError::Disconnected(_)) => {
                remove_active(&self.active, request_id);
                Err(DomainError::Internal(
                    "analysis worker is unavailable".to_owned(),
                ))
            }
        }
    }

    /// Marks an active or queued request cancelled.
    #[must_use]
    pub fn cancel(&self, request_id: AnalysisRequestId) -> bool {
        let cancelled = {
            let active = lock_recover(&self.active);
            active
                .get(&request_id)
                .map(|request| Arc::clone(&request.cancelled))
        };
        let Some(cancelled) = cancelled else {
            return false;
        };
        cancelled.store(true, Ordering::Release);
        true
    }

    /// Returns the next available event without blocking the caller.
    #[must_use]
    pub fn poll_event(&self) -> Option<ProductionRuntimeEvent> {
        let events = lock_recover(&self.events);
        events.try_recv().ok()
    }

    /// Returns current bounded-cache occupancy.
    #[must_use]
    pub fn cache_stats(&self) -> ArtifactCacheStats {
        lock_recover(&self.cache).stats()
    }
}

impl Drop for ProductionAnalysisRuntime {
    fn drop(&mut self) {
        for request in lock_recover(&self.active).values() {
            request.cancelled.store(true, Ordering::Release);
        }
        let sender = lock_recover(&self.commands).take();
        if let Some(sender) = sender {
            let _ = sender.try_send(WorkerCommand::Shutdown);
        }
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn worker_loop(
    commands: &Receiver<WorkerCommand>,
    events: &mpsc::Sender<ProductionRuntimeEvent>,
    active: &Arc<Mutex<HashMap<AnalysisRequestId, ActiveRequest>>>,
    latest_generations: &Arc<Mutex<HashMap<SourceId, SourceGeneration>>>,
    cache: &Arc<Mutex<ArtifactCache>>,
    config: &ProductionRuntimeConfig,
) {
    while let Ok(command) = commands.recv() {
        let WorkerCommand::Analyze(job) = command else {
            break;
        };
        let request_id = job.request.request_id;
        let _ = events.send(ProductionRuntimeEvent::Started { request_id });

        let outcome = execute_job(&job, latest_generations, cache, config);
        let event = match outcome {
            Ok((artifact, cache_hit)) => ProductionRuntimeEvent::Completed {
                request_id,
                artifact,
                cache_hit,
            },
            Err(DomainError::Cancelled) => ProductionRuntimeEvent::Cancelled { request_id },
            Err(DomainError::StaleGeneration) => ProductionRuntimeEvent::Stale { request_id },
            Err(error) => ProductionRuntimeEvent::Failed { request_id, error },
        };
        remove_active(active, request_id);
        let _ = events.send(event);
    }
}

fn execute_job(
    job: &RuntimeJob,
    latest_generations: &Mutex<HashMap<SourceId, SourceGeneration>>,
    cache: &Mutex<ArtifactCache>,
    config: &ProductionRuntimeConfig,
) -> Result<(Arc<StructureEntropyArtifact>, bool), DomainError> {
    check_current(job, latest_generations)?;
    let cached = { lock_recover(cache).get(&job.cache_key) };
    if let Some(artifact) = cached {
        check_current(job, latest_generations)?;
        return Ok((artifact, true));
    }

    let chunks = read_ranges(job, config)?;
    check_current(job, latest_generations)?;
    let artifact = Arc::new(build_structure_entropy_artifact(
        job.source_id,
        job.generation,
        job.request.ranges.clone(),
        job.request.preset,
        &chunks,
    )?);
    check_current(job, latest_generations)?;
    lock_recover(cache).insert(job.cache_key.clone(), Arc::clone(&artifact));
    Ok((artifact, false))
}

fn read_ranges(
    job: &RuntimeJob,
    config: &ProductionRuntimeConfig,
) -> Result<Vec<(ByteRange, Vec<u8>)>, DomainError> {
    let mut output = Vec::with_capacity(job.request.ranges.ranges.len());
    for range in &job.request.ranges.ranges {
        if range.start > range.end {
            return Err(DomainError::InvalidRange {
                start: range.start,
                end: range.end,
            });
        }
        let range_capacity = usize::try_from(range.len()).map_err(|_| {
            DomainError::ResourceLimit("analysis range does not fit platform memory".to_owned())
        })?;
        let mut bytes = Vec::with_capacity(range_capacity);
        let mut offset = range.start;
        while offset < range.end {
            check_cancelled(&job.cancelled)?;
            let end = offset
                .saturating_add(config.read_chunk_bytes)
                .min(range.end);
            let chunk_range = ByteRange::new(offset, end)?;
            let chunk = block_on(job.request.source.read(ReadRequest {
                source_id: job.source_id,
                generation: job.generation,
                ranges: ByteRangeSet {
                    ranges: vec![chunk_range],
                },
                priority: job.request.priority,
                maximum_bytes: chunk_range.len(),
            }))?;
            bytes.extend_from_slice(&chunk.bytes);
            offset = end;
        }
        output.push((*range, bytes));
    }
    Ok(output)
}

/// Builds the canonical artifact from already-read exact range payloads.
///
/// This pure reference path is public so deterministic frontends and tests can
/// verify artifact semantics without starting a scheduler.
///
/// # Errors
///
/// Returns [`DomainError`] when parameters, source coverage, chunk lengths, or
/// checked offset arithmetic violate the artifact contract.
pub fn build_structure_entropy_artifact(
    source_id: SourceId,
    generation: SourceGeneration,
    covered_ranges: ByteRangeSet,
    preset: StructureEntropyPreset,
    chunks: &[(ByteRange, Vec<u8>)],
) -> Result<StructureEntropyArtifact, DomainError> {
    preset.validate()?;
    if covered_ranges.ranges.len() != chunks.len() {
        return Err(DomainError::Internal(
            "range payload count does not match declared coverage".to_owned(),
        ));
    }

    let mut classified_ranges = Vec::with_capacity(chunks.len());
    let mut entropy_blocks = Vec::new();
    let block_size =
        usize::try_from(preset.entropy_block_size).map_err(|_| DomainError::RangeOverflow)?;
    for (expected, (range, bytes)) in covered_ranges.ranges.iter().zip(chunks) {
        if expected != range
            || usize::try_from(range.len()).map_err(|_| DomainError::RangeOverflow)? != bytes.len()
        {
            return Err(DomainError::SourceMismatch);
        }
        classified_ranges.push(ClassifiedRange {
            range: *range,
            classes: bytes.iter().copied().map(classify_byte).collect(),
        });
        for mut block in block_shannon_entropy(bytes, block_size)? {
            block.offset = range
                .start
                .checked_add(block.offset)
                .ok_or(DomainError::RangeOverflow)?;
            entropy_blocks.push(block);
        }
    }

    let artifact_digest =
        artifact_digest(&covered_ranges, preset, &classified_ranges, &entropy_blocks);
    Ok(StructureEntropyArtifact {
        source_id,
        generation,
        covered_ranges,
        preset,
        classified_ranges,
        entropy_blocks,
        artifact_digest,
    })
}

fn check_current(
    job: &RuntimeJob,
    latest_generations: &Mutex<HashMap<SourceId, SourceGeneration>>,
) -> Result<(), DomainError> {
    check_cancelled(&job.cancelled)?;
    let latest = lock_recover(latest_generations)
        .get(&job.source_id)
        .copied();
    let descriptor = job.request.source.descriptor();
    if descriptor.id != job.source_id {
        return Err(DomainError::StaleGeneration);
    }
    if descriptor.generation != job.generation || latest != Some(job.generation) {
        return Err(DomainError::StaleGeneration);
    }
    Ok(())
}

fn check_cancelled(cancelled: &AtomicBool) -> Result<(), DomainError> {
    if cancelled.load(Ordering::Acquire) {
        return Err(DomainError::Cancelled);
    }
    Ok(())
}

fn cache_key(
    descriptor: &strata_source::SourceDescriptor,
    ranges: &ByteRangeSet,
    preset: StructureEntropyPreset,
) -> String {
    let mut digest = Sha256::new();
    digest.update(b"strata.runtime-cache/v1\0");
    digest.update(descriptor.id.0.to_le_bytes());
    digest.update(descriptor.generation.0.to_le_bytes());
    for range in &ranges.ranges {
        digest.update(range.start.to_le_bytes());
        digest.update(range.end.to_le_bytes());
    }
    digest.update(preset.atlas_width.to_le_bytes());
    digest.update(preset.entropy_block_size.to_le_bytes());
    format!("{:x}", digest.finalize())
}

fn artifact_digest(
    ranges: &ByteRangeSet,
    preset: StructureEntropyPreset,
    classified_ranges: &[ClassifiedRange],
    entropy_blocks: &[EntropyBlock],
) -> String {
    let mut digest = Sha256::new();
    digest.update(STRUCTURE_ENTROPY_SEMANTICS.as_bytes());
    digest.update([0]);
    digest.update(preset.atlas_width.to_le_bytes());
    digest.update(preset.entropy_block_size.to_le_bytes());
    for range in &ranges.ranges {
        digest.update(range.start.to_le_bytes());
        digest.update(range.end.to_le_bytes());
    }
    for classified in classified_ranges {
        digest.update(classified.range.start.to_le_bytes());
        digest.update(classified.range.end.to_le_bytes());
        for class in &classified.classes {
            digest.update([byte_class_code(*class)]);
        }
    }
    for block in entropy_blocks {
        digest.update(block.offset.to_le_bytes());
        digest.update(block.length.to_le_bytes());
        digest.update(block.shannon_entropy_bits.to_bits().to_le_bytes());
    }
    format!("{:x}", digest.finalize())
}

const fn byte_class_code(class: ByteClass) -> u8 {
    match class {
        ByteClass::Zero => 0,
        ByteClass::AllOnes => 1,
        ByteClass::Whitespace => 2,
        ByteClass::PrintableAscii => 3,
        ByteClass::Control => 4,
        ByteClass::HighBit => 5,
    }
}

fn cancel_older_requests(
    active: &Mutex<HashMap<AnalysisRequestId, ActiveRequest>>,
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
    active: &Mutex<HashMap<AnalysisRequestId, ActiveRequest>>,
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

struct ThreadWake {
    thread: thread::Thread,
}

impl Wake for ThreadWake {
    fn wake(self: Arc<Self>) {
        self.thread.unpark();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.thread.unpark();
    }
}

fn block_on<T>(mut future: Pin<Box<dyn Future<Output = T> + Send + '_>>) -> T {
    let waker = Waker::from(Arc::new(ThreadWake {
        thread: thread::current(),
    }));
    let mut context = Context::from_waker(&waker);
    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(output) => return output,
            Poll::Pending => thread::park(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        error::Error,
        time::{Duration, Instant},
    };

    use strata_core::ByteRange;
    use strata_source::{BoxFuture, ByteChunk, DigestState, SourceCapabilities, SourceDescriptor};

    use super::*;

    struct MemorySource {
        descriptor: SourceDescriptor,
        bytes: Vec<u8>,
        delay: Duration,
    }

    impl MemorySource {
        fn new(id: u128, generation: u64, bytes: Vec<u8>, delay: Duration) -> Self {
            Self {
                descriptor: SourceDescriptor {
                    id: SourceId(id),
                    generation: SourceGeneration(generation),
                    display_name: "memory fixture".to_owned(),
                    length: u64::try_from(bytes.len()).ok(),
                    primary_address_space: strata_core::AddressSpaceId("file-offset".to_owned()),
                    capabilities: SourceCapabilities(
                        SourceCapabilities::KNOWN_LENGTH | SourceCapabilities::RANDOM_READ,
                    ),
                    content_digest: None,
                    digest_state: DigestState::Unknown,
                    unstable: false,
                },
                bytes,
                delay,
            }
        }
    }

    impl ByteSource for MemorySource {
        fn descriptor(&self) -> SourceDescriptor {
            self.descriptor.clone()
        }

        fn read(&self, request: ReadRequest) -> BoxFuture<'_, Result<ByteChunk, DomainError>> {
            Box::pin(async move {
                thread::sleep(self.delay);
                if request.source_id != self.descriptor.id {
                    return Err(DomainError::SourceMismatch);
                }
                if request.generation != self.descriptor.generation {
                    return Err(DomainError::StaleGeneration);
                }
                let mut output = Vec::new();
                for range in &request.ranges.ranges {
                    let start =
                        usize::try_from(range.start).map_err(|_| DomainError::RangeOverflow)?;
                    let end = usize::try_from(range.end).map_err(|_| DomainError::RangeOverflow)?;
                    let bytes = self
                        .bytes
                        .get(start..end)
                        .ok_or(DomainError::InvalidRange {
                            start: range.start,
                            end: range.end,
                        })?;
                    output.extend_from_slice(bytes);
                }
                Ok(ByteChunk {
                    source_id: self.descriptor.id,
                    generation: self.descriptor.generation,
                    ranges: request.ranges,
                    bytes: output,
                    complete: true,
                })
            })
        }
    }

    fn request(
        request_id: u128,
        source: Arc<dyn ByteSource>,
        byte_length: u64,
    ) -> Result<StructureEntropyRequest, DomainError> {
        Ok(StructureEntropyRequest {
            request_id: AnalysisRequestId(request_id),
            source,
            ranges: ByteRangeSet {
                ranges: vec![ByteRange::new(0, byte_length)?],
            },
            preset: StructureEntropyPreset {
                atlas_width: 64,
                entropy_block_size: 64,
            },
            priority: Priority::Visible,
        })
    }

    fn wait_for_terminal(
        runtime: &ProductionAnalysisRuntime,
        request_id: AnalysisRequestId,
    ) -> Result<ProductionRuntimeEvent, DomainError> {
        let deadline = Instant::now() + Duration::from_secs(3);
        while Instant::now() < deadline {
            if let Some(event) = runtime.poll_event() {
                let event_id = match &event {
                    ProductionRuntimeEvent::Started { request_id }
                    | ProductionRuntimeEvent::Completed { request_id, .. }
                    | ProductionRuntimeEvent::Cancelled { request_id }
                    | ProductionRuntimeEvent::Stale { request_id }
                    | ProductionRuntimeEvent::Failed { request_id, .. } => *request_id,
                };
                if event_id == request_id
                    && !matches!(event, ProductionRuntimeEvent::Started { .. })
                {
                    return Ok(event);
                }
            }
            thread::sleep(Duration::from_millis(1));
        }
        Err(DomainError::Internal(
            "timed out waiting for analysis event".to_owned(),
        ))
    }

    #[test]
    fn cancellation_prevents_publication() -> Result<(), Box<dyn Error>> {
        let runtime = ProductionAnalysisRuntime::new(ProductionRuntimeConfig {
            read_chunk_bytes: 4096,
            ..ProductionRuntimeConfig::default()
        })?;
        let bytes = vec![42_u8; 256 * 1024];
        let source: Arc<dyn ByteSource> =
            Arc::new(MemorySource::new(11, 1, bytes, Duration::from_millis(2)));
        runtime.submit(request(1, source, 256 * 1024)?)?;
        while !matches!(
            runtime.poll_event(),
            Some(ProductionRuntimeEvent::Started { .. })
        ) {
            thread::sleep(Duration::from_millis(1));
        }
        assert!(runtime.cancel(AnalysisRequestId(1)));
        assert!(matches!(
            wait_for_terminal(&runtime, AnalysisRequestId(1))?,
            ProductionRuntimeEvent::Cancelled { .. }
        ));
        Ok(())
    }

    #[test]
    fn newer_generation_suppresses_old_result() -> Result<(), Box<dyn Error>> {
        let runtime = ProductionAnalysisRuntime::new(ProductionRuntimeConfig {
            read_chunk_bytes: 4096,
            ..ProductionRuntimeConfig::default()
        })?;
        let old: Arc<dyn ByteSource> = Arc::new(MemorySource::new(
            21,
            1,
            vec![1; 128 * 1024],
            Duration::from_millis(2),
        ));
        let new: Arc<dyn ByteSource> =
            Arc::new(MemorySource::new(21, 2, vec![2; 8192], Duration::ZERO));
        runtime.submit(request(1, old, 128 * 1024)?)?;
        while !matches!(
            runtime.poll_event(),
            Some(ProductionRuntimeEvent::Started { .. })
        ) {
            thread::sleep(Duration::from_millis(1));
        }
        runtime.submit(request(2, new, 8192)?)?;

        assert!(!matches!(
            wait_for_terminal(&runtime, AnalysisRequestId(1))?,
            ProductionRuntimeEvent::Completed { .. }
        ));
        assert!(matches!(
            wait_for_terminal(&runtime, AnalysisRequestId(2))?,
            ProductionRuntimeEvent::Completed { .. }
        ));
        Ok(())
    }

    #[test]
    fn cache_hits_and_evicts_by_entry_limit() -> Result<(), Box<dyn Error>> {
        let runtime = ProductionAnalysisRuntime::new(ProductionRuntimeConfig {
            cache_max_entries: 1,
            cache_max_bytes: 1024 * 1024,
            ..ProductionRuntimeConfig::default()
        })?;
        let first: Arc<dyn ByteSource> =
            Arc::new(MemorySource::new(31, 1, vec![3; 1024], Duration::ZERO));
        let second: Arc<dyn ByteSource> =
            Arc::new(MemorySource::new(32, 1, vec![4; 1024], Duration::ZERO));

        runtime.submit(request(1, Arc::clone(&first), 1024)?)?;
        assert!(matches!(
            wait_for_terminal(&runtime, AnalysisRequestId(1))?,
            ProductionRuntimeEvent::Completed {
                cache_hit: false,
                ..
            }
        ));
        runtime.submit(request(2, Arc::clone(&first), 1024)?)?;
        assert!(matches!(
            wait_for_terminal(&runtime, AnalysisRequestId(2))?,
            ProductionRuntimeEvent::Completed {
                cache_hit: true,
                ..
            }
        ));
        runtime.submit(request(3, second, 1024)?)?;
        assert!(matches!(
            wait_for_terminal(&runtime, AnalysisRequestId(3))?,
            ProductionRuntimeEvent::Completed {
                cache_hit: false,
                ..
            }
        ));
        runtime.submit(request(4, first, 1024)?)?;
        assert!(matches!(
            wait_for_terminal(&runtime, AnalysisRequestId(4))?,
            ProductionRuntimeEvent::Completed {
                cache_hit: false,
                ..
            }
        ));
        assert_eq!(runtime.cache_stats().entries, 1);
        Ok(())
    }

    #[test]
    fn artifact_digest_ignores_runtime_source_ids() -> Result<(), Box<dyn Error>> {
        let ranges = ByteRangeSet {
            ranges: vec![ByteRange::new(0, 3)?],
        };
        let chunks = vec![(ByteRange::new(0, 3)?, b"abc".to_vec())];
        let first = build_structure_entropy_artifact(
            SourceId(1),
            SourceGeneration(1),
            ranges.clone(),
            StructureEntropyPreset::default(),
            &chunks,
        )?;
        let second = build_structure_entropy_artifact(
            SourceId(999),
            SourceGeneration(88),
            ranges,
            StructureEntropyPreset::default(),
            &chunks,
        )?;
        assert_eq!(first.artifact_digest, second.artifact_digest);
        Ok(())
    }
}
