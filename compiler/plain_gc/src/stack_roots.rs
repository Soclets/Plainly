//! Finding roots on the stack through the stack maps Cranelift records (D69): precise, so a number is never mistaken
//! for a reference.
//!
//! PlainCompiler writes a table into every program, in this format, every field a native-endian 64-bit word:
//!
//! ```text
//! version (1)
//! number of safepoints
//! for each safepoint, a call where references are live:
//!     the address of the procedure the call is in
//!     the call's return address, as an offset from the procedure's start
//!     the frame's size: the distance from its frame pointer down to its stack pointer
//!     the number of reference slots
//!     each slot, as an offset from the stack pointer
//! ```
//!
//! During a call, Cranelift keeps each reference in its slot and reads it back afterwards, so the collector can move
//! an object and rewrite the slot. A collection walks the chain of frame pointers from itself up to the frame of the
//! program's `main`, and for each return address in the table, visits that frame's slots.

use std::collections::HashMap;

/// The table's format version, which PlainCompiler writes first.
pub const TABLE_VERSION: u64 = 1;

/// The longest chain of frames a walk follows before deciding something is wrong.
const MOST_FRAMES: usize = 10_000_000;

/// Where a frame's references are at one call.
pub struct Safepoint {
    frame_size: usize,
    slots: Vec<usize>,
}

/// Every safepoint in the program, by its return address.
#[derive(Default)]
pub struct SafepointTable {
    by_return_address: HashMap<usize, Safepoint>,
}

impl SafepointTable {
    /// Reads the table PlainCompiler wrote.
    ///
    /// # Safety
    ///
    /// `table` must point to a table in the format above, which stays valid while the program runs.
    pub unsafe fn read(table: *const u64) -> Result<SafepointTable, String> {
        if table.is_null() {
            return Err("the program has no stack map table".to_string());
        }
        let mut position = 0;
        let mut next = || {
            // SAFETY: the caller promises a well-formed table, and every read stays within the counts it gives.
            let word = unsafe { table.add(position).read() };
            position += 1;
            word as usize
        };
        if next() as u64 != TABLE_VERSION {
            return Err("the stack map table's version isn't one PlainGC reads".to_string());
        }
        let count = next();
        let mut by_return_address = HashMap::with_capacity(count);
        for _ in 0..count {
            let procedure = next();
            let return_offset = next();
            let frame_size = next();
            let slot_count = next();
            let slots = (0..slot_count).map(|_| next()).collect();
            by_return_address.insert(procedure + return_offset, Safepoint { frame_size, slots });
        }
        Ok(SafepointTable { by_return_address })
    }
}

/// This function's own frame pointer. It's never inlined, so it has a frame of its own whose saved frame pointer is
/// its caller's.
#[inline(never)]
fn frame_pointer() -> usize {
    let pointer: usize;
    // SAFETY: reading the frame pointer register has no effect on anything; Plainly's runtime is built with frame
    // pointers kept (`compiler/.cargo/config.toml`), so it holds this function's frame.
    #[cfg(target_arch = "aarch64")]
    unsafe {
        std::arch::asm!("mov {}, x29", out(reg) pointer, options(nomem, nostack, preserves_flags));
    }
    // SAFETY: as above, for x86-64's frame pointer register.
    #[cfg(target_arch = "x86_64")]
    unsafe {
        std::arch::asm!("mov {}, rbp", out(reg) pointer, options(nomem, nostack, preserves_flags));
    }
    #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
    {
        pointer = 0;
    }
    pointer
}

/// Visits every reference slot in the Plainly frames on the stack, from the caller of this function up to the frame
/// of `main`, whose frame pointer is `main_frame`.
///
/// # Safety
///
/// Every frame from here up to `main_frame` must keep its frame pointer, as Plainly's runtime and compiled code do,
/// and `table` must describe the compiled code on the stack.
pub unsafe fn visit(
    table: &SafepointTable,
    main_frame: usize,
    visitor: &mut dyn FnMut(&mut usize),
) -> Result<(), String> {
    let mut frame = frame_pointer();
    if frame == 0 {
        return Err("this processor's frame pointers can't be read".to_string());
    }
    for _ in 0..MOST_FRAMES {
        // SAFETY: `frame` is a frame pointer on this thread's stack: a frame record of the saved frame pointer,
        // then the return address.
        let (caller, return_address) = unsafe {
            (
                (frame as *const usize).read(),
                ((frame + 8) as *const usize).read(),
            )
        };
        if let Some(safepoint) = table.by_return_address.get(&return_address) {
            let stack_pointer = caller - safepoint.frame_size;
            for &slot in &safepoint.slots {
                // SAFETY: the table says this slot in the caller's frame holds a live reference during this call,
                // and the frame is still on the stack, below `main_frame`.
                visitor(unsafe { &mut *((stack_pointer + slot) as *mut usize) });
            }
        }
        if caller == main_frame {
            return Ok(());
        }
        if caller <= frame {
            return Err(format!(
                "the chain of frames went the wrong way, from {frame:#x} to {caller:#x}"
            ));
        }
        frame = caller;
    }
    Err("the chain of frames never reached the program's main".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_is_read_by_return_address() {
        let table: Vec<u64> = vec![
            TABLE_VERSION,
            2,
            0x1000,
            0x10,
            64,
            2,
            8,
            16,
            0x2000,
            0x20,
            32,
            0,
        ];
        // SAFETY: `table` is a well-formed table that outlives the read.
        let read = unsafe { SafepointTable::read(table.as_ptr()) };
        let read = read.unwrap_or_default();
        assert_eq!(read.by_return_address.len(), 2);
        let safepoint = read.by_return_address.get(&0x1010);
        assert_eq!(
            safepoint.map(|safepoint| (safepoint.frame_size, safepoint.slots.clone())),
            Some((64, vec![8, 16]))
        );
    }

    #[test]
    fn a_table_of_another_version_is_refused() {
        let table: Vec<u64> = vec![99, 0];
        // SAFETY: as above.
        assert!(unsafe { SafepointTable::read(table.as_ptr()) }.is_err());
    }
}
