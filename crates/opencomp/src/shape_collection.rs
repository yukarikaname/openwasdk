//! `ShapeVisual.Shapes`: a live `IVector<CompositionShape>` shared with the node.
//!
//! The shape visual and every collection it hands out share one list, so a shape appended
//! before the next frame reaches the draw list. The iterator is a window over that list and
//! clones on read, which is the shape the ABI promises: writing through the vector does not
//! invalidate a current item.

use std::sync::{Arc, Mutex};

use crate::bindings::Microsoft::UI::Composition::{
    CompositionShape, CompositionShapeCollection, ICompositionObject, ICompositionObject2,
    ICompositionObject4, ICompositionObject5,
};
use crate::object::composition_object;
use windows_collections::IIterator;
use windows_core::{Error, HRESULT, Interface, Ref, Result};

const E_BOUNDS: HRESULT = HRESULT(0x8000_000bu32 as i32);
const E_INVALIDARG: HRESULT = HRESULT(0x8007_0057u32 as i32);

/// The `IVector` behind `ShapeVisual.Shapes`, shared with the node the snapshot reads.
pub(crate) fn collection(
    shapes: Arc<Mutex<Vec<CompositionShape>>>,
) -> Result<CompositionShapeCollection> {
    let object: windows_collections::IVector<CompositionShape> =
        ShapeCollectionObject { shapes }.into();
    object.cast()
}

#[windows_core::implement(
    ICompositionObject,
    ICompositionObject2,
    ICompositionObject4,
    ICompositionObject5,
    windows_collections::IVector<CompositionShape>,
    windows_collections::IIterable<CompositionShape>
)]
pub struct ShapeCollectionObject {
    shapes: Arc<Mutex<Vec<CompositionShape>>>,
}

composition_object!(ShapeCollectionObject_Impl);

impl windows_collections::IIterable_Impl<CompositionShape> for ShapeCollectionObject_Impl {
    fn First(&self) -> Result<IIterator<CompositionShape>> {
        let object: IIterator<CompositionShape> = ShapeIteratorObject {
            shapes: self.shapes.clone(),
            current: Mutex::new(0),
        }
        .into();
        Ok(object)
    }
}

impl windows_collections::IVector_Impl<CompositionShape> for ShapeCollectionObject_Impl {
    fn GetAt(&self, index: u32) -> Result<CompositionShape> {
        self.shapes
            .lock()
            .unwrap()
            .get(index as usize)
            .cloned()
            .ok_or_else(|| Error::from_hresult(E_BOUNDS))
    }

    fn Size(&self) -> Result<u32> {
        Ok(self.shapes.lock().unwrap().len() as u32)
    }

    fn GetView(&self) -> Result<windows_collections::IVectorView<CompositionShape>> {
        let shapes: Vec<_> = self
            .shapes
            .lock()
            .unwrap()
            .iter()
            .cloned()
            .map(Some)
            .collect();
        Ok(windows_collections::IVectorView::from(shapes))
    }

    fn IndexOf(&self, value: Ref<CompositionShape>, index: &mut u32) -> Result<bool> {
        let value = value
            .as_ref()
            .ok_or_else(|| Error::from_hresult(E_INVALIDARG))?;
        match self
            .shapes
            .lock()
            .unwrap()
            .iter()
            .position(|shape| shape == value)
        {
            Some(at) => {
                *index = at as u32;
                Ok(true)
            }
            None => {
                *index = 0;
                Ok(false)
            }
        }
    }

    fn SetAt(&self, index: u32, value: Ref<CompositionShape>) -> Result<()> {
        let value = value
            .as_ref()
            .ok_or_else(|| Error::from_hresult(E_INVALIDARG))?;
        let mut shapes = self.shapes.lock().unwrap();
        let slot = shapes
            .get_mut(index as usize)
            .ok_or_else(|| Error::from_hresult(E_BOUNDS))?;
        *slot = value.clone();
        Ok(())
    }

    fn InsertAt(&self, index: u32, value: Ref<CompositionShape>) -> Result<()> {
        let value = value
            .as_ref()
            .ok_or_else(|| Error::from_hresult(E_INVALIDARG))?;
        let mut shapes = self.shapes.lock().unwrap();
        if index as usize > shapes.len() {
            return Err(Error::from_hresult(E_BOUNDS));
        }
        shapes.insert(index as usize, value.clone());
        Ok(())
    }

    fn RemoveAt(&self, index: u32) -> Result<()> {
        let mut shapes = self.shapes.lock().unwrap();
        if index as usize >= shapes.len() {
            return Err(Error::from_hresult(E_BOUNDS));
        }
        shapes.remove(index as usize);
        Ok(())
    }

    fn Append(&self, value: Ref<CompositionShape>) -> Result<()> {
        let value = value
            .as_ref()
            .ok_or_else(|| Error::from_hresult(E_INVALIDARG))?;
        self.shapes.lock().unwrap().push(value.clone());
        Ok(())
    }

    fn RemoveAtEnd(&self) -> Result<()> {
        if self.shapes.lock().unwrap().pop().is_none() {
            return Err(Error::from_hresult(E_BOUNDS));
        }
        Ok(())
    }

    fn Clear(&self) -> Result<()> {
        self.shapes.lock().unwrap().clear();
        Ok(())
    }

    fn GetMany(&self, start_index: u32, items: &mut [Option<CompositionShape>]) -> Result<u32> {
        let shapes = self.shapes.lock().unwrap();
        let start = start_index as usize;
        let count = shapes.len().saturating_sub(start).min(items.len());
        for (slot, shape) in items.iter_mut().take(count).zip(&shapes[start..]) {
            *slot = Some(shape.clone());
        }
        Ok(count as u32)
    }

    fn ReplaceAll(&self, items: &[Option<CompositionShape>]) -> Result<()> {
        let mut shapes = self.shapes.lock().unwrap();
        *shapes = items.iter().flatten().cloned().collect();
        Ok(())
    }
}

/// The iterator `First` hands out over the same shape list.
#[windows_core::implement(windows_collections::IIterator<CompositionShape>)]
pub struct ShapeIteratorObject {
    shapes: Arc<Mutex<Vec<CompositionShape>>>,
    current: Mutex<u32>,
}

impl windows_collections::IIterator_Impl<CompositionShape> for ShapeIteratorObject_Impl {
    fn Current(&self) -> Result<CompositionShape> {
        let index = *self.current.lock().unwrap() as usize;
        self.shapes
            .lock()
            .unwrap()
            .get(index)
            .cloned()
            .ok_or_else(|| Error::from_hresult(E_BOUNDS))
    }

    fn HasCurrent(&self) -> Result<bool> {
        let index = *self.current.lock().unwrap() as usize;
        Ok(index < self.shapes.lock().unwrap().len())
    }

    fn MoveNext(&self) -> Result<bool> {
        let mut current = self.current.lock().unwrap();
        let len = self.shapes.lock().unwrap().len() as u32;
        if *current < len {
            *current += 1;
        }
        Ok(*current < len)
    }

    fn GetMany(&self, items: &mut [Option<CompositionShape>]) -> Result<u32> {
        let mut current = self.current.lock().unwrap();
        let shapes = self.shapes.lock().unwrap();
        let start = *current as usize;
        let count = shapes.len().saturating_sub(start).min(items.len());
        for (slot, shape) in items.iter_mut().take(count).zip(&shapes[start..]) {
            *slot = Some(shape.clone());
        }
        *current += count as u32;
        Ok(count as u32)
    }
}
