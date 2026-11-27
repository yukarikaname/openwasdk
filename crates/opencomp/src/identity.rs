//! Objects by identity.
//!
//! A client hands this DLL a COM pointer and expects the state behind it back. The
//! identity of an object is the `IUnknown` pointer, the one pointer every interface
//! of the object agrees on; the table holds weak references, so an object that dies
//! leaks nothing.
//!
//! Values are type-erased because a visual and an animation both need this and the
//! lookup knows which one it wants.

use std::any::Any;
use std::sync::{Arc, Mutex, OnceLock, Weak};

use windows_core::{IUnknown, Interface};

/// A weak handle to whatever the DLL keeps for an object.
type Entry = (usize, Weak<dyn Any + Send + Sync>);

static REGISTRY: OnceLock<Mutex<Vec<Entry>>> = OnceLock::new();

fn registry() -> &'static Mutex<Vec<Entry>> {
    REGISTRY.get_or_init(|| Mutex::new(Vec::new()))
}

/// The identity of a COM object, normalised through `IUnknown`.
fn key<T: Interface>(object: &T) -> Option<usize> {
    object
        .cast::<IUnknown>()
        .ok()
        .map(|unknown| unknown.as_raw() as usize)
}

/// Make `value` findable by `object`.
pub fn publish<T: Interface, V: Any + Send + Sync>(object: &T, value: &Arc<V>) {
    let Some(key) = key(object) else { return };
    let value: Arc<dyn Any + Send + Sync> = value.clone();
    let mut table = registry().lock().unwrap();
    if table.len() >= 64 {
        table.retain(|(_, value)| value.strong_count() > 0);
    }
    table.retain(|(existing, _)| *existing != key);
    table.push((key, Arc::downgrade(&value)));
}

/// The value behind `object`, if this DLL made it and it is of type `V`.
pub fn lookup<T: Interface, V: Any + Send + Sync>(object: &T) -> Option<Arc<V>> {
    let key = key(object)?;
    let table = registry().lock().unwrap();
    let value = table
        .iter()
        .find(|(existing, _)| *existing == key)
        .and_then(|(_, value)| value.upgrade())?;
    value.downcast::<V>().ok()
}
