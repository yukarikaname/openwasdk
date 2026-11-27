//! The stop and stop-list objects behind a gradient brush.
//!
//! `ColorStops` is a real `IVector<ICompositionColorGradientStop>`: the brush and every
//! collection it hands out share one `Arc<Mutex<Vec<_>>>`, so a stop appended before `SetBrush`
//! reaches the tree. The iterator is a window over the same list and clones on read, which is
//! the shape the ABI promises: writing through the vector does not invalidate a current item.

use std::sync::{Arc, Mutex};

use crate::bindings::Microsoft::UI::Composition::{
    CompositionColorGradientStop, CompositionColorGradientStopCollection,
    ICompositionColorGradientStop, ICompositionColorGradientStop_Impl,
    ICompositionColorGradientStopCollection, ICompositionColorGradientStopCollection_Impl,
    ICompositionObject, ICompositionObject2, ICompositionObject4, ICompositionObject5,
};
use crate::bindings::Windows::UI::Color;
use crate::brush::{from_rgba, to_rgba};
use crate::object::composition_object;
use windows_core::{Error, HRESULT, Interface, Ref, Result};

const E_BOUNDS: HRESULT = HRESULT(0x8000_000bu32 as i32);
const E_INVALIDARG: HRESULT = HRESULT(0x8007_0057u32 as i32);

/// A stop object, not yet attached to a brush.
pub(crate) fn stop(offset: f32, color: Color) -> Result<CompositionColorGradientStop> {
    let object: ICompositionColorGradientStop = GradientStopObject {
        offset: Mutex::new(offset),
        color: Mutex::new(to_rgba(color)),
    }
    .into();
    object.cast()
}

/// The `IVector` a gradient brush hands out from `ColorStops`.
pub(crate) fn collection(
    stops: Arc<Mutex<Vec<CompositionColorGradientStop>>>,
) -> Result<CompositionColorGradientStopCollection> {
    let object: ICompositionColorGradientStopCollection = StopCollectionObject { stops }.into();
    object.cast()
}

#[windows_core::implement(
    ICompositionObject,
    ICompositionObject2,
    ICompositionObject4,
    ICompositionObject5,
    ICompositionColorGradientStop
)]
pub struct GradientStopObject {
    offset: Mutex<f32>,
    color: Mutex<[f32; 4]>,
}

composition_object!(GradientStopObject_Impl);

impl ICompositionColorGradientStop_Impl for GradientStopObject_Impl {
    fn Color(&self) -> Result<Color> {
        Ok(from_rgba(*self.color.lock().unwrap()))
    }
    fn SetColor(&self, value: &Color) -> Result<()> {
        *self.color.lock().unwrap() = to_rgba(*value);
        Ok(())
    }
    fn Offset(&self) -> Result<f32> {
        Ok(*self.offset.lock().unwrap())
    }
    fn SetOffset(&self, value: f32) -> Result<()> {
        *self.offset.lock().unwrap() = value;
        Ok(())
    }
}

#[windows_core::implement(
    ICompositionColorGradientStopCollection,
    windows_collections::IVector<CompositionColorGradientStop>,
    windows_collections::IIterable<CompositionColorGradientStop>
)]
pub struct StopCollectionObject {
    stops: Arc<Mutex<Vec<CompositionColorGradientStop>>>,
}

impl ICompositionColorGradientStopCollection_Impl for StopCollectionObject_Impl {}

impl windows_collections::IIterable_Impl<CompositionColorGradientStop>
    for StopCollectionObject_Impl
{
    fn First(&self) -> Result<windows_collections::IIterator<CompositionColorGradientStop>> {
        let object: windows_collections::IIterator<CompositionColorGradientStop> =
            StopIteratorObject {
                stops: self.stops.clone(),
                current: Mutex::new(0),
            }
            .into();
        Ok(object)
    }
}

impl windows_collections::IVector_Impl<CompositionColorGradientStop> for StopCollectionObject_Impl {
    fn GetAt(&self, index: u32) -> Result<CompositionColorGradientStop> {
        self.stops
            .lock()
            .unwrap()
            .get(index as usize)
            .cloned()
            .ok_or_else(|| Error::from_hresult(E_BOUNDS))
    }

    fn Size(&self) -> Result<u32> {
        Ok(self.stops.lock().unwrap().len() as u32)
    }

    fn GetView(&self) -> Result<windows_collections::IVectorView<CompositionColorGradientStop>> {
        let stops: Vec<_> = self
            .stops
            .lock()
            .unwrap()
            .iter()
            .cloned()
            .map(Some)
            .collect();
        Ok(windows_collections::IVectorView::from(stops))
    }

    fn IndexOf(&self, value: Ref<CompositionColorGradientStop>, index: &mut u32) -> Result<bool> {
        let value = value
            .as_ref()
            .ok_or_else(|| Error::from_hresult(E_INVALIDARG))?;
        match self
            .stops
            .lock()
            .unwrap()
            .iter()
            .position(|stop| stop == value)
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

    fn SetAt(&self, index: u32, value: Ref<CompositionColorGradientStop>) -> Result<()> {
        let value = value
            .as_ref()
            .ok_or_else(|| Error::from_hresult(E_INVALIDARG))?;
        let mut stops = self.stops.lock().unwrap();
        let slot = stops
            .get_mut(index as usize)
            .ok_or_else(|| Error::from_hresult(E_BOUNDS))?;
        *slot = value.clone();
        Ok(())
    }

    fn InsertAt(&self, index: u32, value: Ref<CompositionColorGradientStop>) -> Result<()> {
        let value = value
            .as_ref()
            .ok_or_else(|| Error::from_hresult(E_INVALIDARG))?;
        let mut stops = self.stops.lock().unwrap();
        if index as usize > stops.len() {
            return Err(Error::from_hresult(E_BOUNDS));
        }
        stops.insert(index as usize, value.clone());
        Ok(())
    }

    fn RemoveAt(&self, index: u32) -> Result<()> {
        let mut stops = self.stops.lock().unwrap();
        if index as usize >= stops.len() {
            return Err(Error::from_hresult(E_BOUNDS));
        }
        stops.remove(index as usize);
        Ok(())
    }

    fn Append(&self, value: Ref<CompositionColorGradientStop>) -> Result<()> {
        let value = value
            .as_ref()
            .ok_or_else(|| Error::from_hresult(E_INVALIDARG))?;
        self.stops.lock().unwrap().push(value.clone());
        Ok(())
    }

    fn RemoveAtEnd(&self) -> Result<()> {
        if self.stops.lock().unwrap().pop().is_none() {
            return Err(Error::from_hresult(E_BOUNDS));
        }
        Ok(())
    }

    fn Clear(&self) -> Result<()> {
        self.stops.lock().unwrap().clear();
        Ok(())
    }

    fn GetMany(
        &self,
        start_index: u32,
        items: &mut [Option<CompositionColorGradientStop>],
    ) -> Result<u32> {
        let stops = self.stops.lock().unwrap();
        let start = start_index as usize;
        let count = stops.len().saturating_sub(start).min(items.len());
        for (slot, stop) in items.iter_mut().take(count).zip(&stops[start..]) {
            *slot = Some(stop.clone());
        }
        Ok(count as u32)
    }

    fn ReplaceAll(&self, items: &[Option<CompositionColorGradientStop>]) -> Result<()> {
        let mut stops = self.stops.lock().unwrap();
        *stops = items.iter().flatten().cloned().collect();
        Ok(())
    }
}

/// The iterator `First` hands out over the same stop list.
#[windows_core::implement(windows_collections::IIterator<CompositionColorGradientStop>)]
pub struct StopIteratorObject {
    stops: Arc<Mutex<Vec<CompositionColorGradientStop>>>,
    current: Mutex<u32>,
}

impl windows_collections::IIterator_Impl<CompositionColorGradientStop> for StopIteratorObject_Impl {
    fn Current(&self) -> Result<CompositionColorGradientStop> {
        let index = *self.current.lock().unwrap() as usize;
        self.stops
            .lock()
            .unwrap()
            .get(index)
            .cloned()
            .ok_or_else(|| Error::from_hresult(E_BOUNDS))
    }

    fn HasCurrent(&self) -> Result<bool> {
        let index = *self.current.lock().unwrap() as usize;
        Ok(index < self.stops.lock().unwrap().len())
    }

    fn MoveNext(&self) -> Result<bool> {
        let mut current = self.current.lock().unwrap();
        let len = self.stops.lock().unwrap().len() as u32;
        if *current < len {
            *current += 1;
        }
        Ok(*current < len)
    }

    fn GetMany(&self, items: &mut [Option<CompositionColorGradientStop>]) -> Result<u32> {
        let mut current = self.current.lock().unwrap();
        let stops = self.stops.lock().unwrap();
        let start = *current as usize;
        let count = stops.len().saturating_sub(start).min(items.len());
        for (slot, stop) in items.iter_mut().take(count).zip(&stops[start..]) {
            *slot = Some(stop.clone());
        }
        *current += count as u32;
        Ok(count as u32)
    }
}
