/*
 * Scope frames and element construction.
 *
 * These helpers act on the stack owned by Builder; they live here so the node
 * dispatch in builder.rs stays readable.  A frame is opened by PushScope* and
 * closed by PopScope, EndInitPopScope or EndInitProvideValuePopScope.
 */

use super::builder::{Builder, Frame, RawElement};
use super::resolve::materialize;
use super::{Element, Property, Value, XAML_NAMESPACE};
use crate::XbfError;
use crate::node::PersistedNode;

impl Builder<'_> {
    pub(super) fn add_namespace(&mut self, prefix: &str, uri: String) {
        let entry = (prefix.to_string(), uri);
        match self.frames.last_mut() {
            Some(frame) => frame.namespaces.push(entry),
            None => self.base_namespaces.push(entry),
        }
    }

    fn namespace(&self, prefix: &str) -> Option<&str> {
        for frame in self.frames.iter().rev() {
            for (candidate, uri) in frame.namespaces.iter().rev() {
                if candidate == prefix {
                    return Some(uri);
                }
            }
        }
        for (candidate, uri) in self.base_namespaces.iter().rev() {
            if candidate == prefix {
                return Some(uri);
            }
        }
        None
    }

    pub(super) fn default_namespace(&self) -> String {
        self.namespace("").unwrap_or("").to_string()
    }

    /*
     * Attach a fresh element to the container of the innermost enclosing
     * scope.  A property scope collects into its pending property instead.
     */
    fn attach(&mut self, index: usize) {
        for frame in self.frames.iter_mut().rev() {
            if let Some(pending) = frame.property.as_mut() {
                pending.values.push(index);
                return;
            }
            if let Some(parent) = frame.element {
                self.raw[parent].children.push(index);
                return;
            }
        }
        self.roots.push(index);
    }

    fn make_element(&mut self, ty: PersistedNode, value: Option<Value>, offset: u32) -> usize {
        let default = self.default_namespace();
        let (type_name, namespace) = self.resolve_type(ty, &default);
        let mut properties = Vec::new();
        if let Some(value) = value {
            /*
             * PushScopeCreateTypeWithConstant initializes the object itself;
             * there is no property name on the wire, so keep the constant
             * under the conventional Value name.
             */
            properties.push(Property {
                name: "Value".to_string(),
                namespace: namespace.clone(),
                value,
            });
        }
        let index = self.raw.len();
        self.raw.push(RawElement {
            type_name,
            namespace,
            properties,
            children: Vec::new(),
            custom: None,
            custom_substream: None,
            explicit_keys: Vec::new(),
            implicit_keys: Vec::new(),
            explicit_key: None,
            source_offset: offset,
        });
        self.attach(index);
        index
    }

    /* CreateType*: create in the current scope and remember its object. */
    pub(super) fn create_object(
        &mut self,
        ty: PersistedNode,
        value: Option<Value>,
        offset: u32,
    ) -> usize {
        let index = self.make_element(ty, value, offset);
        if let Some(frame) = self.frames.last_mut()
            && frame.property.is_none()
            && frame.element.is_none()
        {
            frame.element = Some(index);
        }
        index
    }

    /* PushScopeCreateType*: create in the parent scope, then open a scope. */
    pub(super) fn open_object(&mut self, ty: PersistedNode, value: Option<Value>, offset: u32) {
        let index = self.make_element(ty, value, offset);
        self.frames.push(Frame {
            element: Some(index),
            property: None,
            namespaces: Vec::new(),
        });
    }

    pub(super) fn current_element(&self) -> Option<usize> {
        self.frames.iter().rev().find_map(|frame| frame.element)
    }

    pub(super) fn set_property(
        &mut self,
        index: usize,
        kind: &'static str,
        name: String,
        namespace: String,
        value: Value,
    ) -> Result<(), XbfError> {
        let Some(owner) = self.current_element() else {
            return Err(self.error(index, kind, "property has no current element"));
        };
        self.raw[owner].properties.push(Property {
            name,
            namespace,
            value,
        });
        Ok(())
    }

    pub(super) fn set_directive(
        &mut self,
        index: usize,
        kind: &'static str,
        name: &str,
        value: Value,
    ) -> Result<(), XbfError> {
        let Some(owner) = self.current_element() else {
            return Err(self.error(index, kind, "directive has no current element"));
        };
        self.raw[owner].properties.push(Property {
            name: name.to_string(),
            namespace: XAML_NAMESPACE.to_string(),
            value,
        });
        Ok(())
    }

    pub(super) fn set_last_key(
        &mut self,
        index: usize,
        kind: &'static str,
        value: Value,
    ) -> Result<(), XbfError> {
        /*
         * A dictionary entry is closed before its key arrives (EndInitPopScope, then
         * AddToDictionaryWithKey), so the key names the element that scope closed;
         * inside an open dictionary scope it names the last child of that element.
         */
        let target = match self.current_element() {
            Some(owner) => self.raw[owner].children.last().copied().unwrap_or(owner),
            None => self.last_closed.ok_or_else(|| {
                self.error(index, kind, "dictionary key has no current element")
            })?,
        };
        self.raw[target].properties.push(Property {
            name: "x:Key".to_string(),
            namespace: XAML_NAMESPACE.to_string(),
            value,
        });
        Ok(())
    }

    pub(super) fn pop(&mut self, index: usize, kind: &'static str) -> Result<(), XbfError> {
        let Some(frame) = self.frames.pop() else {
            return Err(self.error(index, kind, "scope closer has no open scope"));
        };
        self.close_frame(frame);
        Ok(())
    }

    /*
     * The closer of a scope whose object is a value: after it, the object it closed is the
     * operand that SetValue or AddToCollection spends.
     */
    pub(super) fn pop_object(&mut self, index: usize, kind: &'static str) -> Result<(), XbfError> {
        let Some(frame) = self.frames.pop() else {
            return Err(self.error(index, kind, "scope closer has no open scope"));
        };
        let element = if frame.property.is_some() { None } else { frame.element };
        self.close_frame(frame);
        self.last_closed = element;
        if let Some(element) = element {
            self.values.push(Value::Element(Box::new(materialize(&self.raw, element))));
        }
        Ok(())
    }

    fn close_frame(&mut self, frame: Frame) {
        let Some(pending) = frame.property else {
            return;
        };
        let values: Vec<Element> = pending
            .values
            .iter()
            .map(|&value| materialize(&self.raw, value))
            .collect();
        let mut iter = values.into_iter();
        let value = match (iter.next(), iter.next()) {
            (None, _) => Value::None,
            (Some(one), None) => Value::Element(Box::new(one)),
            (Some(one), Some(two)) => {
                let mut rest = vec![one, two];
                rest.extend(iter);
                Value::Elements(rest)
            }
        };
        self.raw[pending.owner].properties.push(Property {
            name: pending.name,
            namespace: pending.namespace,
            value,
        });
    }
}
