# Angzarr Kernel Event System

**Architecture:** Event Sourcing with Caching
**Status:** Design Phase
**Target:** Phase 3-4 of Migration

---

## Core Principle

**"All kernel state changes are events. Events are the source of truth. Cache derived state for performance."**

---

## Overview

Traditional kernels use imperative state mutations. Angzarr uses **event sourcing**: all state changes are recorded as immutable events, and current state is derived by replaying events. Frequently accessed state is cached for performance.

```
┌──────────────────────────────────────────────────────────┐
│                   Event Stream (Source of Truth)          │
│  [Event1] → [Event2] → [Event3] → [Event4] → ...        │
└────────────┬─────────────────────────────────────────────┘
             │
             │ Replay/Derive
             ▼
┌──────────────────────────────────────────────────────────┐
│                   Cached State (Performance)              │
│  Current state derived from events, cached in memory      │
└──────────────────────────────────────────────────────────┘
```

---

## Problem Analysis

### Linux Approach

**Traditional State Mutation:**
```c
// Direct state mutation
struct task_struct *p = current;
p->state = TASK_RUNNING;
p->prio = new_prio;
p->policy = SCHED_FIFO;

// No history, can't replay
// Debugging: "how did we get here?"
// Answer: Unknown, state was mutated
```

**Problems:**
- No audit trail of state changes
- Can't replay to debug
- Race conditions hard to diagnose
- No built-in undo capability
- Lost context: why did state change?

**Linux's Partial Solutions:**
- `ftrace` - After-the-fact tracing
- `perf` - Performance counters
- `bpf` - Programmable tracing
- All added later, not fundamental

**BSD Approach:**

FreeBSD:
- Similar to Linux: direct mutation
- `dtrace` for after-the-fact observation
- `ktr` (Kernel Tracing) facility

OpenBSD:
- Simpler, less tracing infrastructure
- Focus on simplicity over observability
- Still direct state mutation

**Common Problem:**
Both Linux and BSD:
1. Debugging requires recreating conditions
2. No inherent state history
3. Race conditions leave no trace
4. Can't "rewind" to see what happened

---

## Angzarr Design: Event Sourcing + Caching

### Architecture Layers

```
┌────────────────────────────────────────────────────────────┐
│  Layer 4: Linux-Compatible API (Adapter)                   │
│  • sync_call() bridges to async/event system               │
│  • Maintains Linux ABI                                     │
└────────────┬───────────────────────────────────────────────┘
             │
┌────────────▼───────────────────────────────────────────────┐
│  Layer 3: Event Handlers (Async)                           │
│  • Process events asynchronously                           │
│  • Update caches                                           │
│  • Trigger side effects                                    │
└────────────┬───────────────────────────────────────────────┘
             │
┌────────────▼───────────────────────────────────────────────┐
│  Layer 2: Event Bus (Core)                                 │
│  • Append-only event log                                   │
│  • Pub/sub for event distribution                          │
│  • Event persistence (ring buffer)                         │
└────────────┬───────────────────────────────────────────────┘
             │
┌────────────▼───────────────────────────────────────────────┐
│  Layer 1: Cache Layer (Performance)                        │
│  • LRU cache for hot state                                 │
│  • Read-mostly data (RCU-like)                             │
│  • Invalidate on relevant events                           │
└────────────────────────────────────────────────────────────┘
```

### Event Types

```rust
/// Core event trait
pub trait KernelEvent: Send + Sync {
    /// Event type identifier
    fn event_type(&self) -> EventType;

    /// Timestamp when event occurred
    fn timestamp(&self) -> Timestamp;

    /// Subsystem that generated event
    fn subsystem(&self) -> Subsystem;

    /// Serialize event for persistence
    fn serialize(&self) -> &[u8];
}

/// Event type categories
#[repr(u32)]
pub enum EventType {
    // Process events
    ProcessCreated,
    ProcessTerminated,
    ProcessStateChanged,
    ProcessPriorityChanged,

    // Memory events
    PageAllocated,
    PageFreed,
    PageFault,
    MemoryMapped,

    // I/O events
    FileOpened,
    FileClosed,
    IOScheduled,
    IOCompleted,

    // Network events
    PacketReceived,
    PacketSent,
    ConnectionEstablished,
    ConnectionClosed,

    // Lock events
    LockAcquired,
    LockReleased,
    LockContended,

    // Custom subsystem events
    Custom(u32),
}
```

### Event Examples

```rust
// angzarr-event/src/events.rs

/// Process state change event
#[derive(Debug, Clone)]
pub struct ProcessStateChanged {
    pub timestamp: Timestamp,
    pub pid: Pid,
    pub old_state: TaskState,
    pub new_state: TaskState,
    pub reason: StateChangeReason,
}

impl KernelEvent for ProcessStateChanged {
    fn event_type(&self) -> EventType {
        EventType::ProcessStateChanged
    }

    fn timestamp(&self) -> Timestamp {
        self.timestamp
    }

    fn subsystem(&self) -> Subsystem {
        Subsystem::Scheduler
    }

    fn serialize(&self) -> &[u8] {
        // Efficient serialization
        unsafe {
            core::slice::from_raw_parts(
                self as *const _ as *const u8,
                core::mem::size_of::<Self>(),
            )
        }
    }
}

/// Page fault event
#[derive(Debug, Clone)]
pub struct PageFaultEvent {
    pub timestamp: Timestamp,
    pub pid: Pid,
    pub address: VirtualAddress,
    pub fault_type: FaultType,
    pub resolved: bool,
}

/// I/O completion event
#[derive(Debug, Clone)]
pub struct IOCompletedEvent {
    pub timestamp: Timestamp,
    pub device: DeviceId,
    pub operation: IOOperation,
    pub bytes: usize,
    pub latency_ns: u64,
    pub result: IOResult,
}
```

---

## Event Bus Design

### Core Structure

```rust
// angzarr-event/src/bus.rs

/// Global kernel event bus
pub struct EventBus {
    /// Ring buffer for event storage
    log: RingBuffer<Event>,

    /// Subscribers by event type
    subscribers: HashMap<EventType, Vec<EventHandler>>,

    /// Event statistics
    stats: EventStats,

    /// Lock for thread-safe access
    lock: Spinlock<()>,
}

impl EventBus {
    /// Publish an event to all subscribers
    pub fn publish<E: KernelEvent>(&self, event: E) {
        // Append to log (immutable, append-only)
        self.log.append(event.clone());

        // Notify subscribers
        if let Some(handlers) = self.subscribers.get(&event.event_type()) {
            for handler in handlers {
                handler.handle(event.clone());
            }
        }

        // Update stats
        self.stats.increment(event.event_type());
    }

    /// Subscribe to events of a specific type
    pub fn subscribe(&mut self, event_type: EventType, handler: EventHandler) {
        self.subscribers
            .entry(event_type)
            .or_insert_with(Vec::new)
            .push(handler);
    }

    /// Replay events from timestamp
    pub fn replay_from(&self, start: Timestamp) -> EventIterator {
        self.log.iter_from(start)
    }

    /// Query events matching criteria
    pub fn query(&self, filter: EventFilter) -> Vec<Event> {
        self.log
            .iter()
            .filter(|e| filter.matches(e))
            .collect()
    }
}

/// Event handler callback
pub type EventHandler = Arc<dyn Fn(Event) + Send + Sync>;

/// Ring buffer for event storage
pub struct RingBuffer<T> {
    buffer: Vec<Option<T>>,
    write_pos: AtomicUsize,
    read_pos: AtomicUsize,
    capacity: usize,
}
```

---

## Caching Strategy

### Cache Design

```rust
// angzarr-event/src/cache.rs

/// Cached state derived from events
pub struct StateCache<K, V> {
    /// LRU cache for hot entries
    cache: LruCache<K, CachedEntry<V>>,

    /// Event subscriptions that invalidate cache
    invalidators: Vec<EventType>,

    /// Cache statistics
    hits: AtomicU64,
    misses: AtomicU64,
}

/// Cached entry with metadata
pub struct CachedEntry<V> {
    /// Cached value
    value: V,

    /// Timestamp when cached
    cached_at: Timestamp,

    /// Last event that updated this
    event_id: EventId,

    /// Access count
    access_count: AtomicU32,
}

impl<K, V> StateCache<K, V>
where
    K: Hash + Eq,
    V: Clone,
{
    /// Get from cache or derive from events
    pub fn get_or_derive<F>(&mut self, key: &K, derive: F) -> V
    where
        F: FnOnce() -> V,
    {
        if let Some(entry) = self.cache.get(key) {
            self.hits.fetch_add(1, Ordering::Relaxed);
            entry.value.clone()
        } else {
            self.misses.fetch_add(1, Ordering::Relaxed);
            let value = derive();
            self.cache.put(key.clone(), CachedEntry {
                value: value.clone(),
                cached_at: Timestamp::now(),
                event_id: EventId::current(),
                access_count: AtomicU32::new(1),
            });
            value
        }
    }

    /// Invalidate cache entries affected by event
    pub fn handle_event(&mut self, event: &Event) {
        if self.invalidators.contains(&event.event_type()) {
            // Invalidate affected entries
            self.cache.invalidate_matching(|entry| {
                entry.event_id < event.id()
            });
        }
    }

    /// Cache hit rate
    pub fn hit_rate(&self) -> f64 {
        let hits = self.hits.load(Ordering::Relaxed);
        let misses = self.misses.load(Ordering::Relaxed);
        if hits + misses == 0 {
            return 0.0;
        }
        hits as f64 / (hits + misses) as f64
    }
}
```

### Example: Process State Caching

```rust
// angzarr-sched/src/cache.rs

/// Cache for process states
pub struct ProcessStateCache {
    cache: StateCache<Pid, TaskState>,
}

impl ProcessStateCache {
    pub fn new() -> Self {
        let mut cache = StateCache::new(1024);  // 1024 entries

        // Invalidate on state change events
        cache.add_invalidator(EventType::ProcessStateChanged);
        cache.add_invalidator(EventType::ProcessTerminated);

        Self { cache }
    }

    /// Get current process state
    pub fn get_state(&mut self, pid: Pid) -> Option<TaskState> {
        self.cache.get_or_derive(&pid, || {
            // Derive from events if not in cache
            let events = EVENT_BUS.query(EventFilter {
                event_type: Some(EventType::ProcessStateChanged),
                pid: Some(pid),
            });

            // Replay events to get current state
            events.iter().fold(None, |_state, event| {
                if let Event::ProcessStateChanged(e) = event {
                    Some(e.new_state)
                } else {
                    None
                }
            })
        })
    }
}

// Subscribe to events for automatic cache updates
fn init_cache_updater() {
    EVENT_BUS.subscribe(
        EventType::ProcessStateChanged,
        Arc::new(|event| {
            if let Event::ProcessStateChanged(e) = event {
                PROCESS_CACHE.handle_event(&event);
            }
        }),
    );
}
```

---

## Async/Sync Bridge

### Synchronous Linux API

```rust
// angzarr-linux-compat/src/sched.rs

/// Set process state (Linux-compatible)
#[no_mangle]
pub unsafe extern "C" fn set_task_state(task: *mut task_struct, state: c_int) {
    if task.is_null() {
        return;
    }

    let pid = (*task).pid;
    let old_state = (*task).state;
    let new_state = state;

    // Publish event asynchronously
    EVENT_BUS.publish(ProcessStateChanged {
        timestamp: Timestamp::now(),
        pid: Pid(pid),
        old_state: TaskState::from(old_state),
        new_state: TaskState::from(new_state),
        reason: StateChangeReason::Explicit,
    });

    // Update C struct immediately (for compatibility)
    (*task).state = new_state;

    // Cache will be updated asynchronously by event handler
}
```

### Asynchronous Angzarr API

```rust
// angzarr-sched/src/scheduler.rs

/// Set process state (Angzarr native, async)
pub async fn set_process_state(
    pid: Pid,
    new_state: TaskState,
    reason: StateChangeReason,
) -> KernelResult<()> {
    // Get current state from cache
    let old_state = PROCESS_CACHE.get_state(pid)
        .ok_or(KernelError::ESRCH)?;

    // Publish event
    EVENT_BUS.publish(ProcessStateChanged {
        timestamp: Timestamp::now(),
        pid,
        old_state,
        new_state,
        reason,
    }).await;

    // Event handlers will update caches asynchronously
    Ok(())
}
```

---

## Event Persistence

### Ring Buffer Implementation

```rust
// angzarr-event/src/ring_buffer.rs

const EVENT_LOG_SIZE: usize = 1_000_000;  // 1M events

pub struct EventLog {
    /// Circular buffer of events
    events: [Option<Event>; EVENT_LOG_SIZE],

    /// Write position (monotonic)
    write_pos: AtomicUsize,

    /// Oldest event position
    oldest_pos: AtomicUsize,
}

impl EventLog {
    /// Append event to log
    pub fn append(&self, event: Event) {
        let pos = self.write_pos.fetch_add(1, Ordering::SeqCst);
        let index = pos % EVENT_LOG_SIZE;

        // Write event
        unsafe {
            let slot = &self.events[index] as *const _ as *mut Option<Event>;
            *slot = Some(event);
        }

        // Update oldest if we wrapped
        if pos >= EVENT_LOG_SIZE {
            self.oldest_pos.store(pos - EVENT_LOG_SIZE + 1, Ordering::Release);
        }
    }

    /// Iterate events from timestamp
    pub fn iter_from(&self, start: Timestamp) -> EventIterator {
        // Binary search for first event >= start
        // Then iterate forward
        EventIterator {
            log: self,
            current_pos: self.find_first(start),
        }
    }

    /// Get event at position
    pub fn get(&self, pos: usize) -> Option<&Event> {
        let index = pos % EVENT_LOG_SIZE;
        self.events[index].as_ref()
    }
}
```

---

## Benefits

### Compared to Linux/BSD

| Feature | Linux/BSD | Angzarr Event Sourcing |
|---------|-----------|------------------------|
| State history | ❌ No | ✅ Full event log |
| Debugging | ⚠️ After-the-fact tracing | ✅ Replay any sequence |
| Audit trail | ⚠️ Optional (auditd) | ✅ Built-in |
| Race diagnosis | ❌ Hard | ✅ Event ordering preserved |
| Undo capability | ❌ No | ✅ Replay to previous state |
| Performance | ✅ Direct mutation | ✅ Cached derived state |
| Observability | ⚠️ Add-on tools | ✅ Fundamental |

### Specific Advantages

1. **Debugging**
   - Replay exact event sequence that led to bug
   - No need to recreate conditions
   - Event log shows causality

2. **Auditing**
   - Every state change recorded
   - Compliance (e.g., security audits) built-in
   - Tamper-evident (append-only)

3. **Testing**
   - Record production events, replay in test
   - Deterministic replay for race conditions
   - Property-based testing on event streams

4. **Performance**
   - Hot state cached (LRU)
   - Read-mostly data uses RCU-like patterns
   - Cache invalidation precise (event-driven)

5. **Flexibility**
   - Can change derived state format without changing events
   - Multiple views of same events
   - Time-travel debugging

---

## Implementation Plan

### Phase 1: Foundation (Phase 3 in Migration)

- [ ] Design Event trait and core types
- [ ] Implement ring buffer for event storage
- [ ] Basic EventBus with pub/sub
- [ ] Simple caching layer
- [ ] Tests for event persistence

### Phase 2: Integration (Phase 4 in Migration)

- [ ] Integrate with scheduler
- [ ] Process state events and caching
- [ ] Memory subsystem events
- [ ] Async/sync bridge for Linux API
- [ ] Performance benchmarks

### Phase 3: Advanced Features (Phase 5-6)

- [ ] Event compression for old events
- [ ] Persistent storage (optional)
- [ ] Event replay tools
- [ ] Query language for events
- [ ] Distributed tracing integration

---

## Performance Considerations

### Event Overhead

**Write Path:**
- Event creation: ~50ns (stack allocation)
- Ring buffer append: ~20ns (atomic increment + write)
- Pub/sub notify: ~10ns per subscriber
- **Total: ~80ns + subscribers**

**Read Path (Cached):**
- Cache hit: ~5ns (hash lookup)
- Cache miss: replay events (~1μs for 100 events)
- **Hot path: ~5ns (cached)**

**Memory:**
- Event size: 64-128 bytes typical
- Ring buffer: 1M events × 128 bytes = 128MB
- Cache: LRU with configurable size
- **Total: <200MB for full system**

### Comparison to Direct Mutation

| Operation | Direct Mutation | Event Sourcing (Cached) | Overhead |
|-----------|-----------------|-------------------------|----------|
| State write | ~5ns | ~80ns | +75ns |
| State read (hot) | ~5ns | ~5ns | 0ns |
| State read (cold) | ~5ns | ~1μs | +995ns |
| Debug/audit | N/A | ~5μs (query) | - |

**Conclusion:** Small write overhead (~75ns) acceptable for kernel robustness and observability benefits.

---

## Example Use Cases

### 1. Debugging Deadlock

```rust
// Query all lock events for process
let events = EVENT_BUS.query(EventFilter {
    pid: Some(Pid(1234)),
    event_types: vec![
        EventType::LockAcquired,
        EventType::LockReleased,
        EventType::LockContended,
    ],
    time_range: Some(TimeRange::last_seconds(10)),
});

// Replay to see lock acquisition order
for event in events {
    match event {
        Event::LockAcquired(e) => println!("Acquired {:?} at {}", e.lock_id, e.timestamp),
        Event::LockContended(e) => println!("Contended on {:?}", e.lock_id),
        _ => {}
    }
}
```

### 2. Performance Analysis

```rust
// Find slow I/O operations
let slow_io = EVENT_BUS.query(EventFilter {
    event_type: Some(EventType::IOCompleted),
    filter: Box::new(|e| {
        if let Event::IOCompleted(io) = e {
            io.latency_ns > 1_000_000  // > 1ms
        } else {
            false
        }
    }),
});

// Analyze patterns
for event in slow_io {
    println!("Slow I/O: {:?}", event);
}
```

### 3. Security Auditing

```rust
// Track all permission changes
EVENT_BUS.subscribe(
    EventType::PermissionChanged,
    Arc::new(|event| {
        if let Event::PermissionChanged(e) = event {
            audit_log!("Permission changed: {:?} → {:?} for {:?}",
                e.old_perms, e.new_perms, e.file);
        }
    }),
);
```

---

## Comparison Table

| Aspect | Linux ftrace | BSD dtrace | Angzarr Events |
|--------|--------------|------------|----------------|
| **Architecture** | Tracepoints | Probes | Event Sourcing |
| **Performance** | Low overhead | Low overhead | Cached state |
| **History** | Ring buffer | Limited | Persistent log |
| **Replay** | ❌ No | ❌ No | ✅ Yes |
| **Real-time** | ⚠️ Optional | ⚠️ Optional | ✅ Built-in |
| **Audit** | ⚠️ Separate tool | ⚠️ Separate tool | ✅ Built-in |
| **Causality** | ⚠️ Timestamps only | ⚠️ Timestamps only | ✅ Event ordering |
| **Type safety** | ❌ Text-based | ❌ Text-based | ✅ Rust types |

---

## Future Enhancements

1. **Distributed Tracing**
   - OpenTelemetry integration
   - Cross-system event correlation
   - Distributed causality

2. **Machine Learning**
   - Anomaly detection on event patterns
   - Predictive failure analysis
   - Workload classification

3. **Persistence**
   - Optional persistent event log
   - Snapshot/restore from events
   - Time-travel debugging across reboots

4. **Query Language**
   - SQL-like queries on events
   - Real-time stream processing
   - Complex event processing (CEP)

---

## References

### Event Sourcing

- Martin Fowler: "Event Sourcing" (martinfowler.com)
- Greg Young: "CQRS and Event Sourcing" (cqrs.files.wordpress.com)
- Event Store documentation (eventstore.com)

### Kernel Tracing

- Linux ftrace documentation
- BSD dtrace documentation
- eBPF/BCC documentation

### Angzarr Documentation

- `ADAPTER_LAYER.md` - How events integrate with Linux API
- `LINUX_KERNEL_LESSONS.md` - Event-driven architecture decision
- `NAMING_STRATEGY.md` - Event naming conventions

---

## Summary

Angzarr's event sourcing architecture provides:

1. ✅ **Complete Observability** - All state changes recorded
2. ✅ **Replay Debugging** - Reproduce any sequence
3. ✅ **Audit Trail** - Built-in security compliance
4. ✅ **Performance** - Cached derived state
5. ✅ **Flexibility** - Multiple views from same events
6. ✅ **Robustness** - Understand how system reached any state

**Trade-off:** Small write overhead (~75ns) for massive debugging and auditing benefits.

**Golden Rule:** "Events are truth. Cache for speed. Replay for understanding."
