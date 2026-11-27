//! The live islands, for the four `IContentIslandStatics` lookups.
//!
//! Ownership is weak on purpose: the table holds a `windows_core::Weak<IContentIsland>`,
//! so the registry never keeps an island alive. A lookup upgrades the weak handle; a
//! lookup that loses the race finds a dead entry, prunes it, and answers with nothing
//! (an empty array, or `E_NOTIMPL` where the ABI has no null) rather than a dangling
//! pointer. The compositor, creator thread, root visual and id are read from the
//! island's state, which the live object keeps alive; the table only holds the id and
//! the weak handle.

use std::sync::{Arc, Mutex, OnceLock};
use std::thread::ThreadId;

use crate::bindings::Microsoft::UI::Composition::Visual;
use crate::bindings::Microsoft::UI::Content::{ContentIsland, IContentIsland};
use crate::clock::Clock;
use crate::node::{self, NodeRef};
use windows_core::{Interface, Weak};

use super::island::IslandState;

/// One island the table has seen. The id is fixed at creation; everything a lookup
/// filters on lives in the state behind the weak handle.
struct Entry {
    id: u64,
    island: Weak<IContentIsland>,
}

static REGISTRY: OnceLock<Mutex<Vec<Entry>>> = OnceLock::new();

fn registry() -> &'static Mutex<Vec<Entry>> {
    REGISTRY.get_or_init(|| Mutex::new(Vec::new()))
}

/// Remember a freshly made island with a weak handle; the object keeps its own state.
pub(crate) fn register(island: &IContentIsland, id: u64) {
    let Ok(weak) = island.downgrade() else {
        return;
    };
    let mut table = registry().lock().unwrap();
    table.retain(|entry| entry.id != id && entry.island.upgrade().is_some());
    table.push(Entry { id, island: weak });
}

/// The state behind a live island. Every island made here has one, and a missing
/// state means the object is not ours; such an entry cannot answer and is skipped.
fn state_of(island: &IContentIsland) -> Option<Arc<Mutex<IslandState>>> {
    crate::identity::lookup::<_, Mutex<IslandState>>(island)
}

/// Every live island whose state `keep` accepts, with dead entries pruned. The table
/// lock is held while the state is read; no code takes the table lock while holding
/// a state, so the two locks are only ever taken in this order.
fn select(mut keep: impl FnMut(u64, &IslandState) -> bool) -> Vec<ContentIsland> {
    let mut table = registry().lock().unwrap();
    table.retain(|entry| entry.island.upgrade().is_some());
    let mut found = Vec::new();
    for entry in table.iter() {
        let Some(island) = entry.island.upgrade() else {
            continue;
        };
        let Some(state) = state_of(&island) else {
            continue;
        };
        if !keep(entry.id, &state.lock().unwrap()) {
            continue;
        }
        if let Ok(island) = island.cast::<ContentIsland>() {
            found.push(island);
        }
    }
    found
}

/// Islands created on `thread`.
pub(crate) fn for_thread(thread: ThreadId) -> Vec<ContentIsland> {
    select(|_, state| state.thread == thread)
}

/// Islands rooted in a visual of `clock`'s compositor.
pub(crate) fn for_clock(clock: &Arc<Clock>) -> Vec<ContentIsland> {
    select(|_, state| Arc::ptr_eq(&state.clock, clock))
}

/// The island whose tree contains `visual`, if any.
pub(crate) fn by_visual(visual: &Visual) -> Option<ContentIsland> {
    let target = node::resolve(visual)?;
    select(|_, state| contains(&state.node, &target))
        .into_iter()
        .next()
}

/// The island with `id`, if it is still alive.
pub(crate) fn by_id(id: u64) -> Option<ContentIsland> {
    select(|entry_id, _| entry_id == id).into_iter().next()
}

/// Whether `target` is `root` or one of its descendants.
fn contains(root: &NodeRef, target: &NodeRef) -> bool {
    if Arc::ptr_eq(root, target) {
        return true;
    }
    let children = root.lock().unwrap().children.clone();
    children.iter().any(|child| contains(child, target))
}
