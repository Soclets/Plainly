//! The heap: pages of objects, allocation, and a collection that marks and compacts with the program stopped
//! (step 1 of PlainGC, D69).
//!
//! As in ZGC, the heap is made of pages. Small objects are bump-allocated in small pages; an object too big for that
//! gets a large page of its own and is never moved. A collection marks everything reachable from the roots, then
//! evacuates the live objects of sparse pages into fresh pages, rewrites every reference to them through a
//! forwarding table, and frees the pages it emptied.

use std::alloc::{Layout, alloc_zeroed, dealloc};
use std::collections::{BTreeMap, HashMap};

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

/// Visits every root, each a place holding a reference that the collector may rewrite.
pub type RootVisitor<'a> = dyn FnMut(&mut dyn FnMut(&mut usize)) -> Result<(), HeapError> + 'a;

struct Page {
    start: usize,
    size: usize,
    /// How many bytes from the start are allocated.
    top: usize,
    large: bool,
    /// One mark bit per word, set on the word where a live object starts.
    marks: Vec<u64>,
    live_bytes: usize,
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
        })
    }

    fn contains(&self, address: usize) -> bool {
        address >= self.start && address < self.start + self.top
    }

    /// Sets the mark bit for the object at `address`, giving whether it was newly set.
    fn mark(&mut self, address: usize) -> bool {
        let word = (address - self.start) / WORD;
        let bit = 1u64 << (word % 64);
        let cell = &mut self.marks[word / 64];
        let newly = *cell & bit == 0;
        *cell |= bit;
        newly
    }

    fn is_marked(&self, address: usize) -> bool {
        let word = (address - self.start) / WORD;
        self.marks[word / 64] & (1u64 << (word % 64)) != 0
    }

    /// The address of every object in the page, in order.
    fn objects(&self) -> Result<Vec<usize>, HeapError> {
        let mut objects = Vec::new();
        let mut offset = 0;
        while offset < self.top {
            let address = self.start + offset;
            let (_, length) = object_at(address)?;
            objects.push(address);
            offset += object_size(length);
        }
        Ok(objects)
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

/// Reads the reference stored at `place`.
fn read_reference(place: usize) -> usize {
    // SAFETY: `place` is a word inside a live object of kind `References`, found by `references_in`.
    unsafe { (place as *const usize).read() }
}

fn write_reference(place: usize, value: usize) {
    // SAFETY: as for `read_reference`.
    unsafe { (place as *mut usize).write(value) }
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
        }
    }

    /// The page holding the object at `address`, if it's in the heap.
    #[cfg(test)]
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
            self.pages.insert(start, page);
            start
        } else {
            self.bump(size)?
        };
        // SAFETY: `address` is the start of `size` freshly allocated, zeroed bytes in one of the heap's pages.
        unsafe { (address as *mut u64).write(header(kind, length)) };
        self.allocated_since_collection += size;
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
        self.current = Some(page.start);
        self.pages.insert(page.start, page);
        Ok(address)
    }

    /// A collection, with the program stopped: marks everything reachable from the roots, then compacts. With
    /// `evacuate_everything`, every small page with a live object is evacuated, so tests reach the moving path.
    pub fn collect(
        &mut self,
        roots: &mut RootVisitor<'_>,
        evacuate_everything: bool,
    ) -> Result<CollectionReport, HeapError> {
        let mut report = CollectionReport::default();
        self.mark(roots, &mut report)?;

        // Which pages to evacuate, and which to free outright.
        let mut evacuated = Vec::new();
        let mut freed = Vec::new();
        for (&start, page) in &self.pages {
            if page.live_bytes == 0 {
                freed.push(start);
            } else if !page.large
                && (evacuate_everything
                    || page.live_bytes * 100 < page.top * EVACUATE_BELOW_LIVE_PERCENT)
            {
                evacuated.push(start);
            }
        }

        // Survivors go into fresh pages, never into a page being evacuated.
        self.current = None;
        let mut forwarding: HashMap<usize, usize> = HashMap::new();
        let mut moved = Vec::new();
        for &start in &evacuated {
            let objects = match self.pages.get(&start) {
                Some(page) => page
                    .objects()?
                    .into_iter()
                    .filter(|&address| page.is_marked(address))
                    .collect::<Vec<_>>(),
                None => Vec::new(),
            };
            for old in objects {
                let (_, length) = object_at(old)?;
                let size = object_size(length);
                let new = self.bump(size)?;
                // SAFETY: `old` is a live object of `size` bytes in a page being evacuated, and `new` is `size`
                // fresh bytes in another page, so the two can't overlap.
                unsafe { std::ptr::copy_nonoverlapping(old as *const u8, new as *mut u8, size) };
                forwarding.insert(old, new);
                moved.push(new);
            }
        }
        report.moved_objects = moved.len();

        // Every reference to a moved object now goes to its new place: roots, then references inside live objects.
        let forward = |value: usize| forwarding.get(&value).copied().unwrap_or(value);
        roots(&mut |slot: &mut usize| *slot = forward(*slot))?;
        let mut live = moved;
        for (start, page) in &self.pages {
            if evacuated.contains(start) || freed.contains(start) {
                continue;
            }
            for address in page.objects()? {
                if page.is_marked(address) {
                    live.push(address);
                }
            }
        }
        for object in live {
            for place in references_in(object)? {
                write_reference(place, forward(read_reference(place)));
            }
        }

        for start in evacuated.iter().chain(&freed) {
            self.pages.remove(start);
        }
        report.freed_pages = evacuated.len() + freed.len();
        self.allocated_since_collection = 0;
        self.live_after_collection = report.live_bytes;
        Ok(report)
    }

    /// Marks every object reachable from the roots, counting each page's live bytes.
    fn mark(
        &mut self,
        roots: &mut RootVisitor<'_>,
        report: &mut CollectionReport,
    ) -> Result<(), HeapError> {
        for page in self.pages.values_mut() {
            page.marks.iter_mut().for_each(|cell| *cell = 0);
            page.live_bytes = 0;
        }
        let mut work = Vec::new();
        roots(&mut |slot: &mut usize| {
            if *slot != 0 {
                work.push(*slot);
            }
        })?;
        while let Some(address) = work.pop() {
            let Some(page) = self.page_of_mut(address) else {
                return Err(HeapError::Internal(format!(
                    "a reference to {address:#x} points outside PlainGC's heap"
                )));
            };
            if !page.mark(address) {
                continue;
            }
            let (_, length) = object_at(address)?;
            page.live_bytes += object_size(length);
            report.live_objects += 1;
            report.live_bytes += object_size(length);
            for place in references_in(address)? {
                let reference = read_reference(place);
                if reference != 0 {
                    work.push(reference);
                }
            }
        }
        Ok(())
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

    fn write_bytes(address: usize, bytes: &[u8]) {
        // SAFETY: tests write into the payload of a `Bytes` object allocated with at least `bytes.len()` bytes.
        unsafe {
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), (address + WORD) as *mut u8, bytes.len())
        };
    }

    fn read_bytes(address: usize) -> Vec<u8> {
        let (_, length) = object_at(address).unwrap_or((ObjectKind::Bytes, 0));
        // SAFETY: as for `write_bytes`, reading the payload back.
        unsafe { std::slice::from_raw_parts((address + WORD) as *const u8, length).to_vec() }
    }

    fn text(heap: &mut Heap, value: &str) -> usize {
        let address = heap.allocate(ObjectKind::Bytes, value.len()).unwrap_or(0);
        write_bytes(address, value.as_bytes());
        address
    }

    /// Collects with `roots` as the only roots.
    fn collect(heap: &mut Heap, roots: &mut [usize], everything: bool) -> CollectionReport {
        let mut visit = |visitor: &mut dyn FnMut(&mut usize)| {
            roots.iter_mut().for_each(&mut *visitor);
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
        let list = heap.allocate(ObjectKind::References, 2 * WORD).unwrap_or(0);
        let a = text(&mut heap, "a");
        let b = text(&mut heap, "b");
        let _garbage = text(&mut heap, "garbage");
        write_reference(list + WORD, a);
        write_reference(list + 2 * WORD, b);
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
    fn moving_rewrites_roots_and_references_inside_objects() {
        let mut heap = Heap::new();
        let list = heap.allocate(ObjectKind::References, 2 * WORD).unwrap_or(0);
        let a = text(&mut heap, "first");
        let b = text(&mut heap, "second");
        write_reference(list + WORD, a);
        write_reference(list + 2 * WORD, b);
        let mut roots = [list, b];
        let report = collect(&mut heap, &mut roots, true);
        assert_eq!(report.moved_objects, 3);
        let [list, b] = roots;
        assert!(heap.holds(list) && heap.holds(b));
        assert_eq!(read_bytes(read_reference(list + WORD)), b"first");
        assert_eq!(read_reference(list + 2 * WORD), b);
        assert_eq!(read_bytes(b), b"second");
    }

    #[test]
    fn compacting_a_sparse_page_keeps_every_value() {
        let mut heap = Heap::new();
        let mut kept = Vec::new();
        for number in 0..50_000 {
            let address = text(&mut heap, &number.to_string());
            if number % 10 == 0 {
                kept.push(address);
            }
        }
        let pages_before = heap.page_count();
        let report = collect(&mut heap, &mut kept, false);
        assert!(report.moved_objects > 0);
        assert!(heap.page_count() < pages_before);
        for (index, &address) in kept.iter().enumerate() {
            assert_eq!(read_bytes(address), (index * 10).to_string().as_bytes());
        }
    }

    #[test]
    fn a_cycle_is_marked_once_and_moved_whole() {
        let mut heap = Heap::new();
        let first = heap.allocate(ObjectKind::References, WORD).unwrap_or(0);
        let second = heap.allocate(ObjectKind::References, WORD).unwrap_or(0);
        write_reference(first + WORD, second);
        write_reference(second + WORD, first);
        let mut roots = [first];
        let report = collect(&mut heap, &mut roots, true);
        assert_eq!(report.live_objects, 2);
        let first = roots[0];
        let second = read_reference(first + WORD);
        assert_eq!(read_reference(second + WORD), first);
    }

    #[test]
    fn large_objects_stay_where_they_are() {
        let mut heap = Heap::new();
        let big = heap
            .allocate(ObjectKind::Bytes, LARGEST_SMALL_OBJECT * 2)
            .unwrap_or(0);
        let mut roots = [big];
        collect(&mut heap, &mut roots, true);
        assert_eq!(roots[0], big);
        assert!(heap.holds(big));
    }

    #[test]
    fn a_reference_outside_the_heap_is_a_bug() {
        let mut heap = Heap::new();
        let mut roots = [0x10usize];
        let mut visit = |visitor: &mut dyn FnMut(&mut usize)| {
            roots.iter_mut().for_each(&mut *visitor);
            Ok(())
        };
        assert!(matches!(
            heap.collect(&mut visit, false),
            Err(HeapError::Internal(_))
        ));
    }
}
