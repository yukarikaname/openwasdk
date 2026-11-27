//! `CompositionPropertySet`, the key/value store animations and expressions read from.
//!
//! A client inserts typed values by name and reads them back with a `TryGet*` pair, so the store
//! only has to keep a name-to-value map and answer honestly when the name or the type misses.
//! Colours are held in linear rgba, the same form the renderer uses, and converted at the ABI.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use crate::bindings::Microsoft::UI::Composition::{
    CompositionGetValueStatus, ICompositionObject, ICompositionObject2, ICompositionObject4,
    ICompositionObject5, ICompositionPropertySet, ICompositionPropertySet_Impl,
};
use crate::bindings::Windows::UI::Color;
use crate::brush::{from_rgba, to_rgba};
use crate::object::composition_object;
use crate::object::prelude::*;
use windows_core::{Interface, Result};
use windows_numerics::{Matrix3x2, Vector4};

/// One stored value, tagged with the type it was inserted as.
enum Value {
    Color([f32; 4]),
    Matrix3x2(Matrix3x2),
    Matrix4x4(Matrix4x4),
    Quaternion(Quaternion),
    Scalar(f32),
    Vector2(Vector2),
    Vector3(Vector3),
    Vector4(Vector4),
}

#[windows_core::implement(
    ICompositionObject,
    ICompositionObject2,
    ICompositionObject4,
    ICompositionObject5,
    ICompositionPropertySet
)]
pub struct PropertySetObject {
    values: Arc<Mutex<HashMap<String, Value>>>,
}

/// One map per object that asked for its property set, keyed by the address of the
/// object's implementation. `ICompositionObject.Properties` has to be the same store
/// every time it is read, or a value written through one call is lost to the next.
///
/// The table holds the maps, not weak references to them: the handle a client gets
/// back is destroyed with the call, so a weak entry would die before the next read.
/// Nothing tells this DLL when an object is gone, so the table is capped instead and
/// the least recently used map is dropped; that costs the values on an object nobody
/// has asked about since 256 others did, which is the cheapest honest limit.
type Shared = Arc<Mutex<HashMap<String, Value>>>;
type Entry = (usize, Shared);
const CAPACITY: usize = 256;
static SETS: OnceLock<Mutex<Vec<Entry>>> = OnceLock::new();

fn sets() -> &'static Mutex<Vec<Entry>> {
    SETS.get_or_init(|| Mutex::new(Vec::new()))
}

/// The property set of the object at `key`, created on first use.
pub fn for_key(key: usize) -> Result<CompositionPropertySet> {
    let shared = {
        let mut table = sets().lock().unwrap();
        let found = table.iter().position(|(existing, _)| *existing == key);
        match found {
            Some(at) => {
                // Move to the back, so the front of the list is the coldest.
                let entry = table.remove(at);
                let values = entry.1.clone();
                table.push(entry);
                values
            }
            None => {
                if table.len() >= CAPACITY {
                    table.remove(0);
                }
                let values: Shared = Arc::new(Mutex::new(HashMap::new()));
                table.push((key, values.clone()));
                values
            }
        }
    };
    let set: ICompositionPropertySet = PropertySetObject { values: shared }.into();
    set.cast()
}

composition_object!(PropertySetObject_Impl);

/// The map key. `HSTRING` is not cheap to hash repeatedly, so the name is copied once per call.
fn key(name: &HSTRING) -> String {
    name.to_string_lossy()
}

impl ICompositionPropertySet_Impl for PropertySetObject_Impl {
    fn InsertColor(&self, propertyName: &HSTRING, value: &Color) -> Result<()> {
        self.values
            .lock()
            .unwrap()
            .insert(key(propertyName), Value::Color(to_rgba(*value)));
        Ok(())
    }

    fn InsertMatrix3x2(&self, propertyName: &HSTRING, value: &Matrix3x2) -> Result<()> {
        self.values
            .lock()
            .unwrap()
            .insert(key(propertyName), Value::Matrix3x2(*value));
        Ok(())
    }

    fn InsertMatrix4x4(&self, propertyName: &HSTRING, value: &Matrix4x4) -> Result<()> {
        self.values
            .lock()
            .unwrap()
            .insert(key(propertyName), Value::Matrix4x4(*value));
        Ok(())
    }

    fn InsertQuaternion(&self, propertyName: &HSTRING, value: &Quaternion) -> Result<()> {
        self.values
            .lock()
            .unwrap()
            .insert(key(propertyName), Value::Quaternion(*value));
        Ok(())
    }

    fn InsertScalar(&self, propertyName: &HSTRING, value: f32) -> Result<()> {
        self.values
            .lock()
            .unwrap()
            .insert(key(propertyName), Value::Scalar(value));
        Ok(())
    }

    fn InsertVector2(&self, propertyName: &HSTRING, value: &Vector2) -> Result<()> {
        self.values
            .lock()
            .unwrap()
            .insert(key(propertyName), Value::Vector2(*value));
        Ok(())
    }

    fn InsertVector3(&self, propertyName: &HSTRING, value: &Vector3) -> Result<()> {
        self.values
            .lock()
            .unwrap()
            .insert(key(propertyName), Value::Vector3(*value));
        Ok(())
    }

    fn InsertVector4(&self, propertyName: &HSTRING, value: &Vector4) -> Result<()> {
        self.values
            .lock()
            .unwrap()
            .insert(key(propertyName), Value::Vector4(*value));
        Ok(())
    }

    // A miss leaves the caller's out-parameter untouched, which is what the documented contract
    // promises: the status, not the buffer, says whether there is a value.

    fn TryGetColor(
        &self,
        propertyName: &HSTRING,
        value: &mut Color,
    ) -> Result<CompositionGetValueStatus> {
        match self.values.lock().unwrap().get(&key(propertyName)) {
            None => Ok(CompositionGetValueStatus::NotFound),
            Some(Value::Color(color)) => {
                *value = from_rgba(*color);
                Ok(CompositionGetValueStatus::Succeeded)
            }
            Some(_) => Ok(CompositionGetValueStatus::TypeMismatch),
        }
    }

    fn TryGetMatrix3x2(
        &self,
        propertyName: &HSTRING,
        value: &mut Matrix3x2,
    ) -> Result<CompositionGetValueStatus> {
        match self.values.lock().unwrap().get(&key(propertyName)) {
            None => Ok(CompositionGetValueStatus::NotFound),
            Some(Value::Matrix3x2(matrix)) => {
                *value = *matrix;
                Ok(CompositionGetValueStatus::Succeeded)
            }
            Some(_) => Ok(CompositionGetValueStatus::TypeMismatch),
        }
    }

    fn TryGetMatrix4x4(
        &self,
        propertyName: &HSTRING,
        value: &mut Matrix4x4,
    ) -> Result<CompositionGetValueStatus> {
        match self.values.lock().unwrap().get(&key(propertyName)) {
            None => Ok(CompositionGetValueStatus::NotFound),
            Some(Value::Matrix4x4(matrix)) => {
                *value = *matrix;
                Ok(CompositionGetValueStatus::Succeeded)
            }
            Some(_) => Ok(CompositionGetValueStatus::TypeMismatch),
        }
    }

    fn TryGetQuaternion(
        &self,
        propertyName: &HSTRING,
        value: &mut Quaternion,
    ) -> Result<CompositionGetValueStatus> {
        match self.values.lock().unwrap().get(&key(propertyName)) {
            None => Ok(CompositionGetValueStatus::NotFound),
            Some(Value::Quaternion(quaternion)) => {
                *value = *quaternion;
                Ok(CompositionGetValueStatus::Succeeded)
            }
            Some(_) => Ok(CompositionGetValueStatus::TypeMismatch),
        }
    }

    fn TryGetScalar(
        &self,
        propertyName: &HSTRING,
        value: &mut f32,
    ) -> Result<CompositionGetValueStatus> {
        match self.values.lock().unwrap().get(&key(propertyName)) {
            None => Ok(CompositionGetValueStatus::NotFound),
            Some(Value::Scalar(scalar)) => {
                *value = *scalar;
                Ok(CompositionGetValueStatus::Succeeded)
            }
            Some(_) => Ok(CompositionGetValueStatus::TypeMismatch),
        }
    }

    fn TryGetVector2(
        &self,
        propertyName: &HSTRING,
        value: &mut Vector2,
    ) -> Result<CompositionGetValueStatus> {
        match self.values.lock().unwrap().get(&key(propertyName)) {
            None => Ok(CompositionGetValueStatus::NotFound),
            Some(Value::Vector2(vector)) => {
                *value = *vector;
                Ok(CompositionGetValueStatus::Succeeded)
            }
            Some(_) => Ok(CompositionGetValueStatus::TypeMismatch),
        }
    }

    fn TryGetVector3(
        &self,
        propertyName: &HSTRING,
        value: &mut Vector3,
    ) -> Result<CompositionGetValueStatus> {
        match self.values.lock().unwrap().get(&key(propertyName)) {
            None => Ok(CompositionGetValueStatus::NotFound),
            Some(Value::Vector3(vector)) => {
                *value = *vector;
                Ok(CompositionGetValueStatus::Succeeded)
            }
            Some(_) => Ok(CompositionGetValueStatus::TypeMismatch),
        }
    }

    fn TryGetVector4(
        &self,
        propertyName: &HSTRING,
        value: &mut Vector4,
    ) -> Result<CompositionGetValueStatus> {
        match self.values.lock().unwrap().get(&key(propertyName)) {
            None => Ok(CompositionGetValueStatus::NotFound),
            Some(Value::Vector4(vector)) => {
                *value = *vector;
                Ok(CompositionGetValueStatus::Succeeded)
            }
            Some(_) => Ok(CompositionGetValueStatus::TypeMismatch),
        }
    }
}

pub fn create() -> Result<CompositionPropertySet> {
    let set: ICompositionPropertySet = PropertySetObject {
        values: Arc::new(Mutex::new(HashMap::new())),
    }
    .into();
    set.cast()
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use crate::bindings::Microsoft::UI::Composition::ICompositor;

    fn property_set() -> ICompositionPropertySet {
        let compositor: ICompositor = crate::compositor::instance().unwrap().cast().unwrap();
        let set = compositor.CreatePropertySet().unwrap();
        set.cast().unwrap()
    }

    #[test]
    fn every_value_kind_round_trips() {
        let set = property_set();

        let color = Color {
            a: 255,
            r: 0,
            g: 128,
            b: 255,
        };
        set.InsertColor("color", color).unwrap();
        let mut read = Color::default();
        assert_eq!(
            set.TryGetColor("color", &mut read).unwrap(),
            CompositionGetValueStatus::Succeeded
        );
        assert_eq!(read, color);

        let matrix3x2 = Matrix3x2 {
            m11: 1.0,
            m12: 2.0,
            m21: 3.0,
            m22: 4.0,
            m31: 5.0,
            m32: 6.0,
        };
        set.InsertMatrix3x2("matrix3x2", matrix3x2).unwrap();
        let mut read = Matrix3x2::default();
        assert_eq!(
            set.TryGetMatrix3x2("matrix3x2", &mut read).unwrap(),
            CompositionGetValueStatus::Succeeded
        );
        assert_eq!(read, matrix3x2);

        let matrix4x4 = Matrix4x4 {
            m11: 1.0,
            m12: 2.0,
            m13: 3.0,
            m14: 4.0,
            ..Matrix4x4::default()
        };
        set.InsertMatrix4x4("matrix4x4", matrix4x4).unwrap();
        let mut read = Matrix4x4::default();
        assert_eq!(
            set.TryGetMatrix4x4("matrix4x4", &mut read).unwrap(),
            CompositionGetValueStatus::Succeeded
        );
        assert_eq!(read, matrix4x4);

        let quaternion = Quaternion {
            x: 0.0,
            y: 0.5,
            z: 0.0,
            w: 0.5,
        };
        set.InsertQuaternion("quaternion", quaternion).unwrap();
        let mut read = Quaternion::default();
        assert_eq!(
            set.TryGetQuaternion("quaternion", &mut read).unwrap(),
            CompositionGetValueStatus::Succeeded
        );
        assert_eq!(read, quaternion);

        set.InsertScalar("scalar", 0.25).unwrap();
        let mut read = 0.0f32;
        assert_eq!(
            set.TryGetScalar("scalar", &mut read).unwrap(),
            CompositionGetValueStatus::Succeeded
        );
        assert_eq!(read, 0.25);

        let vector2 = Vector2 { x: 1.0, y: 2.0 };
        set.InsertVector2("vector2", vector2).unwrap();
        let mut read = Vector2::default();
        assert_eq!(
            set.TryGetVector2("vector2", &mut read).unwrap(),
            CompositionGetValueStatus::Succeeded
        );
        assert_eq!(read, vector2);

        let vector3 = Vector3 {
            x: 1.0,
            y: 2.0,
            z: 3.0,
        };
        set.InsertVector3("vector3", vector3).unwrap();
        let mut read = Vector3::default();
        assert_eq!(
            set.TryGetVector3("vector3", &mut read).unwrap(),
            CompositionGetValueStatus::Succeeded
        );
        assert_eq!(read, vector3);

        let vector4 = Vector4 {
            x: 1.0,
            y: 2.0,
            z: 3.0,
            w: 4.0,
        };
        set.InsertVector4("vector4", vector4).unwrap();
        let mut read = Vector4::default();
        assert_eq!(
            set.TryGetVector4("vector4", &mut read).unwrap(),
            CompositionGetValueStatus::Succeeded
        );
        assert_eq!(read, vector4);
    }

    #[test]
    fn a_missing_name_and_a_wrong_type_are_reported() {
        let set = property_set();
        set.InsertScalar("scalar", 1.0).unwrap();

        let mut scalar = 0.0f32;
        assert_eq!(
            set.TryGetScalar("absent", &mut scalar).unwrap(),
            CompositionGetValueStatus::NotFound
        );
        let mut vector2 = Vector2::default();
        assert_eq!(
            set.TryGetVector2("scalar", &mut vector2).unwrap(),
            CompositionGetValueStatus::TypeMismatch
        );
    }
}
