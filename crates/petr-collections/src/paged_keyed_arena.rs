use core::{marker::PhantomData, num::NonZeroUsize};

use petr_core::{Key, LowTagLayout, ProphecyBits, SelectorWord};

use crate::{KeyedArenaError, KeyedArenaResult};

/// Opaque handle for one live entry in a [`PagedKeyedArena`].
///
/// The handle carries a flat slot index so it remains compact and serializable,
/// while the arena maps that index to a stable boxed page and slot. As with
/// [`crate::KeyedArenaHandle`], this is validation metadata rather than a raw
/// pointer authority.
#[derive(Debug, PartialEq, Eq)]
pub struct PagedKeyedArenaHandle<T, const LOW_BITS: usize, const PAGE_SIZE: usize> {
    index: u32,
    generation: u32,
    check: NonZeroUsize,
    selector: SelectorWord,
    prophecy: ProphecyBits,
    _marker: PhantomData<fn() -> T>,
}

impl<T, const LOW_BITS: usize, const PAGE_SIZE: usize> Copy
    for PagedKeyedArenaHandle<T, LOW_BITS, PAGE_SIZE>
{
}

impl<T, const LOW_BITS: usize, const PAGE_SIZE: usize> Clone
    for PagedKeyedArenaHandle<T, LOW_BITS, PAGE_SIZE>
{
    fn clone(&self) -> Self {
        *self
    }
}

/// Opaque capability for one stable physical page in a [`PagedKeyedArena`].
///
/// A page handle is validated once before a page kernel may traverse its live
/// entries. Page generations advance when retained pages are reset, so a handle
/// from a previous workspace execution cannot lease the recycled page.
#[derive(Debug, PartialEq, Eq)]
pub struct PagedKeyedArenaPageHandle<T, const LOW_BITS: usize, const PAGE_SIZE: usize> {
    page_index: u32,
    generation: u32,
    check: NonZeroUsize,
    _marker: PhantomData<fn() -> T>,
}

impl<T, const LOW_BITS: usize, const PAGE_SIZE: usize> Copy
    for PagedKeyedArenaPageHandle<T, LOW_BITS, PAGE_SIZE>
{
}

impl<T, const LOW_BITS: usize, const PAGE_SIZE: usize> Clone
    for PagedKeyedArenaPageHandle<T, LOW_BITS, PAGE_SIZE>
{
    fn clone(&self) -> Self {
        *self
    }
}

impl<T, const LOW_BITS: usize, const PAGE_SIZE: usize>
    PagedKeyedArenaPageHandle<T, LOW_BITS, PAGE_SIZE>
{
    /// Physical page index carried by this capability.
    pub const fn page_index(self) -> usize {
        self.page_index as usize
    }

    /// Page generation carried by this capability.
    pub const fn generation(self) -> u32 {
        self.generation
    }

    /// Masked keyed page-address check word.
    pub const fn check_word(self) -> NonZeroUsize {
        self.check
    }

    /// Build a forged/test capability with another generation.
    pub const fn with_generation(self, generation: u32) -> Self {
        Self { generation, ..self }
    }

    /// Build a forged/test capability with another check word.
    pub const fn with_check_word(self, check: NonZeroUsize) -> Self {
        Self { check, ..self }
    }
}

/// Metadata action returned by an exclusive page kernel for one live entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PagedEntryAction {
    /// Keep the entry's keyed tag, selector, and prophecy metadata unchanged.
    Preserve,
    /// Apply one keyed metadata transition after the value kernel returns.
    Transition {
        /// New low lifecycle tag.
        tag: usize,
        /// New selector word.
        selector: SelectorWord,
        /// New prophecy metadata.
        prophecy: ProphecyBits,
    },
}

impl<T, const LOW_BITS: usize, const PAGE_SIZE: usize>
    PagedKeyedArenaHandle<T, LOW_BITS, PAGE_SIZE>
{
    /// Flat slot index carried by this handle.
    pub const fn index(self) -> u32 {
        self.index
    }

    /// Physical page index derived from the flat slot index.
    pub const fn page_index(self) -> usize {
        self.index as usize / PAGE_SIZE
    }

    /// Slot index within the physical page.
    pub const fn slot_index(self) -> usize {
        self.index as usize % PAGE_SIZE
    }

    /// Generation carried by this handle.
    pub const fn generation(self) -> u32 {
        self.generation
    }

    /// Masked nonzero verification word. This is not a raw pointer authority.
    pub const fn check_word(self) -> NonZeroUsize {
        self.check
    }

    /// Trait/implementation/function selector attached to this handle.
    pub const fn selector(self) -> SelectorWord {
        self.selector
    }

    /// Prophecy/hint bits attached to this handle.
    pub const fn prophecy(self) -> ProphecyBits {
        self.prophecy
    }

    /// Low state tag visible in the keyed check word.
    pub const fn tag(self) -> usize {
        self.check.get() & LowTagLayout::<LOW_BITS>::LOW_MASK
    }

    /// Build a forged/test handle by changing only the check word.
    pub const fn with_check_word(self, check: NonZeroUsize) -> Self {
        Self { check, ..self }
    }

    /// Build a forged/test handle by changing only the generation.
    pub const fn with_generation(self, generation: u32) -> Self {
        Self { generation, ..self }
    }

    /// Build a forged/test handle by changing only the selector.
    pub const fn with_selector(self, selector: SelectorWord) -> Self {
        Self { selector, ..self }
    }

    /// Build a forged/test handle by changing only the prophecy bits.
    pub const fn with_prophecy(self, prophecy: ProphecyBits) -> Self {
        Self { prophecy, ..self }
    }

    /// Rebuild a handle from opaque validation parts.
    ///
    /// The owning arena still has to validate every field before granting access.
    pub const fn from_validation_parts(
        index: u32,
        generation: u32,
        check: NonZeroUsize,
        selector: SelectorWord,
        prophecy: ProphecyBits,
    ) -> Self {
        Self {
            index,
            generation,
            check,
            selector,
            prophecy,
            _marker: PhantomData,
        }
    }
}

#[derive(Debug)]
struct PagedArenaEntry<T> {
    value: T,
    generation: u32,
    check: NonZeroUsize,
    selector: SelectorWord,
    prophecy: ProphecyBits,
}

impl<T> PagedArenaEntry<T> {
    fn handle<const LOW_BITS: usize, const PAGE_SIZE: usize>(
        &self,
        index: usize,
    ) -> PagedKeyedArenaHandle<T, LOW_BITS, PAGE_SIZE> {
        PagedKeyedArenaHandle {
            index: index as u32,
            generation: self.generation,
            check: self.check,
            selector: self.selector,
            prophecy: self.prophecy,
            _marker: PhantomData,
        }
    }
}

#[repr(align(16))]
#[derive(Debug)]
struct ArenaPage<T, const PAGE_SIZE: usize> {
    slots: [Option<PagedArenaEntry<T>>; PAGE_SIZE],
    live: usize,
    generation: u32,
    check: NonZeroUsize,
}

impl<T, const PAGE_SIZE: usize> ArenaPage<T, PAGE_SIZE> {
    fn new(generation: u32) -> Self {
        Self {
            slots: core::array::from_fn(|_| None),
            live: 0,
            generation,
            check: NonZeroUsize::new(1).expect("one is nonzero"),
        }
    }

    fn handle<const LOW_BITS: usize>(
        &self,
        page_index: usize,
    ) -> PagedKeyedArenaPageHandle<T, LOW_BITS, PAGE_SIZE> {
        PagedKeyedArenaPageHandle {
            page_index: page_index as u32,
            generation: self.generation,
            check: self.check,
            _marker: PhantomData,
        }
    }
}

/// A provenance-preserving keyed arena backed by stable, fixed-size pages.
///
/// Each page is separately boxed. Growing the outer `Vec` moves only box
/// handles, never the page allocations, so addresses of live `T` values remain
/// stable. A flat handle index maps to `(page, slot)` without storing raw
/// addresses outside the owning arena.
///
/// Compared with [`crate::KeyedArena`], this form amortizes allocation cost over
/// `PAGE_SIZE` entries and improves locality for record-oriented runtimes.
#[derive(Debug)]
pub struct PagedKeyedArena<T, const LOW_BITS: usize, const PAGE_SIZE: usize> {
    key: Key<LOW_BITS>,
    pages: Vec<Box<ArenaPage<T, PAGE_SIZE>>>,
    free: Vec<usize>,
    live: usize,
    next_slot: usize,
    next_generation: u32,
    next_page_generation: u32,
}

impl<T, const LOW_BITS: usize, const PAGE_SIZE: usize> Default
    for PagedKeyedArena<T, LOW_BITS, PAGE_SIZE>
{
    fn default() -> Self {
        Self::new_randomized()
    }
}

impl<T, const LOW_BITS: usize, const PAGE_SIZE: usize> PagedKeyedArena<T, LOW_BITS, PAGE_SIZE> {
    /// Create an arena with an OS-random key when available.
    pub fn new_randomized() -> Self {
        Self::with_key(Key::random())
    }

    /// Create an arena with a deterministic key, mostly useful for tests.
    pub fn with_key(key: Key<LOW_BITS>) -> Self {
        assert!(PAGE_SIZE != 0, "PagedKeyedArena PAGE_SIZE must be nonzero");
        Self {
            key,
            pages: Vec::new(),
            free: Vec::new(),
            live: 0,
            next_slot: 0,
            next_generation: 1,
            next_page_generation: 1,
        }
    }

    /// Return the arena key value.
    pub const fn key(&self) -> Key<LOW_BITS> {
        self.key
    }

    /// Number of live entries.
    pub const fn len(&self) -> usize {
        self.live
    }

    /// Return true when the arena has no live entries.
    pub const fn is_empty(&self) -> bool {
        self.live == 0
    }

    /// Number of allocated physical pages.
    pub const fn page_count(&self) -> usize {
        self.pages.len()
    }

    /// Number of entries each page can hold.
    pub const fn page_size(&self) -> usize {
        PAGE_SIZE
    }

    /// Number of slot indexes that have been issued at least once.
    pub const fn slot_capacity(&self) -> usize {
        self.next_slot
    }

    /// Total slots available in already allocated pages.
    pub const fn allocated_slot_capacity(&self) -> usize {
        self.pages.len() * PAGE_SIZE
    }

    /// Number of vacant reusable slots.
    pub const fn free_len(&self) -> usize {
        self.free.len()
    }

    /// Return the current capability for one allocated physical page.
    pub fn page_handle_at(
        &self,
        page_index: usize,
    ) -> Option<PagedKeyedArenaPageHandle<T, LOW_BITS, PAGE_SIZE>> {
        self.pages
            .get(page_index)
            .map(|page| page.handle::<LOW_BITS>(page_index))
    }

    /// Return capabilities for every allocated page containing live entries.
    pub fn live_page_handles(&self) -> Vec<PagedKeyedArenaPageHandle<T, LOW_BITS, PAGE_SIZE>> {
        self.pages
            .iter()
            .enumerate()
            .filter(|(_, page)| page.live != 0)
            .map(|(page_index, page)| page.handle::<LOW_BITS>(page_index))
            .collect()
    }

    /// Refill a caller-owned scratch vector with live page capabilities.
    pub fn collect_live_page_handles(
        &self,
        output: &mut Vec<PagedKeyedArenaPageHandle<T, LOW_BITS, PAGE_SIZE>>,
    ) {
        output.clear();
        output.extend(
            self.pages
                .iter()
                .enumerate()
                .filter(|(_, page)| page.live != 0)
                .map(|(page_index, page)| page.handle::<LOW_BITS>(page_index)),
        );
    }

    /// Validate one page capability once before a page-oriented kernel executes.
    pub fn validate_page_handle(
        &self,
        handle: PagedKeyedArenaPageHandle<T, LOW_BITS, PAGE_SIZE>,
    ) -> KeyedArenaResult<usize> {
        let page_index = handle.page_index as usize;
        let Some(page) = self.pages.get(page_index) else {
            return Err(KeyedArenaError::BadIndex {
                index: handle.page_index,
            });
        };
        if page.generation != handle.generation {
            return Err(KeyedArenaError::StaleGeneration {
                index: handle.page_index,
                handle_generation: handle.generation,
                live_generation: page.generation,
            });
        }
        if page.check != handle.check {
            return Err(KeyedArenaError::CheckMismatch {
                index: handle.page_index,
            });
        }
        let revealed = self.key.reveal_addr(handle.check)?;
        let address = (&**page as *const ArenaPage<T, PAGE_SIZE>) as usize;
        if revealed != address {
            return Err(KeyedArenaError::RevealMismatch {
                index: handle.page_index,
            });
        }
        Ok(page_index)
    }

    /// Visit every live value in one validated page without per-entry handle checks.
    ///
    /// The page capability is checked once. Entry order is stable slot order, which
    /// is also insertion order for dense workspace executions.
    pub fn try_visit_page<E, F>(
        &self,
        handle: PagedKeyedArenaPageHandle<T, LOW_BITS, PAGE_SIZE>,
        mut visitor: F,
    ) -> Result<(), E>
    where
        E: From<KeyedArenaError>,
        F: FnMut(usize, &T, SelectorWord, ProphecyBits) -> Result<(), E>,
    {
        let page_index = self.validate_page_handle(handle).map_err(E::from)?;
        let page = &self.pages[page_index];
        for (slot_index, slot) in page.slots.iter().enumerate() {
            if let Some(entry) = slot.as_ref() {
                let index = page_index * PAGE_SIZE + slot_index;
                visitor(index, &entry.value, entry.selector, entry.prophecy)?;
            }
        }
        Ok(())
    }

    /// Mutate every live value in one exclusively borrowed, validated page.
    ///
    /// Validation is amortized over the whole page. The visitor may preserve
    /// metadata or request one keyed tag/selector/prophecy transition per entry.
    pub fn try_visit_page_mut<E, F>(
        &mut self,
        handle: PagedKeyedArenaPageHandle<T, LOW_BITS, PAGE_SIZE>,
        mut visitor: F,
    ) -> Result<(), E>
    where
        E: From<KeyedArenaError>,
        F: FnMut(usize, &mut T, SelectorWord, ProphecyBits) -> Result<PagedEntryAction, E>,
    {
        let page_index = self.validate_page_handle(handle).map_err(E::from)?;
        let key = self.key;
        let page = &mut self.pages[page_index];
        for (slot_index, slot) in page.slots.iter_mut().enumerate() {
            let Some(entry) = slot.as_mut() else { continue };
            let index = page_index * PAGE_SIZE + slot_index;
            match visitor(index, &mut entry.value, entry.selector, entry.prophecy)? {
                PagedEntryAction::Preserve => {}
                PagedEntryAction::Transition {
                    tag,
                    selector,
                    prophecy,
                } => {
                    LowTagLayout::<LOW_BITS>::validate_tag(tag)
                        .map_err(KeyedArenaError::from)
                        .map_err(E::from)?;
                    let address = (&entry.value as *const T) as usize;
                    entry.check = key
                        .mask_addr(address, tag)
                        .map_err(KeyedArenaError::from)
                        .map_err(E::from)?;
                    entry.selector = selector;
                    entry.prophecy = prophecy;
                }
            }
        }
        Ok(())
    }

    /// Allocate enough stable pages for at least `additional` new slots.
    pub fn reserve_entries(&mut self, additional: usize) -> KeyedArenaResult<()> {
        let required = self
            .next_slot
            .checked_add(additional)
            .ok_or(KeyedArenaError::CapacityExceeded)?;
        let required_pages = required.div_ceil(PAGE_SIZE);
        self.pages
            .reserve(required_pages.saturating_sub(self.pages.len()));
        while self.pages.len() < required_pages {
            let generation = self.next_page_generation;
            self.next_page_generation = self.next_page_generation.wrapping_add(1).max(1);
            let mut page = Box::new(ArenaPage::new(generation));
            let address = (&*page as *const ArenaPage<T, PAGE_SIZE>) as usize;
            page.check = self.key.mask_addr(address, 0)?;
            self.pages.push(page);
        }
        Ok(())
    }

    /// Insert a value and return an opaque paged-arena handle.
    pub fn insert(
        &mut self,
        value: T,
        tag: usize,
        selector: SelectorWord,
        prophecy: ProphecyBits,
    ) -> KeyedArenaResult<PagedKeyedArenaHandle<T, LOW_BITS, PAGE_SIZE>> {
        LowTagLayout::<LOW_BITS>::validate_tag(tag)?;

        let reused = self.free.pop();
        let index = match reused {
            Some(index) => index,
            None => {
                let index = self.next_slot;
                if index > u32::MAX as usize {
                    return Err(KeyedArenaError::CapacityExceeded);
                }
                self.reserve_entries(1)?;
                self.next_slot += 1;
                index
            }
        };

        let page_index = index / PAGE_SIZE;
        let slot_index = index % PAGE_SIZE;
        let generation = self.next_generation;
        self.next_generation = self.next_generation.wrapping_add(1).max(1);
        let placeholder = NonZeroUsize::new(1).expect("one is nonzero");

        let key = self.key;
        let handle_result = {
            let page = self
                .pages
                .get_mut(page_index)
                .expect("reserved page exists");
            debug_assert!(page.slots[slot_index].is_none());
            page.slots[slot_index] = Some(PagedArenaEntry {
                value,
                generation,
                check: placeholder,
                selector,
                prophecy,
            });

            let check_result = {
                let entry = page.slots[slot_index]
                    .as_ref()
                    .expect("inserted entry exists");
                let address = &entry.value as *const T as usize;
                key.mask_addr(address, tag)
            };

            match check_result {
                Ok(check) => {
                    let handle = {
                        let entry = page.slots[slot_index]
                            .as_mut()
                            .expect("inserted entry exists");
                        entry.check = check;
                        entry.handle(index)
                    };
                    page.live += 1;
                    Ok(handle)
                }
                Err(error) => {
                    page.slots[slot_index] = None;
                    Err(error)
                }
            }
        };

        match handle_result {
            Ok(handle) => {
                self.live += 1;
                Ok(handle)
            }
            Err(error) => {
                if reused.is_some() {
                    self.free.push(index);
                } else {
                    self.next_slot -= 1;
                }
                Err(KeyedArenaError::Tag(error))
            }
        }
    }

    fn entry(&self, index: usize) -> Option<&PagedArenaEntry<T>> {
        let page = self.pages.get(index / PAGE_SIZE)?;
        page.slots.get(index % PAGE_SIZE)?.as_ref()
    }

    fn entry_mut(&mut self, index: usize) -> Option<&mut PagedArenaEntry<T>> {
        let page = self.pages.get_mut(index / PAGE_SIZE)?;
        page.slots.get_mut(index % PAGE_SIZE)?.as_mut()
    }

    /// Validate a handle and return its flat live slot index.
    pub fn validate_handle(
        &self,
        handle: PagedKeyedArenaHandle<T, LOW_BITS, PAGE_SIZE>,
    ) -> KeyedArenaResult<usize> {
        let index = handle.index as usize;
        if index >= self.next_slot {
            return Err(KeyedArenaError::BadIndex {
                index: handle.index,
            });
        }
        let Some(entry) = self.entry(index) else {
            return Err(KeyedArenaError::Vacant {
                index: handle.index,
            });
        };
        if entry.generation != handle.generation {
            return Err(KeyedArenaError::StaleGeneration {
                index: handle.index,
                handle_generation: handle.generation,
                live_generation: entry.generation,
            });
        }
        if entry.check != handle.check {
            return Err(KeyedArenaError::CheckMismatch {
                index: handle.index,
            });
        }
        if entry.selector != handle.selector {
            return Err(KeyedArenaError::SelectorMismatch {
                index: handle.index,
            });
        }
        if entry.prophecy != handle.prophecy {
            return Err(KeyedArenaError::ProphecyMismatch {
                index: handle.index,
            });
        }
        let revealed = self.key.reveal_addr(handle.check)?;
        if revealed != &entry.value as *const T as usize {
            return Err(KeyedArenaError::RevealMismatch {
                index: handle.index,
            });
        }
        Ok(index)
    }

    /// Return true if the handle currently resolves to a live entry.
    pub fn contains(&self, handle: PagedKeyedArenaHandle<T, LOW_BITS, PAGE_SIZE>) -> bool {
        self.validate_handle(handle).is_ok()
    }

    /// Return a shared reference through a validated handle.
    pub fn get(
        &self,
        handle: PagedKeyedArenaHandle<T, LOW_BITS, PAGE_SIZE>,
    ) -> KeyedArenaResult<&T> {
        let index = self.validate_handle(handle)?;
        Ok(&self.entry(index).expect("validated live paged entry").value)
    }

    /// Return a mutable reference through a validated handle.
    pub fn get_mut(
        &mut self,
        handle: PagedKeyedArenaHandle<T, LOW_BITS, PAGE_SIZE>,
    ) -> KeyedArenaResult<&mut T> {
        let index = self.validate_handle(handle)?;
        Ok(&mut self
            .entry_mut(index)
            .expect("validated live paged entry")
            .value)
    }

    /// Return the current handle for a flat index when it is live.
    pub fn handle_at(&self, index: usize) -> Option<PagedKeyedArenaHandle<T, LOW_BITS, PAGE_SIZE>> {
        self.entry(index).map(|entry| entry.handle(index))
    }

    /// Return all current live handles in flat slot order.
    pub fn live_handles(&self) -> Vec<PagedKeyedArenaHandle<T, LOW_BITS, PAGE_SIZE>> {
        (0..self.next_slot)
            .filter_map(|index| self.handle_at(index))
            .collect()
    }

    /// Set tag, selector, and prophecy in one validated metadata transition.
    ///
    /// The previous handle becomes invalid after this call.
    pub fn transition(
        &mut self,
        handle: PagedKeyedArenaHandle<T, LOW_BITS, PAGE_SIZE>,
        tag: usize,
        selector: SelectorWord,
        prophecy: ProphecyBits,
    ) -> KeyedArenaResult<PagedKeyedArenaHandle<T, LOW_BITS, PAGE_SIZE>> {
        LowTagLayout::<LOW_BITS>::validate_tag(tag)?;
        let index = self.validate_handle(handle)?;
        let key = self.key;
        let entry = self.entry_mut(index).expect("validated live paged entry");
        let address = &entry.value as *const T as usize;
        entry.check = key.mask_addr(address, tag)?;
        entry.selector = selector;
        entry.prophecy = prophecy;
        Ok(entry.handle(index))
    }

    /// Set only the low tag and return the refreshed handle.
    pub fn set_tag(
        &mut self,
        handle: PagedKeyedArenaHandle<T, LOW_BITS, PAGE_SIZE>,
        tag: usize,
    ) -> KeyedArenaResult<PagedKeyedArenaHandle<T, LOW_BITS, PAGE_SIZE>> {
        self.transition(handle, tag, handle.selector, handle.prophecy)
    }

    /// Set only the selector and return the refreshed handle.
    pub fn set_selector(
        &mut self,
        handle: PagedKeyedArenaHandle<T, LOW_BITS, PAGE_SIZE>,
        selector: SelectorWord,
    ) -> KeyedArenaResult<PagedKeyedArenaHandle<T, LOW_BITS, PAGE_SIZE>> {
        self.transition(handle, handle.tag(), selector, handle.prophecy)
    }

    /// Set only prophecy bits and return the refreshed handle.
    pub fn set_prophecy(
        &mut self,
        handle: PagedKeyedArenaHandle<T, LOW_BITS, PAGE_SIZE>,
        prophecy: ProphecyBits,
    ) -> KeyedArenaResult<PagedKeyedArenaHandle<T, LOW_BITS, PAGE_SIZE>> {
        self.transition(handle, handle.tag(), handle.selector, prophecy)
    }

    /// Refresh the generation for a live slot and return the updated handle.
    pub fn refresh_generation(
        &mut self,
        handle: PagedKeyedArenaHandle<T, LOW_BITS, PAGE_SIZE>,
    ) -> KeyedArenaResult<PagedKeyedArenaHandle<T, LOW_BITS, PAGE_SIZE>> {
        let index = self.validate_handle(handle)?;
        let generation = self.next_generation;
        self.next_generation = self.next_generation.wrapping_add(1).max(1);
        let entry = self.entry_mut(index).expect("validated live paged entry");
        entry.generation = generation;
        Ok(entry.handle(index))
    }

    /// Remove a value. The old handle becomes invalid immediately.
    pub fn remove(
        &mut self,
        handle: PagedKeyedArenaHandle<T, LOW_BITS, PAGE_SIZE>,
    ) -> KeyedArenaResult<T> {
        let index = self.validate_handle(handle)?;
        let page_index = index / PAGE_SIZE;
        let slot_index = index % PAGE_SIZE;
        let value = {
            let page = self
                .pages
                .get_mut(page_index)
                .expect("validated page exists");
            let entry = page.slots[slot_index]
                .take()
                .expect("validated entry exists");
            page.live -= 1;
            entry.value
        };
        self.live -= 1;
        self.free.push(index);
        Ok(value)
    }

    /// Drop every live entry while retaining all allocated physical pages.
    ///
    /// All previously issued handles become invalid immediately. Future inserts
    /// reuse the retained pages from slot zero and receive fresh generations and
    /// keyed check words. This is the reset operation for reusable workspaces.
    pub fn reset_retain_pages(&mut self) {
        let key = self.key;
        for page in &mut self.pages {
            for slot in &mut page.slots {
                let _ = slot.take();
            }
            page.live = 0;
            page.generation = self.next_page_generation;
            self.next_page_generation = self.next_page_generation.wrapping_add(1).max(1);
            let address = (&**page as *const ArenaPage<T, PAGE_SIZE>) as usize;
            page.check = key
                .mask_addr(address, 0)
                .expect("aligned retained page address remains maskable");
        }
        self.free.clear();
        self.live = 0;
        self.next_slot = 0;
        self.next_generation = self.next_generation.wrapping_add(1).max(1);
    }

    /// Remove all live entries and release every page.
    pub fn clear(&mut self) {
        self.pages.clear();
        self.free.clear();
        self.live = 0;
        self.next_slot = 0;
        self.next_generation = self.next_generation.wrapping_add(1).max(1);
    }
}
