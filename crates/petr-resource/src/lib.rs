//! Chunk-oriented resource scheduler driven by selector words.

use petr_core::{PortHint, SelectorWord};
use thiserror::Error;

/// Result type for resource scheduling.
pub type ResourceResult<T> = Result<T, ResourceError>;

/// Resource errors.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ResourceError {
    /// The pool does not have enough chunks.
    #[error("not enough chunks for {kind:?}: requested {requested}, available {available}")]
    NotEnough {
        kind: ResourceKind,
        requested: u32,
        available: u32,
    },

    /// A resource handle failed validation.
    #[error("invalid resource handle")]
    InvalidHandle,

    /// The resource handle belongs to a different pool.
    #[error("resource handle belongs to pool {found}, expected pool {expected}")]
    WrongPool { expected: u16, found: u16 },

    /// The resource handle points at a stale generation.
    #[error(
        "stale resource handle at index {index}: handle generation {handle_generation}, live generation {live_generation}"
    )]
    StaleHandle {
        index: u32,
        handle_generation: u32,
        live_generation: u32,
    },
}

/// Major resource type; maps naturally into the 4-bit trait field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceKind {
    /// RAM-backed chunk.
    Ram = 1,
    /// Storage-backed chunk.
    Storage = 2,
    /// File patch/change request chunk.
    FilePatch = 3,
    /// Network/request chunk.
    Network = 4,
}

/// Chunk class; maps naturally into the 4-bit implementation field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChunkClass {
    /// 4 KiB chunk.
    K4 = 1,
    /// 64 KiB chunk.
    K64 = 2,
    /// 2 MiB chunk.
    M2 = 3,
}

impl ChunkClass {
    /// Size in bytes.
    pub const fn bytes(self) -> usize {
        match self {
            Self::K4 => 4 * 1024,
            Self::K64 => 64 * 1024,
            Self::M2 => 2 * 1024 * 1024,
        }
    }
}

/// Resource operation; maps into the 8-bit function selector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceOp {
    /// Allocate chunks.
    Allocate = 1,
    /// Release chunks.
    Release = 2,
    /// Grow allocation.
    Grow = 3,
    /// Shrink allocation.
    Shrink = 4,
    /// Commit RAM chunk to storage/file patch.
    Commit = 5,
}

/// A selector describing a resource command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResourceSelector {
    /// Resource kind.
    pub kind: ResourceKind,
    /// Chunk class.
    pub class: ChunkClass,
    /// Operation.
    pub op: ResourceOp,
    /// Software scheduling hint.
    pub hint: PortHint,
}

impl ResourceSelector {
    /// Pack this resource selector into a 4/4/8 selector word.
    pub const fn selector_word(self) -> SelectorWord {
        SelectorWord::new(self.kind as u8, self.class as u8, self.op as u8)
    }
}

/// A simple pool of consumable chunks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourcePool {
    /// Available RAM chunks.
    pub ram_chunks: u32,
    /// Available storage chunks.
    pub storage_chunks: u32,
    /// Pending file patch chunks.
    pub file_patch_chunks: u32,
    /// Pending network chunks.
    pub network_chunks: u32,
}

impl ResourcePool {
    /// Create a resource pool.
    pub const fn new(ram_chunks: u32, storage_chunks: u32) -> Self {
        Self {
            ram_chunks,
            storage_chunks,
            file_patch_chunks: 0,
            network_chunks: 0,
        }
    }
}

/// Event produced by the scheduler.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceEvent {
    /// Selector that executed.
    pub selector: SelectorWord,
    /// Human-readable description.
    pub detail: String,
}

/// Chunk scheduler. It models the “pointer dispatches resource stuff” idea.
#[derive(Debug, Default)]
pub struct ResourceScheduler;

impl ResourceScheduler {
    /// Dispatch a resource operation.
    pub fn dispatch(
        pool: &mut ResourcePool,
        selector: ResourceSelector,
        chunks: u32,
    ) -> ResourceResult<ResourceEvent> {
        match (selector.kind, selector.op) {
            (ResourceKind::Ram, ResourceOp::Allocate | ResourceOp::Grow) => {
                take(&mut pool.ram_chunks, ResourceKind::Ram, chunks)?;
                Ok(event(
                    selector,
                    format!(
                        "allocated {chunks} RAM chunks of {} bytes",
                        selector.class.bytes()
                    ),
                ))
            }
            (ResourceKind::Ram, ResourceOp::Release | ResourceOp::Shrink) => {
                pool.ram_chunks = pool.ram_chunks.saturating_add(chunks);
                Ok(event(selector, format!("released {chunks} RAM chunks")))
            }
            (ResourceKind::Storage, ResourceOp::Allocate | ResourceOp::Grow) => {
                take(&mut pool.storage_chunks, ResourceKind::Storage, chunks)?;
                Ok(event(
                    selector,
                    format!(
                        "allocated {chunks} storage chunks of {} bytes",
                        selector.class.bytes()
                    ),
                ))
            }
            (ResourceKind::Storage, ResourceOp::Release | ResourceOp::Shrink) => {
                pool.storage_chunks = pool.storage_chunks.saturating_add(chunks);
                Ok(event(selector, format!("released {chunks} storage chunks")))
            }
            (ResourceKind::FilePatch, ResourceOp::Commit) => {
                pool.file_patch_chunks = pool.file_patch_chunks.saturating_add(chunks);
                Ok(event(
                    selector,
                    format!("committed {chunks} file patch chunks"),
                ))
            }
            (ResourceKind::Network, ResourceOp::Commit) => {
                pool.network_chunks = pool.network_chunks.saturating_add(chunks);
                Ok(event(selector, format!("queued {chunks} network chunks")))
            }
            _ => Ok(event(
                selector,
                "operation accepted as no-op for this resource kind".into(),
            )),
        }
    }
}

fn take(slot: &mut u32, kind: ResourceKind, requested: u32) -> ResourceResult<()> {
    if *slot < requested {
        return Err(ResourceError::NotEnough {
            kind,
            requested,
            available: *slot,
        });
    }
    *slot -= requested;
    Ok(())
}

fn event(selector: ResourceSelector, detail: String) -> ResourceEvent {
    ResourceEvent {
        selector: selector.selector_word(),
        detail,
    }
}

use core::{marker::PhantomData, num::NonZeroUsize};
use petr_core::Key;

/// Opaque handle to a resource allocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResourceHandle<const LOW_BITS: usize = 4> {
    pool_id: u16,
    index: u32,
    generation: u32,
    selector: SelectorWord,
    chunks: u32,
    check: NonZeroUsize,
    _marker: PhantomData<[(); LOW_BITS]>,
}

impl<const LOW_BITS: usize> ResourceHandle<LOW_BITS> {
    /// Pool id encoded in this handle.
    pub const fn pool_id(self) -> u16 {
        self.pool_id
    }
    /// Slot index.
    pub const fn index(self) -> u32 {
        self.index
    }
    /// Slot generation.
    pub const fn generation(self) -> u32 {
        self.generation
    }
    /// Resource selector.
    pub const fn selector(self) -> SelectorWord {
        self.selector
    }
    /// Number of chunks represented by this handle.
    pub const fn chunks(self) -> u32 {
        self.chunks
    }
    /// Masked verification word.
    pub const fn check_word(self) -> NonZeroUsize {
        self.check
    }
    /// Test helper for forged handles.
    pub const fn with_generation(self, generation: u32) -> Self {
        Self { generation, ..self }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ResourceAllocation<const LOW_BITS: usize> {
    kind: ResourceKind,
    class: ChunkClass,
    chunks: u32,
    generation: u32,
    selector: SelectorWord,
    check: NonZeroUsize,
    committed: bool,
}

/// A generation-checked, chunk-oriented resource manager.
#[derive(Debug)]
pub struct ResourceManager<const LOW_BITS: usize = 4> {
    key: Key<LOW_BITS>,
    pool_id: u16,
    pool: ResourcePool,
    allocations: Vec<Option<ResourceAllocation<LOW_BITS>>>,
    free: Vec<usize>,
    next_generation: u32,
}

impl<const LOW_BITS: usize> ResourceManager<LOW_BITS> {
    /// Create a manager with deterministic keying.
    pub fn with_key(pool_id: u16, pool: ResourcePool, key: Key<LOW_BITS>) -> Self {
        Self {
            key,
            pool_id,
            pool,
            allocations: Vec::new(),
            free: Vec::new(),
            next_generation: 1,
        }
    }

    /// Create a manager with an OS-random key when available.
    pub fn new_randomized(pool_id: u16, pool: ResourcePool) -> Self {
        Self::with_key(pool_id, pool, Key::random())
    }

    /// Available pool snapshot.
    pub fn pool(&self) -> &ResourcePool {
        &self.pool
    }

    /// Allocate chunks and return a generation-checked handle.
    pub fn allocate(
        &mut self,
        selector: ResourceSelector,
        chunks: u32,
    ) -> ResourceResult<ResourceHandle<LOW_BITS>> {
        ResourceScheduler::dispatch(&mut self.pool, selector, chunks)?;
        let generation = self.next_generation;
        self.next_generation = self.next_generation.wrapping_add(1).max(1);
        let selector_word = selector.selector_word();
        let index = if let Some(index) = self.free.pop() {
            index
        } else {
            self.allocations.push(None);
            self.allocations.len() - 1
        };
        let tag = (selector.op as usize) & petr_core::LowTagLayout::<LOW_BITS>::LOW_MASK;
        let pseudo_addr = self.pseudo_addr(index, generation, selector_word, chunks);
        let check = self
            .key
            .mask_addr(pseudo_addr, tag)
            .map_err(|_| ResourceError::InvalidHandle)?;
        let allocation = ResourceAllocation {
            kind: selector.kind,
            class: selector.class,
            chunks,
            generation,
            selector: selector_word,
            check,
            committed: false,
        };
        self.allocations[index] = Some(allocation.clone());
        Ok(ResourceHandle {
            pool_id: self.pool_id,
            index: index as u32,
            generation,
            selector: selector_word,
            chunks,
            check,
            _marker: PhantomData,
        })
    }

    /// Validate a handle and return its slot index.
    pub fn validate(&self, handle: ResourceHandle<LOW_BITS>) -> ResourceResult<usize> {
        if handle.pool_id != self.pool_id {
            return Err(ResourceError::WrongPool {
                expected: self.pool_id,
                found: handle.pool_id,
            });
        }
        let index = handle.index as usize;
        let Some(slot) = self.allocations.get(index) else {
            return Err(ResourceError::InvalidHandle);
        };
        let Some(allocation) = slot.as_ref() else {
            return Err(ResourceError::InvalidHandle);
        };
        if allocation.generation != handle.generation {
            return Err(ResourceError::StaleHandle {
                index: handle.index,
                handle_generation: handle.generation,
                live_generation: allocation.generation,
            });
        }
        if allocation.check != handle.check
            || allocation.selector != handle.selector
            || allocation.chunks != handle.chunks
        {
            return Err(ResourceError::InvalidHandle);
        }
        let expected = self.pseudo_addr(index, handle.generation, handle.selector, handle.chunks);
        let revealed = self
            .key
            .reveal_addr(handle.check)
            .map_err(|_| ResourceError::InvalidHandle)?;
        if revealed != expected {
            return Err(ResourceError::InvalidHandle);
        }
        Ok(index)
    }

    /// Mark an allocation as committed, useful for file patch / storage handoff flows.
    pub fn commit(&mut self, handle: ResourceHandle<LOW_BITS>) -> ResourceResult<ResourceEvent> {
        let index = self.validate(handle)?;
        let allocation = self.allocations[index]
            .as_mut()
            .expect("validated allocation");
        allocation.committed = true;
        Ok(ResourceEvent {
            selector: allocation.selector,
            detail: format!(
                "committed {} {:?} {:?} chunks",
                allocation.chunks, allocation.kind, allocation.class
            ),
        })
    }

    /// Release an allocation and return its chunks to the corresponding pool.
    pub fn release(&mut self, handle: ResourceHandle<LOW_BITS>) -> ResourceResult<ResourceEvent> {
        let index = self.validate(handle)?;
        let allocation = self.allocations[index]
            .take()
            .expect("validated allocation");
        self.free.push(index);
        match allocation.kind {
            ResourceKind::Ram => {
                self.pool.ram_chunks = self.pool.ram_chunks.saturating_add(allocation.chunks)
            }
            ResourceKind::Storage => {
                self.pool.storage_chunks =
                    self.pool.storage_chunks.saturating_add(allocation.chunks)
            }
            ResourceKind::FilePatch => {
                self.pool.file_patch_chunks = self
                    .pool
                    .file_patch_chunks
                    .saturating_sub(allocation.chunks)
            }
            ResourceKind::Network => {
                self.pool.network_chunks =
                    self.pool.network_chunks.saturating_sub(allocation.chunks)
            }
        }
        Ok(ResourceEvent {
            selector: allocation.selector,
            detail: format!(
                "released {} {:?} chunks",
                allocation.chunks, allocation.kind
            ),
        })
    }

    fn pseudo_addr(
        &self,
        index: usize,
        generation: u32,
        selector: SelectorWord,
        chunks: u32,
    ) -> usize {
        let raw = ((self.pool_id as usize) << 48)
            ^ ((index + 1) << 24)
            ^ ((generation as usize) << 8)
            ^ selector.raw() as usize
            ^ ((chunks as usize) << 16);
        let clean = raw & petr_core::LowTagLayout::<LOW_BITS>::CLEAN_MASK;
        if clean == 0 {
            petr_core::LowTagLayout::<LOW_BITS>::ALIGN << 10
        } else {
            clean
        }
    }
}
