//! The `IContentIslandStatics` lookups.
//!
//! Two islands, each made on its own thread from its own compositor, make every
//! lookup discriminating: by id, by visual, by compositor and by creator thread the
//! one that must be found is present and the other is not.
//!
//! **This test has not been run.** The build host is macOS, so `content` is not
//! compiled here; the Windows target is only type-checked.

use std::thread;

use crate::bindings::Microsoft::UI::Composition::{Compositor, ContainerVisual, Visual};
use crate::bindings::Microsoft::UI::Content::ContentIsland;
use crate::content::island::{
    find_all_for_compositor, find_all_for_current_thread, get_by_id, get_by_visual,
    island_from_visual,
};
use windows_core::Interface;

struct Fixture {
    compositor: Compositor,
    root: Visual,
    island: ContentIsland,
}

fn fixture() -> Fixture {
    let compositor: Compositor = crate::compositor::instance().unwrap().cast().unwrap();
    let root: Visual = compositor.CreateContainerVisual().unwrap().cast().unwrap();
    let island = island_from_visual(&root).unwrap();
    Fixture {
        compositor,
        root,
        island,
    }
}

fn ids(islands: &windows_core::Array<ContentIsland>) -> Vec<u64> {
    islands
        .iter()
        .filter_map(|island| island.as_ref())
        .map(|island| island.Id().unwrap())
        .collect()
}

#[test]
fn each_lookup_finds_the_island_it_names_and_not_the_other() {
    let main = fixture();

    // A second thread creates the other island, so `FindAllForCurrentThread` has
    // two different creator threads to tell apart.
    let other = thread::spawn(|| {
        let fixture = fixture();
        let found = find_all_for_current_thread().unwrap();
        assert_eq!(ids(&found), vec![fixture.island.Id().unwrap()]);
        fixture
    })
    .join()
    .unwrap();

    let main_id = main.island.Id().unwrap();
    let other_id = other.island.Id().unwrap();

    // By creator thread: this thread made only the first island.
    assert_eq!(ids(&find_all_for_current_thread().unwrap()), vec![main_id]);

    // By compositor: each island's root visual came from its own compositor.
    assert_eq!(
        ids(&find_all_for_compositor(Some(&main.compositor)).unwrap()),
        vec![main_id]
    );
    assert_eq!(
        ids(&find_all_for_compositor(Some(&other.compositor)).unwrap()),
        vec![other_id]
    );

    // By visual: the root names its own island, and a descendant of the root
    // still names the island that contains it.
    assert_eq!(
        get_by_visual(Some(&main.root)).unwrap().Id().unwrap(),
        main_id
    );
    assert_eq!(
        get_by_visual(Some(&other.root)).unwrap().Id().unwrap(),
        other_id
    );
    let container: ContainerVisual = main.root.cast().unwrap();
    let child: Visual = main
        .compositor
        .CreateContainerVisual()
        .unwrap()
        .cast()
        .unwrap();
    container.Children().unwrap().InsertAtTop(&child).unwrap();
    assert_eq!(get_by_visual(Some(&child)).unwrap().Id().unwrap(), main_id);

    // By id: each id answers its own island, not the other.
    assert_eq!(get_by_id(main_id).unwrap().Id().unwrap(), main_id);
    assert_eq!(get_by_id(other_id).unwrap().Id().unwrap(), other_id);
    assert!(get_by_id(u64::MAX).is_err(), "no island has that id");
}
