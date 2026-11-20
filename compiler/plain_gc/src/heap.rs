//! The heap: pages of objects, allocation, and collections, with the program stopped for now (PlainGC's steps 1
//! and 2, D69).
//!
//! As in ZGC, the heap is made of pages. Small objects are bump-allocated in small pages; an object too big for that
//! gets a large page of its own and is never moved. Every stored reference is colored (`color.rs`). A collection
//! marks everything reachable from the roots, healing each reference it passes; then evacuates the live objects of
//! sparse pages into fresh pages, recording where each went in a forwarding table, and frees the pages it emptied. It
//! doesn't rewrite any reference: it changes the good color instead, so every older reference goes through the load
//! barrier's slow path, which finds the object through the forwarding table. The next collection's marking heals
//! every reference it reaches, and then that forwarding table is dropped.

use std::alloc::{Layout, alloc_zeroed, dealloc};
use std::collections::{BTreeMap, HashMap, HashSet};

use crate::color::{FIRST_COLOR, address, colored, next_color};
use crate::object::{MAX_LENGTH, ObjectKind, WORD, header, object_size, read_header};

/// The size of a small page.
pub const PAGE_SIZE: usize = 256 * 1024;

/// The biggest object put in a small page; anything bigger gets a large page of its own.
pub const LARGEST_SMALL_OBJECT: usize = PAGE_SIZE / 8;

/// A page is evacuated when less than this share of it is live. A placeholder until the design session decides
/// (`compiler/PROGRESS.md`).
const EVACUATE_BELOW_LIVE_PERCENT: usize = 75;

/// Pages are aligned to this.
const PAGE_ALIGNMENT: usize = 16;

/// Why an allocation or a collection couldn't go on.
#[derive(Debug, PartialEq, Eq)]
pub enum HeapError {
    /// The system has no more memory to give.
    OutOfMemory,
    /// PlainGC found something that can only be a bug in PlainGC or PlainCompiler.
    Internal(String),
}

/// Visits every root, each a place holding a colored reference that the collector may heal.
pub type RootVisitor<'a> = dyn FnMut(&mut dyn FnMut(&mut usize)) -> Result<(), HeapError> + 'a;

struct Page {
    start: usize,
    size: usize,
    /// How many bytes from the start are allocated.
    top: usize,
    large: bool,
    /// One mark bit per word, set on the word where a live object starts, for the marking numbered `cycle`.
    marks: Vec<u64>,
    live_bytes: usize,
    /// Which marking `marks` and `live_bytes` belong to. Older ones count as nothing marked, so starting a marking
    /// never has to clear every page, and its pause stays the same at any heap size (D69).
    cycle: u64,
}

impl Page {
    fn new(size: usize, large: bool) -> Result<Page, HeapError> {
        let layout =
            Layout::from_size_align(size, PAGE_ALIGNMENT).map_err(|_| HeapError::OutOfMemory)?;
        // SAFETY: `layout` has a non-zero size, since every page holds at least one object of at least one word.
        let start = unsafe { alloc_zeroed(layout) };
        if start.is_null() {
            return Err(HeapError::OutOfMemory);
        }
        Ok(Page {
            start: start as usize,
            size,
            top: 0,
            large,
            marks: vec![0; (size / WORD).div_ceil(64)],
            live_bytes: 0,
            cycle: 0,
        })
    }

    fn contains(&self, address: usize) -> bool {
        address >= self.start && address < self.start + self.top
    }

    /// Sets the mark bit for the object at `address`, giving whether it was newly set.
    fn mark(&mut self, address: usize, cycle: u64) -> bool {
        if self.cycle != cycle {
            self.marks.iter_mut().for_each(|cell| *cell = 0);
            self.live_bytes = 0;
            self.cycle = cycle;
        }
        let word = (address - self.start) / WORD;
        let bit = 1u64 << (word % 64);
        let cell = &mut self.marks[word / 64];
        let newly = *cell & bit == 0;
        *cell |= bit;
        newly
    }

    fn is_marked(&self, address: usize, cycle: u64) -> bool {
        let word = (address - self.start) / WORD;
        self.cycle == cycle && self.marks[word / 64] & (1u64 << (word % 64)) != 0
    }

    /// The bytes marked live in the marking numbered `cycle`.
    fn live(&self, cycle: u64) -> usize {
        if self.cycle == cycle {
            self.live_bytes
        } else {
            0
        }
    }
}

impl Drop for Page {
    fn drop(&mut self) {
        if let Ok(layout) = Layout::from_size_align(self.size, PAGE_ALIGNMENT) {
            // SAFETY: `start` came from `alloc_zeroed` with this same layout in `Page::new`, and a page is dropped
            // only once, when the heap no longer holds any reference into it.
            unsafe { dealloc(self.start as *mut u8, layout) };
        }
    }
}

/// The kind and payload length of the object at `address`.
fn object_at(address: usize) -> Result<(ObjectKind, usize), HeapError> {
    // SAFETY: callers pass the address of an object inside a page, which starts with its header word, aligned.
    let word = unsafe { (address as *const u64).read() };
    read_header(word).ok_or_else(|| {
        HeapError::Internal(format!(
            "PlainGC found no object header at address {address:#x}"
        ))
    })
}

/// The address of every reference inside the object at `address`.
fn references_in(address: usize) -> Result<Vec<usize>, HeapError> {
    match object_at(address)? {
        (ObjectKind::Bytes, _) => Ok(Vec::new()),
        (ObjectKind::References, length) => Ok((0..length / WORD)
            .map(|index| address + WORD + index * WORD)
            .collect()),
    }
}

/// Reads the colored reference stored at `place`.
fn read_reference(place: usize) -> u64 {
    // SAFETY: `place` is a word inside a live object of kind `References`, found by `references_in`.
    unsafe { (place as *const u64).read() }
}

fn write_reference(place: usize, value: u64) {
    // SAFETY: as for `read_reference`.
    unsafe { (place as *mut u64).write(value) }
}

/// A reference brought up to date: good-colored, and pointing where its object is now. A reference of a bad color
/// may point where its object was before the last collection moved it; the forwarding table says where it went.
fn heal_with(forwarding: &HashMap<usize, usize>, good: u64, reference: u64) -> u64 {
    if reference == 0 || reference & good != 0 {
        return reference;
    }
    let old = address(reference);
    colored(forwarding.get(&old).copied().unwrap_or(old), good)
}

/// What a collection did, for tests and for the pacer.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CollectionReport {
    pub live_objects: usize,
    pub live_bytes: usize,
    pub moved_objects: usize,
    pub freed_pages: usize,
}

pub struct Heap {
    /// Every page, by its start address.
    pages: BTreeMap<usize, Page>,
    /// The small page allocation is filling.
    current: Option<usize>,
    /// Bytes allocated since the last collection.
    pub allocated_since_collection: usize,
    /// Bytes live after the last collection.
    pub live_after_collection: usize,
    /// Bytes the heap's pages take: what counts against its limits (D114).
    pub used: usize,
    /// Every byte ever allocated, for the pacer's allocation rate (D113).
    pub allocated_total: u64,
    /// The good color: references of any other color go through the barrier's slow path.
    good: u64,
    /// Where each object the last collection moved went, by its old address, kept until the next collection's
    /// marking has healed every reference to it.
    forwarding: HashMap<usize, usize>,
    /// How many references the barrier's slow path has healed.
    pub healed: usize,
    /// How many objects the barrier moved itself, reaching them before PlainGC's thread did.
    pub relocated_by_barrier: usize,
    /// Whether a marking is under way, between [`Heap::begin_marking`] and [`Heap::finish`].
    marking: bool,
    /// Objects reached but not yet marked and traced: marking's work.
    work: Vec<usize>,
    /// What the collection under way has found so far.
    report: CollectionReport,
    /// The pages being evacuated, by their start, until every live object in them has moved.
    relocation_set: HashSet<usize>,
    /// Evacuated pages, emptied but still holding their memory while the forwarding table has entries for their
    /// addresses: if the system gave an address back for a new page, a new object there would be mistaken for one
    /// that moved away. They're freed when the forwarding table is dropped, as in ZGC.
    retired: Vec<Page>,
    /// The pages still to walk for objects to move, and how far into the last of them the walk has got.
    relocating: Vec<usize>,
    relocation_cursor: usize,
    /// The number of the marking under way or last finished, which tells current mark bits from stale ones.
    cycle: u64,
}

// SAFETY: a `Heap` owns its pages' memory outright, and is only ever reached through the one lock in `lib.rs`, so
// moving it to another thread can't let two threads touch its memory at once.
unsafe impl Send for Heap {}

impl Default for Heap {
    fn default() -> Heap {
        Heap::new()
    }
}

impl Heap {
    pub fn new() -> Heap {
        Heap {
            pages: BTreeMap::new(),
            current: None,
            allocated_since_collection: 0,
            live_after_collection: 0,
            used: 0,
            allocated_total: 0,
            good: FIRST_COLOR,
            forwarding: HashMap::new(),
            healed: 0,
            relocated_by_barrier: 0,
            marking: false,
            work: Vec::new(),
            report: CollectionReport::default(),
            relocation_set: HashSet::new(),
            retired: Vec::new(),
            relocating: Vec::new(),
            relocation_cursor: 0,
            cycle: 0,
        }
    }

    /// The good color.
    pub fn good_color(&self) -> u64 {
        self.good
    }

    /// A reference to the object at `address`, good-colored.
    pub fn reference_to(&self, address: usize) -> u64 {
        colored(address, self.good)
    }

    /// The load barrier's slow path: heals a reference of a bad color (D69). While marking, the object it points to
    /// joins marking's work, so the program never holds a reference marking hasn't reached. While objects are moving,
    /// one the program reaches before PlainGC's thread has moved it is moved here and now, so the program only ever
    /// sees an object in its new place.
    pub fn heal(&mut self, reference: u64) -> Result<u64, HeapError> {
        self.healed += 1;
        let old = address(reference);
        if reference != 0
            && reference & self.good == 0
            && !self.forwarding.contains_key(&old)
            && self
                .page_of(old)
                .is_some_and(|page| self.relocation_set.contains(&page.start))
        {
            // trace: D69
            self.relocated_by_barrier += 1;
            return Ok(colored(self.relocate(old)?, self.good));
        }
        let healed = heal_with(&self.forwarding, self.good, reference);
        if self.marking && healed != 0 {
            self.work.push(address(healed));
        }
        Ok(healed)
    }

    /// The page holding the object at `address`, if it's in the heap.
    fn page_of(&self, address: usize) -> Option<&Page> {
        self.pages
            .range(..=address)
            .next_back()
            .map(|(_, page)| page)
            .filter(|page| page.contains(address))
    }

    fn page_of_mut(&mut self, address: usize) -> Option<&mut Page> {
        self.pages
            .range_mut(..=address)
            .next_back()
            .map(|(_, page)| page)
            .filter(|page| page.contains(address))
    }

    /// How many pages the heap holds.
    #[cfg(test)]
    pub fn page_count(&self) -> usize {
        self.pages.len()
    }

    /// Allocates an object of `kind` with a payload of `length` bytes, all zero. Gives its address.
    pub fn allocate(&mut self, kind: ObjectKind, length: usize) -> Result<usize, HeapError> {
        if length > MAX_LENGTH {
            return Err(HeapError::OutOfMemory);
        }
        let size = object_size(length);
        let address = if size > LARGEST_SMALL_OBJECT {
            let mut page = Page::new(size, true)?;
            page.top = size;
            let start = page.start;
            self.used += page.size;
            self.pages.insert(start, page);
            start
        } else {
            self.bump(size)?
        };
        // SAFETY: `address` is the start of `size` freshly allocated, zeroed bytes in one of the heap's pages.
        unsafe { (address as *mut u64).write(header(kind, length)) };
        self.allocated_since_collection += size;
        self.allocated_total += size as u64;
        if self.marking {
            // An object made while marking is live: it's marked at birth, and needs no tracing, since every
            // reference stored in it has already passed the barrier.
            let cycle = self.cycle;
            if let Some(page) = self.page_of_mut(address) {
                page.mark(address, cycle);
                page.live_bytes += size;
            }
            self.report.live_objects += 1;
            self.report.live_bytes += size;
        }
        Ok(address)
    }

    /// Takes `size` bytes from the current small page, starting a new one when it's full.
    fn bump(&mut self, size: usize) -> Result<usize, HeapError> {
        if let Some(start) = self.current
            && let Some(page) = self.pages.get_mut(&start)
            && page.top + size <= page.size
        {
            let address = page.start + page.top;
            page.top += size;
            return Ok(address);
        }
        let mut page = Page::new(PAGE_SIZE, false)?;
        let address = page.start;
        page.top = size;
        self.used += page.size;
        self.current = Some(page.start);
        self.pages.insert(page.start, page);
        Ok(address)
    }

    /// A whole collection with the program stopped: every phase in a row, for tests.
    #[cfg(test)]
    pub fn collect(
        &mut self,
        roots: &mut RootVisitor<'_>,
        evacuate_everything: bool,
    ) -> Result<CollectionReport, HeapError> {
        self.begin_marking(roots)?;
        while !self.mark_some(usize::MAX)? {}
        self.finish(evacuate_everything)
    }

    /// Starts a marking, with the program paused: a new good color, and every root healed and handed to marking.
    /// The roots are the only thing marking needs the program paused for.
    pub fn begin_marking(&mut self, roots: &mut RootVisitor<'_>) -> Result<(), HeapError> {
        // trace: D69
        self.cycle += 1;
        self.report = CollectionReport::default();
        self.good = next_color(self.good);
        self.marking = true;
        self.work.clear();
        let (forwarding, good, work) = (&self.forwarding, self.good, &mut self.work);
        roots(&mut |slot: &mut usize| {
            let healed = heal_with(forwarding, good, *slot as u64);
            *slot = healed as usize;
            if healed != 0 {
                work.push(address(healed));
            }
        })
    }

    /// Marks and traces up to `budget` objects, healing every reference inside them. Gives whether marking's work
    /// has run out; the barrier can still add more, until [`Heap::finish`].
    pub fn mark_some(&mut self, budget: usize) -> Result<bool, HeapError> {
        let mut done = 0;
        while done < budget {
            let Some(object) = self.work.pop() else {
                return Ok(true);
            };
            done += 1;
            let cycle = self.cycle;
            let Some(page) = self.page_of_mut(object) else {
                return Err(HeapError::Internal(format!(
                    "a reference to {object:#x} points outside PlainGC's heap"
                )));
            };
            if !page.mark(object, cycle) {
                continue;
            }
            let (_, length) = object_at(object)?;
            page.live_bytes += object_size(length);
            self.report.live_objects += 1;
            self.report.live_bytes += object_size(length);
            for place in references_in(object)? {
                let healed = heal_with(&self.forwarding, self.good, read_reference(place));
                write_reference(place, healed);
                if healed != 0 {
                    self.work.push(address(healed));
                }
            }
        }
        Ok(self.work.is_empty())
    }

    /// The end of a collection, all at once, for tests: the end of marking, every move, and freeing the pages.
    #[cfg(test)]
    pub fn finish(&mut self, evacuate_everything: bool) -> Result<CollectionReport, HeapError> {
        let mut report = self.end_marking(evacuate_everything)?;
        while !self.relocate_some(usize::MAX)? {}
        report.moved_objects = self.end_relocation();
        Ok(report)
    }

    /// Ends marking, with the program paused: marks what the barrier added since marking's work last ran out,
    /// chooses the sparse pages to evacuate, frees the empty ones, and changes the good color. Nothing moves yet:
    /// [`Heap::relocate_some`] moves the live objects while the program runs, and the barrier moves any the program
    /// reaches first. With `evacuate_everything`, every small page with a live object is evacuated, so tests reach
    /// the moving path.
    pub fn end_marking(
        &mut self,
        evacuate_everything: bool,
    ) -> Result<CollectionReport, HeapError> {
        while !self.mark_some(usize::MAX)? {}
        self.marking = false;
        let report = std::mem::take(&mut self.report);
        // Every reachable reference is healed by now, so the last collection's forwarding is no longer needed.
        self.forwarding.clear();
        // trace: D69
        for page in std::mem::take(&mut self.retired) {
            self.used -= page.size;
        }

        // Only pages are chosen here; PlainGC's thread finds their live objects as it moves them, so this pause stays
        // short at any heap size (D69).
        let mut freed = Vec::new();
        self.relocation_set.clear();
        self.relocating.clear();
        self.relocation_cursor = 0;
        for (&start, page) in &self.pages {
            let live = page.live(self.cycle);
            if live == 0 {
                freed.push(start);
            } else if !page.large
                && (evacuate_everything || live * 100 < page.top * EVACUATE_BELOW_LIVE_PERCENT)
            {
                self.relocation_set.insert(start);
                self.relocating.push(start);
            }
        }
        for start in &freed {
            if let Some(page) = self.pages.remove(start) {
                self.used -= page.size;
            }
        }
        // Moved objects go into fresh pages, never into a page being evacuated.
        self.current = None;
        // trace: D69
        self.good = next_color(self.good);
        self.allocated_since_collection = 0;
        self.live_after_collection = report.live_bytes;
        Ok(report)
    }

    /// Moves the live objects out of the pages being evacuated while the program runs, looking at up to `budget`
    /// objects. Gives whether every one has moved.
    pub fn relocate_some(&mut self, budget: usize) -> Result<bool, HeapError> {
        for _ in 0..budget {
            let Some(&start) = self.relocating.last() else {
                return Ok(true);
            };
            let Some(page) = self.pages.get(&start) else {
                return Err(HeapError::Internal(
                    "a page being evacuated went missing".to_string(),
                ));
            };
            if self.relocation_cursor >= page.top {
                self.relocating.pop();
                self.relocation_cursor = 0;
                continue;
            }
            let object = page.start + self.relocation_cursor;
            let live = page.is_marked(object, self.cycle);
            let (_, length) = object_at(object)?;
            self.relocation_cursor += object_size(length);
            if live && !self.forwarding.contains_key(&object) {
                self.relocate(object)?;
            }
        }
        Ok(self.relocating.is_empty())
    }

    /// Moves one object, records where it went, and gives its new address.
    fn relocate(&mut self, old: usize) -> Result<usize, HeapError> {
        let (_, length) = object_at(old)?;
        let size = object_size(length);
        let new = self.bump(size)?;
        // SAFETY: `old` is a live object of `size` bytes in a page being evacuated, which stays until every object in
        // it has moved, and `new` is `size` fresh bytes in another page, so the two can't overlap.
        unsafe { std::ptr::copy_nonoverlapping(old as *const u8, new as *mut u8, size) };
        self.forwarding.insert(old, new);
        Ok(new)
    }

    /// Ends the moving, once every object has moved: takes the evacuated pages out of the heap. Gives how many
    /// objects moved. The forwarding table stays until the next marking has healed every reference to their old
    /// places, and so do the pages' memory, so no new page can take their addresses meanwhile.
    pub fn end_relocation(&mut self) -> usize {
        for start in std::mem::take(&mut self.relocation_set) {
            if let Some(page) = self.pages.remove(&start) {
                self.retired.push(page);
            }
        }
        self.forwarding.len()
    }

    /// Whether `address` is an object in the heap, for tests.
    #[cfg(test)]
    pub fn holds(&self, address: usize) -> bool {
        self.page_of(address).is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::bad_mask;

    /// A text object's reference, good-colored, as allocation gives it.
    fn text(heap: &mut Heap, value: &str) -> u64 {
        let object = heap.allocate(ObjectKind::Bytes, value.len()).unwrap_or(0);
        // SAFETY: the object was just allocated with a payload of `value.len()` bytes.
        unsafe {
            std::ptr::copy_nonoverlapping(value.as_ptr(), (object + WORD) as *mut u8, value.len())
        };
        heap.reference_to(object)
    }

    fn list(heap: &mut Heap, items: &[u64]) -> u64 {
        let object = heap
            .allocate(ObjectKind::References, items.len() * WORD)
            .unwrap_or(0);
        for (index, &item) in items.iter().enumerate() {
            write_reference(object + WORD + index * WORD, item);
        }
        heap.reference_to(object)
    }

    /// Reads through a reference as compiled code does: the barrier first, healing a bad-colored reference.
    fn read(heap: &mut Heap, reference: &mut u64) -> usize {
        if *reference & bad_mask(heap.good_color()) != 0 {
            *reference = heap.heal(*reference).unwrap_or(0);
        }
        address(*reference)
    }

    fn text_at(heap: &mut Heap, reference: &mut u64) -> String {
        let object = read(heap, reference);
        let (_, length) = object_at(object).unwrap_or((ObjectKind::Bytes, 0));
        // SAFETY: `object` is a live text object with a payload of `length` bytes.
        let bytes = unsafe { std::slice::from_raw_parts((object + WORD) as *const u8, length) };
        String::from_utf8_lossy(bytes).into_owned()
    }

    /// The `index`th reference in a list, read through the barrier, which heals it where it's stored.
    fn item(heap: &mut Heap, list: &mut u64, index: usize) -> u64 {
        let object = read(heap, list);
        let place = object + WORD + index * WORD;
        let mut reference = read_reference(place);
        read(heap, &mut reference);
        write_reference(place, reference);
        reference
    }

    /// Collects with `roots` as the only roots.
    fn collect(heap: &mut Heap, roots: &mut [u64], everything: bool) -> CollectionReport {
        let mut visit = |visitor: &mut dyn FnMut(&mut usize)| {
            for root in roots.iter_mut() {
                let mut slot = *root as usize;
                visitor(&mut slot);
                *root = slot as u64;
            }
            Ok(())
        };
        heap.collect(&mut visit, everything).unwrap_or_default()
    }

    #[test]
    fn allocation_gives_separate_zeroed_objects() {
        let mut heap = Heap::new();
        let first = heap.allocate(ObjectKind::References, 16).unwrap_or(0);
        let second = heap.allocate(ObjectKind::Bytes, 3).unwrap_or(0);
        assert_ne!(first, second);
        assert_eq!(object_at(first), Ok((ObjectKind::References, 16)));
        assert_eq!(read_reference(first + WORD), 0);
        assert_eq!(second, first + object_size(16));
    }

    #[test]
    fn a_big_object_gets_a_page_of_its_own() {
        let mut heap = Heap::new();
        let small = heap.allocate(ObjectKind::Bytes, 10).unwrap_or(0);
        let big = heap
            .allocate(ObjectKind::Bytes, LARGEST_SMALL_OBJECT)
            .unwrap_or(0);
        assert!(heap.holds(small) && heap.holds(big));
        assert_eq!(heap.page_count(), 2);
    }

    #[test]
    fn marking_follows_references_and_skips_garbage() {
        let mut heap = Heap::new();
        let a = text(&mut heap, "a");
        let b = text(&mut heap, "b");
        let _garbage = text(&mut heap, "garbage");
        let list = list(&mut heap, &[a, b]);
        let report = collect(&mut heap, &mut [list], false);
        assert_eq!(report.live_objects, 3);
    }

    #[test]
    fn nothing_reachable_frees_every_page() {
        let mut heap = Heap::new();
        for _ in 0..10_000 {
            text(&mut heap, "garbage, all of it");
        }
        assert!(heap.page_count() > 1);
        let report = collect(&mut heap, &mut [], false);
        assert_eq!(report.live_objects, 0);
        assert_eq!(heap.page_count(), 0);
    }

    #[test]
    fn a_collection_changes_the_good_color() {
        let mut heap = Heap::new();
        let mut kept = text(&mut heap, "kept");
        let before = heap.good_color();
        collect(&mut heap, &mut [kept], false);
        assert_ne!(heap.good_color(), before);
        assert_ne!(kept & bad_mask(heap.good_color()), 0);
        assert_eq!(text_at(&mut heap, &mut kept), "kept");
        assert_eq!(kept & bad_mask(heap.good_color()), 0);
    }

    #[test]
    fn the_barrier_finds_a_moved_object_from_its_old_reference() {
        let mut heap = Heap::new();
        let mut first = text(&mut heap, "first");
        let before = address(first);
        collect(&mut heap, &mut [first], true);
        // The root still holds the old address: nothing rewrote it, and only the barrier heals it.
        assert_eq!(address(first), before);
        assert_eq!(text_at(&mut heap, &mut first), "first");
        assert_ne!(address(first), before);
        assert_eq!(heap.healed, 1);
    }

    #[test]
    fn references_inside_objects_are_forwarded_too() {
        let mut heap = Heap::new();
        let a = text(&mut heap, "first");
        let b = text(&mut heap, "second");
        let mut list = list(&mut heap, &[a, b]);
        let report = collect(&mut heap, &mut [list], true);
        assert_eq!(report.moved_objects, 3);
        let mut first = item(&mut heap, &mut list, 0);
        let mut second = item(&mut heap, &mut list, 1);
        assert_eq!(text_at(&mut heap, &mut first), "first");
        assert_eq!(text_at(&mut heap, &mut second), "second");
    }

    #[test]
    fn the_next_marking_heals_every_reference_and_drops_the_forwarding() {
        let mut heap = Heap::new();
        let a = text(&mut heap, "inside");
        let mut list = list(&mut heap, &[a]);
        let mut roots = [list];
        collect(&mut heap, &mut roots, true);
        // A second collection that moves nothing: its marking heals the root and the reference in the list.
        collect(&mut heap, &mut roots, false);
        list = roots[0];
        let good = heap.good_color();
        let object = address(list);
        // The second collection changed the color again, so heal once more through the empty forwarding table.
        let healed = heap.heal(list).unwrap_or(0);
        assert_eq!(address(healed), object);
        assert!(heap.forwarding.is_empty());
        let mut inside = item(&mut heap, &mut list, 0);
        assert_eq!(text_at(&mut heap, &mut inside), "inside");
        assert_ne!(good, FIRST_COLOR);
    }

    #[test]
    fn compacting_a_sparse_page_keeps_every_value() {
        let mut heap = Heap::new();
        let mut kept = Vec::new();
        for number in 0..50_000 {
            let reference = text(&mut heap, &number.to_string());
            if number % 10 == 0 {
                kept.push(reference);
            }
        }
        let pages_before = heap.page_count();
        let report = collect(&mut heap, &mut kept, false);
        assert!(report.moved_objects > 0);
        assert!(heap.page_count() < pages_before);
        for (index, reference) in kept.iter_mut().enumerate() {
            assert_eq!(text_at(&mut heap, reference), (index * 10).to_string());
        }
    }

    #[test]
    fn a_cycle_is_marked_once_and_moved_whole() {
        let mut heap = Heap::new();
        let first = list(&mut heap, &[0]);
        let second = list(&mut heap, &[first]);
        write_reference(address(first) + WORD, second);
        let mut roots = [first];
        let report = collect(&mut heap, &mut roots, true);
        assert_eq!(report.live_objects, 2);
        let mut first = roots[0];
        let mut second = item(&mut heap, &mut first, 0);
        let back = item(&mut heap, &mut second, 0);
        assert_eq!(address(back), address(first));
    }

    #[test]
    fn several_collections_in_a_row_keep_every_value() {
        let mut heap = Heap::new();
        let mut roots = [text(&mut heap, "one"), text(&mut heap, "two")];
        for round in 0..10 {
            for _ in 0..1000 {
                text(&mut heap, "garbage");
            }
            collect(&mut heap, &mut roots, round % 2 == 0);
        }
        assert_eq!(text_at(&mut heap, &mut roots[0]), "one");
        assert_eq!(text_at(&mut heap, &mut roots[1]), "two");
    }

    #[test]
    fn large_objects_stay_where_they_are() {
        let mut heap = Heap::new();
        let big = heap
            .allocate(ObjectKind::Bytes, LARGEST_SMALL_OBJECT * 2)
            .unwrap_or(0);
        let mut roots = [heap.reference_to(big)];
        collect(&mut heap, &mut roots, true);
        assert_eq!(read(&mut heap, &mut roots[0]), big);
        assert!(heap.holds(big));
    }

    #[test]
    fn a_reference_outside_the_heap_is_a_bug() {
        let mut heap = Heap::new();
        let mut roots = [heap.reference_to(0x10)];
        let mut visit = |visitor: &mut dyn FnMut(&mut usize)| {
            for root in roots.iter_mut() {
                let mut slot = *root as usize;
                visitor(&mut slot);
                *root = slot as u64;
            }
            Ok(())
        };
        assert!(matches!(
            heap.collect(&mut visit, false),
            Err(HeapError::Internal(_))
        ));
    }

    /// Roots as a visitor over `roots`, for the phases' tests.
    fn visit(roots: &mut [u64]) -> Box<RootVisitor<'_>> {
        Box::new(move |visitor: &mut dyn FnMut(&mut usize)| {
            for root in roots.iter_mut() {
                let mut slot = *root as usize;
                visitor(&mut slot);
                *root = slot as u64;
            }
            Ok(())
        })
    }

    #[test]
    fn a_reference_taken_while_marking_is_never_lost() {
        let mut heap = Heap::new();
        let hidden = text(&mut heap, "hidden");
        // A collection first, so every reference stored in the heap has an old color.
        let mut roots = [list(&mut heap, &[hidden])];
        collect(&mut heap, &mut roots, false);
        heap.begin_marking(&mut *visit(&mut roots)).ok();
        let mut holder = roots[0];
        // Before marking traces `holder`, the program takes `hidden` out of it through the barrier, then clears the
        // place it came from: marking can now only learn of `hidden` from the barrier.
        let mut taken = item(&mut heap, &mut holder, 0);
        let object = read(&mut heap, &mut holder);
        write_reference(object + WORD, 0);
        let fresh = text(&mut heap, "made while marking");
        while !heap.mark_some(1).unwrap_or(true) {}
        let report = heap.finish(true).unwrap_or_default();
        assert_eq!(report.live_objects, 3);
        assert_eq!(text_at(&mut heap, &mut taken), "hidden");
        let mut fresh = fresh;
        assert_eq!(text_at(&mut heap, &mut fresh), "made while marking");
    }

    #[test]
    fn marking_on_another_thread_while_the_program_allocates_and_reads() {
        use std::sync::{Arc, Mutex};
        let mut heap = Heap::new();
        let items: Vec<u64> = (0..5_000)
            .map(|number| text(&mut heap, &number.to_string()))
            .collect();
        let mut roots = [list(&mut heap, &items)];
        collect(&mut heap, &mut roots, false);
        heap.begin_marking(&mut *visit(&mut roots)).ok();
        let heap = Arc::new(Mutex::new(heap));
        let marker = {
            let heap = Arc::clone(&heap);
            std::thread::spawn(move || {
                loop {
                    let done = match heap.lock() {
                        Ok(mut heap) => heap.mark_some(64).unwrap_or(true),
                        Err(_) => true,
                    };
                    if done {
                        break;
                    }
                    std::thread::yield_now();
                }
            })
        };
        // Meanwhile the program allocates garbage and reads items through the barrier.
        for number in 0..5_000 {
            if let Ok(mut heap) = heap.lock() {
                text(&mut heap, "garbage");
                if number % 50 == 0 {
                    let mut item = item(&mut heap, &mut roots[0], number);
                    assert_eq!(text_at(&mut heap, &mut item), number.to_string());
                }
            }
        }
        assert!(marker.join().is_ok());
        let Ok(mut heap) = heap.lock() else {
            panic!("the heap's lock broke");
        };
        let report = heap.finish(true).unwrap_or_default();
        assert!(report.live_objects > 5_000);
        for number in (0..5_000).step_by(7) {
            let mut item = item(&mut heap, &mut roots[0], number);
            assert_eq!(text_at(&mut heap, &mut item), number.to_string());
        }
    }

    #[test]
    fn an_evacuated_page_keeps_its_memory_while_its_forwarding_lasts() {
        let mut heap = Heap::new();
        let mut kept = text(&mut heap, "kept");
        let old = address(kept);
        collect(&mut heap, &mut [kept], true);
        // The old place is still forwarded, so the page there keeps its memory: no new page can take its address
        // and have its objects mistaken for ones that moved away.
        assert!(heap.retired.iter().any(|page| page.contains(old)));
        let used_while_forwarding = heap.used;
        assert_eq!(text_at(&mut heap, &mut kept), "kept");
        // The next marking heals every reference and drops the forwarding, and the page's memory goes with it.
        let mut roots = [kept];
        collect(&mut heap, &mut roots, false);
        assert!(heap.retired.is_empty());
        assert!(heap.used < used_while_forwarding);
        assert_eq!(text_at(&mut heap, &mut roots[0]), "kept");
    }

    #[test]
    fn the_barrier_moves_an_object_the_program_reaches_first() {
        let mut heap = Heap::new();
        let mut roots = [text(&mut heap, "first"), text(&mut heap, "second")];
        heap.begin_marking(&mut *visit(&mut roots)).ok();
        while !heap.mark_some(usize::MAX).unwrap_or(true) {}
        heap.end_marking(true).ok();
        let before = address(roots[1]);
        // The program reads `second` before PlainGC's thread moves anything: the barrier moves it.
        assert_eq!(text_at(&mut heap, &mut roots[1]), "second");
        assert_ne!(address(roots[1]), before);
        assert_eq!(heap.relocated_by_barrier, 1);
        // Then the thread moves the rest, skipping what the barrier already moved.
        while !heap.relocate_some(1).unwrap_or(true) {}
        assert_eq!(heap.end_relocation(), 2);
        let moved_second = address(roots[1]);
        assert_eq!(text_at(&mut heap, &mut roots[0]), "first");
        assert_eq!(text_at(&mut heap, &mut roots[1]), "second");
        assert_eq!(address(roots[1]), moved_second);
    }

    #[test]
    fn moving_on_another_thread_while_the_program_allocates_and_reads() {
        use std::sync::{Arc, Mutex};
        let mut heap = Heap::new();
        let items: Vec<u64> = (0..5_000)
            .map(|number| text(&mut heap, &number.to_string()))
            .collect();
        let mut roots = [list(&mut heap, &items)];
        heap.begin_marking(&mut *visit(&mut roots)).ok();
        while !heap.mark_some(usize::MAX).unwrap_or(true) {}
        heap.end_marking(true).ok();
        let heap = Arc::new(Mutex::new(heap));
        let mover = {
            let heap = Arc::clone(&heap);
            std::thread::spawn(move || {
                loop {
                    let done = match heap.lock() {
                        Ok(mut heap) => heap.relocate_some(64).unwrap_or(true),
                        Err(_) => true,
                    };
                    if done {
                        break;
                    }
                    std::thread::yield_now();
                }
            })
        };
        // Meanwhile the program allocates and reads items, moving any the thread hasn't reached yet.
        for number in 0..5_000 {
            if let Ok(mut heap) = heap.lock() {
                text(&mut heap, "garbage");
                if number % 25 == 0 {
                    let mut item = item(&mut heap, &mut roots[0], number);
                    assert_eq!(text_at(&mut heap, &mut item), number.to_string());
                }
            }
        }
        assert!(mover.join().is_ok());
        let Ok(mut heap) = heap.lock() else {
            panic!("the heap's lock broke");
        };
        // Every text moved; the list itself is a large object, which stays where it is.
        assert_eq!(heap.end_relocation(), 5_000);
        for number in (0..5_000).step_by(3) {
            let mut item = item(&mut heap, &mut roots[0], number);
            assert_eq!(text_at(&mut heap, &mut item), number.to_string());
        }
    }
}
