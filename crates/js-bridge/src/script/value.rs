use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::fmt;
use std::ptr::NonNull;
use std::rc::Rc;

use mozjs::context::{JSContext, RawJSContext};
use mozjs::conversions::{ToJSValConvertible, jsstr_to_string};
use mozjs::gc::{Handle, MutableHandle, RootedTraceableBox};
use mozjs::jsapi::{self, Heap, JSObject, Value, jsid};
use mozjs::jsval::{BooleanValue, DoubleValue, Int32Value, NullValue, ObjectValue, UndefinedValue};
use mozjs::rooted;
use mozjs::rust::IdVector;
use num_traits::ToPrimitive;
use serde::de::{self, DeserializeOwned, DeserializeSeed, IntoDeserializer, Visitor};
use serde::ser::{self, Serialize};

#[derive(Debug)]
pub struct Error {
    path: String,
    message: String,
}

impl Error {
    fn new(message: impl fmt::Display) -> Self {
        Self {
            path: String::new(),
            message: message.to_string(),
        }
    }

    fn at(mut self, segment: &str) -> Self {
        let separator = if self.path.is_empty() || self.path.starts_with('[') {
            ""
        } else {
            "."
        };
        self.path = format!("{segment}{separator}{}", self.path);
        self
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.path.is_empty() {
            f.write_str(&self.message)
        } else {
            write!(f, "{}: {}", self.path, self.message)
        }
    }
}

impl std::error::Error for Error {}

impl de::Error for Error {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        Self::new(msg)
    }
}

impl ser::Error for Error {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        Self::new(msg)
    }
}

type Result<T, E = Error> = std::result::Result<T, E>;

use deflorta_common::handler::HANDLER;
pub use deflorta_common::handler::Handler;

/// Collects the functions of a tree being read into one JS array (one GC root
/// per commit instead of one per function).
pub struct HandlerSink<'a> {
    generation: u32,
    array: Handle<'a, *mut JSObject>,
    len: Cell<u32>,
}

impl<'a> HandlerSink<'a> {
    pub const fn new(generation: u32, array: Handle<'a, *mut JSObject>) -> Self {
        Self {
            generation,
            array,
            len: Cell::new(0),
        }
    }

    unsafe fn push(&self, cx: *mut RawJSContext, function: Handle<Value>) -> Result<Handler> {
        let index = self.len.get();
        if !unsafe { jsapi::JS_SetElement(cx, self.array.into(), index, function.into()) } {
            return Err(Error::new("cannot store handler"));
        }
        self.len.set(index + 1);
        Ok(Handler {
            generation: self.generation,
            index,
        })
    }
}

type FieldAtoms = HashMap<(usize, usize), Rc<[usize]>>;

thread_local! {
    /// Pinned atoms of struct field names, keyed by the address of the field list.
    static FIELD_ATOMS: RefCell<FieldAtoms> = RefCell::new(HashMap::new());
    static NAME_ATOMS: RefCell<HashMap<&'static str, usize>> = RefCell::new(HashMap::new());
}

/// Forgets cached atoms; call when the runtime that pinned them shuts down.
pub fn clear_atom_cache() {
    FIELD_ATOMS.with(|a| a.borrow_mut().clear());
    NAME_ATOMS.with(|a| a.borrow_mut().clear());
}

/// Property keys are interned atoms, so a key matches a name when the bits are equal.
unsafe fn atom(cx: *mut RawJSContext, name: &'static str) -> Result<usize> {
    if let Some(bits) = NAME_ATOMS.with(|a| a.borrow().get(name).copied()) {
        return Ok(bits);
    }
    let string = unsafe { jsapi::JS_AtomizeAndPinStringN(cx, name.as_ptr().cast(), name.len()) };
    if string.is_null() {
        return Err(Error::new(format!("cannot intern '{name}'")));
    }
    let bits = mozjs::jsid::StringId(string).asBits_;
    NAME_ATOMS.with(|a| a.borrow_mut().insert(name, bits));
    Ok(bits)
}

unsafe fn field_atoms(
    cx: *mut RawJSContext,
    fields: &'static [&'static str],
) -> Result<Rc<[usize]>> {
    let key = (fields.as_ptr() as usize, fields.len());
    if let Some(atoms) = FIELD_ATOMS.with(|a| a.borrow().get(&key).cloned()) {
        return Ok(atoms);
    }
    let atoms: Rc<[usize]> = fields
        .iter()
        .map(|name| unsafe { atom(cx, name) })
        .collect::<Result<_>>()?;
    FIELD_ATOMS.with(|a| a.borrow_mut().insert(key, atoms.clone()));
    Ok(atoms)
}

const fn jsid_from_bits(bits: usize) -> jsid {
    jsid { asBits_: bits }
}

pub unsafe fn from_js<T: DeserializeOwned>(
    cx: *mut RawJSContext,
    value: Handle<Value>,
) -> Result<T> {
    T::deserialize(Deserializer {
        cx,
        value,
        handlers: None,
    })
}

pub unsafe fn from_js_with_handlers<T: DeserializeOwned>(
    cx: *mut RawJSContext,
    value: Handle<Value>,
    sink: &HandlerSink,
) -> Result<T> {
    T::deserialize(Deserializer {
        cx,
        value,
        handlers: Some(sink),
    })
}

struct Deserializer<'a, 's> {
    cx: *mut RawJSContext,
    value: Handle<'a, Value>,
    handlers: Option<&'s HandlerSink<'s>>,
}

impl<'s> Deserializer<'_, 's> {
    fn string(&self) -> String {
        let string = self.value.get().to_string();
        // Strings come from rooted values; non-null by construction.
        let string = NonNull::new(string).expect("string value without a string");
        unsafe {
            let cx = JSContext::from_ptr(NonNull::new_unchecked(self.cx));
            jsstr_to_string(&cx, string)
        }
    }

    fn object_kind(&self) -> Option<ObjectKind> {
        let value = self.value.get();
        if !value.is_object() {
            return None;
        }
        let obj = value.to_object();
        if unsafe { jsapi::IsCallable(obj) } {
            return Some(ObjectKind::Function);
        }
        let cx = self.cx;
        rooted!(in(cx) let obj = obj);
        let mut is_array = false;
        if unsafe { jsapi::IsArrayObject1(cx, obj.handle().into(), &raw mut is_array) } && is_array
        {
            Some(ObjectKind::Array)
        } else {
            Some(ObjectKind::Plain)
        }
    }

    const fn nested<'b>(&self, value: Handle<'b, Value>) -> Deserializer<'b, 's> {
        Deserializer {
            cx: self.cx,
            value,
            handlers: self.handlers,
        }
    }
}

#[derive(PartialEq, Eq)]
enum ObjectKind {
    Plain,
    Array,
    Function,
}

/// Numbers are presented like JSON text: integers as integers, `NaN` and infinities as null.
fn visit_number<'de, V: Visitor<'de>>(n: f64, visitor: V) -> Result<V::Value> {
    if !n.is_finite() {
        return visitor.visit_unit();
    }
    if n.fract() == 0.0 {
        if let Some(u) = n.to_u64() {
            return visitor.visit_u64(u);
        }
        if let Some(i) = n.to_i64() {
            return visitor.visit_i64(i);
        }
    }
    visitor.visit_f64(n)
}

/// Own enumerable string keys of an object, in property order.
unsafe fn own_keys(cx: *mut RawJSContext, obj: Handle<*mut JSObject>) -> Result<IdVector> {
    let mut safe_cx = unsafe { JSContext::from_ptr(NonNull::new_unchecked(cx)) };
    let mut ids = IdVector::new(&mut safe_cx);
    if unsafe { jsapi::GetPropertyKeys(cx, obj.into(), jsapi::JSITER_OWNONLY, ids.handle_mut()) } {
        Ok(ids)
    } else {
        Err(Error::new("cannot list object properties"))
    }
}

impl<'de> de::Deserializer<'de> for Deserializer<'_, '_> {
    type Error = Error;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        let value = self.value.get();
        if value.is_undefined() || value.is_null() {
            visitor.visit_unit()
        } else if value.is_boolean() {
            visitor.visit_bool(value.to_boolean())
        } else if value.is_number() {
            visit_number(value.to_number(), visitor)
        } else if value.is_string() {
            visitor.visit_string(self.string())
        } else {
            match self.object_kind() {
                Some(ObjectKind::Array) => visit_array(&self, visitor),
                Some(ObjectKind::Plain) => visit_object(&self, None, visitor),
                Some(ObjectKind::Function) => Err(Error::new("unexpected function")),
                None => Err(Error::new("unsupported value (symbol or bigint)")),
            }
        }
    }

    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        let value = self.value.get();
        let absent = value.is_undefined()
            || value.is_null()
            || (value.is_number() && !value.to_number().is_finite());
        if absent {
            visitor.visit_none()
        } else {
            visitor.visit_some(self)
        }
    }

    fn deserialize_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value> {
        if self.object_kind() == Some(ObjectKind::Plain) {
            visit_object(&self, Some(fields), visitor)
        } else {
            self.deserialize_any(visitor)
        }
    }

    fn deserialize_newtype_struct<V: Visitor<'de>>(
        self,
        name: &'static str,
        visitor: V,
    ) -> Result<V::Value> {
        if name != HANDLER {
            return visitor.visit_newtype_struct(self);
        }
        if self.object_kind() != Some(ObjectKind::Function) {
            return Err(Error::new("expected a function"));
        }
        let Some(sink) = self.handlers else {
            return Err(Error::new("functions are only accepted in element trees"));
        };
        let handler = unsafe { sink.push(self.cx, self.value)? };
        visitor.visit_u64(handler.to_bits())
    }

    fn deserialize_enum<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value> {
        if self.value.get().is_string() {
            visitor.visit_enum(self.string().into_deserializer())
        } else {
            Err(Error::new("expected a string"))
        }
    }

    fn deserialize_ignored_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        visitor.visit_unit()
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf unit unit_struct seq tuple tuple_struct map identifier
    }
}

fn visit_array<'de, V: Visitor<'de>>(d: &Deserializer, visitor: V) -> Result<V::Value> {
    let cx = d.cx;
    rooted!(in(cx) let obj = d.value.get().to_object());
    let mut len = 0;
    if !unsafe { jsapi::GetArrayLength(cx, obj.handle().into(), &raw mut len) } {
        return Err(Error::new("cannot read array length"));
    }
    rooted!(in(cx) let mut slot = UndefinedValue());
    visitor.visit_seq(ArrayReader {
        d,
        obj: obj.handle(),
        slot: slot.handle_mut(),
        index: 0,
        len,
    })
}

struct ArrayReader<'r, 'a, 's> {
    d: &'r Deserializer<'a, 's>,
    obj: Handle<'r, *mut JSObject>,
    slot: MutableHandle<'r, Value>,
    index: u32,
    len: u32,
}

impl<'de> de::SeqAccess<'de> for ArrayReader<'_, '_, '_> {
    type Error = Error;

    fn next_element_seed<T: DeserializeSeed<'de>>(&mut self, seed: T) -> Result<Option<T::Value>> {
        if self.index >= self.len {
            return Ok(None);
        }
        let index = self.index;
        self.index += 1;
        let cx = self.d.cx;
        if !unsafe { jsapi::JS_GetElement(cx, self.obj.into(), index, self.slot.reborrow().into()) }
        {
            return Err(Error::new("cannot read array element"));
        }
        seed.deserialize(self.d.nested(self.slot.handle()))
            .map(Some)
            .map_err(|e| e.at(&format!("[{index}]")))
    }

    fn size_hint(&self) -> Option<usize> {
        (self.len - self.index).to_usize()
    }
}

/// Visits an object's properties. With `fields` (a struct), keys are matched by
/// atom and unknown properties are skipped without being read.
fn visit_object<'de, V: Visitor<'de>>(
    d: &Deserializer,
    fields: Option<&'static [&'static str]>,
    visitor: V,
) -> Result<V::Value> {
    let cx = d.cx;
    rooted!(in(cx) let obj = d.value.get().to_object());
    let ids = unsafe { own_keys(cx, obj.handle())? };
    let atoms = match fields {
        Some(fields) => Some(unsafe { field_atoms(cx, fields)? }),
        None => None,
    };
    rooted!(in(cx) let mut slot = UndefinedValue());
    visitor.visit_map(ObjectReader {
        d,
        obj: obj.handle(),
        slot: slot.handle_mut(),
        ids,
        pos: 0,
        fields: fields.zip(atoms),
        key: String::new(),
    })
}

struct ObjectReader<'r, 'a, 's> {
    d: &'r Deserializer<'a, 's>,
    obj: Handle<'r, *mut JSObject>,
    slot: MutableHandle<'r, Value>,
    ids: IdVector,
    pos: usize,
    fields: Option<(&'static [&'static str], Rc<[usize]>)>,
    key: String,
}

impl ObjectReader<'_, '_, '_> {
    /// Reads property `id` into the slot; false when it should be skipped like
    /// `JSON.stringify` does (undefined, or a function outside a struct field).
    fn read(&mut self, id: jsid, in_struct: bool) -> Result<bool> {
        let cx = self.d.cx;
        rooted!(in(cx) let id = id);
        if !unsafe {
            jsapi::JS_GetPropertyById(
                cx,
                self.obj.into(),
                id.handle().into(),
                self.slot.reborrow().into(),
            )
        } {
            return Err(Error::new("cannot read property"));
        }
        let value = self.slot.get();
        let function = value.is_object() && unsafe { jsapi::IsCallable(value.to_object()) };
        Ok(!value.is_undefined() && (in_struct || !function))
    }
}

impl<'de> de::MapAccess<'de> for ObjectReader<'_, '_, '_> {
    type Error = Error;

    fn next_key_seed<K: DeserializeSeed<'de>>(&mut self, seed: K) -> Result<Option<K::Value>> {
        while self.pos < self.ids.len() {
            let id = self.ids[self.pos];
            self.pos += 1;
            if let Some((fields, atoms)) = &self.fields {
                let Some(index) = atoms.iter().position(|&a| a == id.asBits_) else {
                    continue;
                };
                let name = fields[index];
                if !self.read(id, true)? {
                    continue;
                }
                name.clone_into(&mut self.key);
                return seed.deserialize(name.into_deserializer()).map(Some);
            }
            let key = if id.is_string() {
                let cx = unsafe { JSContext::from_ptr(NonNull::new_unchecked(self.d.cx)) };
                let string = NonNull::new(id.to_string()).expect("string id without a string");
                unsafe { jsstr_to_string(&cx, string) }
            } else if id.is_int() {
                id.to_int().to_string()
            } else {
                continue;
            };
            if !self.read(id, false)? {
                continue;
            }
            self.key.clone_from(&key);
            return seed.deserialize(key.into_deserializer()).map(Some);
        }
        Ok(None)
    }

    fn next_value_seed<V: DeserializeSeed<'de>>(&mut self, seed: V) -> Result<V::Value> {
        seed.deserialize(self.d.nested(self.slot.handle()))
            .map_err(|e| e.at(&self.key))
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.ids.len() - self.pos)
    }
}

/// Writes a Rust value as a JS value. `()` becomes `undefined`, `None` becomes `null`.
pub unsafe fn to_js<T: Serialize + ?Sized>(
    cx: *mut RawJSContext,
    value: &T,
    out: MutableHandle<Value>,
) -> Result<()> {
    value.serialize(Serializer { cx, out })
}

struct Serializer<'a> {
    cx: *mut RawJSContext,
    out: MutableHandle<'a, Value>,
}

impl Serializer<'_> {
    fn number(mut self, n: f64) {
        match n.to_i32() {
            Some(i) if f64::from(i).to_bits() == n.to_bits() => {
                self.out.set(Int32Value(i));
            }
            _ => self.out.set(DoubleValue(n)),
        }
    }

    fn new_object(&self) -> Result<RootedTraceableBox<Heap<*mut JSObject>>> {
        let obj = unsafe { jsapi::JS_NewPlainObject(self.cx) };
        if obj.is_null() {
            return Err(Error::new("cannot create object"));
        }
        Ok(RootedTraceableBox::from_box(Heap::boxed(obj)))
    }
}

/// Serializes `value` into a fresh rooted slot and passes it to `store`.
unsafe fn with_value<T: Serialize + ?Sized>(
    cx: *mut RawJSContext,
    value: &T,
    store: impl FnOnce(Handle<Value>) -> bool,
) -> Result<()> {
    rooted!(in(cx) let mut slot = UndefinedValue());
    value.serialize(Serializer {
        cx,
        out: slot.handle_mut(),
    })?;
    if store(slot.handle()) {
        Ok(())
    } else {
        Err(Error::new("cannot store value"))
    }
}

impl<'a> ser::Serializer for Serializer<'a> {
    type Ok = ();
    type Error = Error;
    type SerializeSeq = ArrayWriter<'a>;
    type SerializeTuple = ArrayWriter<'a>;
    type SerializeTupleStruct = ArrayWriter<'a>;
    type SerializeTupleVariant = ser::Impossible<(), Error>;
    type SerializeMap = ser::Impossible<(), Error>;
    type SerializeStruct = ObjectWriter<'a>;
    type SerializeStructVariant = ser::Impossible<(), Error>;

    fn serialize_bool(mut self, v: bool) -> Result<()> {
        self.out.set(BooleanValue(v));
        Ok(())
    }

    fn serialize_i8(self, v: i8) -> Result<()> {
        self.number(v.into());
        Ok(())
    }

    fn serialize_i16(self, v: i16) -> Result<()> {
        self.number(v.into());
        Ok(())
    }

    fn serialize_i32(self, v: i32) -> Result<()> {
        self.number(v.into());
        Ok(())
    }

    fn serialize_i64(self, v: i64) -> Result<()> {
        self.number(v.to_f64().unwrap_or(f64::NAN));
        Ok(())
    }

    fn serialize_u8(self, v: u8) -> Result<()> {
        self.number(v.into());
        Ok(())
    }

    fn serialize_u16(self, v: u16) -> Result<()> {
        self.number(v.into());
        Ok(())
    }

    fn serialize_u32(self, v: u32) -> Result<()> {
        self.number(v.into());
        Ok(())
    }

    fn serialize_u64(self, v: u64) -> Result<()> {
        self.number(v.to_f64().unwrap_or(f64::NAN));
        Ok(())
    }

    /// Uses the shortest decimal form, so 0.1f32 arrives as 0.1 rather than 0.10000000149.
    fn serialize_f32(self, v: f32) -> Result<()> {
        self.number(v.to_string().parse().unwrap_or_else(|_| v.into()));
        Ok(())
    }

    fn serialize_f64(self, v: f64) -> Result<()> {
        self.number(v);
        Ok(())
    }

    fn serialize_char(self, v: char) -> Result<()> {
        self.serialize_str(v.encode_utf8(&mut [0; 4]))
    }

    fn serialize_str(self, v: &str) -> Result<()> {
        let mut cx = unsafe { JSContext::from_ptr(NonNull::new_unchecked(self.cx)) };
        v.to_jsval(&mut cx, self.out);
        Ok(())
    }

    fn serialize_bytes(self, _v: &[u8]) -> Result<()> {
        Err(Error::new("byte arrays are not supported"))
    }

    fn serialize_none(mut self) -> Result<()> {
        self.out.set(NullValue());
        Ok(())
    }

    fn serialize_some<T: Serialize + ?Sized>(self, value: &T) -> Result<()> {
        value.serialize(self)
    }

    fn serialize_unit(mut self) -> Result<()> {
        self.out.set(UndefinedValue());
        Ok(())
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<()> {
        self.serialize_none()
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _index: u32,
        variant: &'static str,
    ) -> Result<()> {
        self.serialize_str(variant)
    }

    fn serialize_newtype_struct<T: Serialize + ?Sized>(
        mut self,
        name: &'static str,
        value: &T,
    ) -> Result<()> {
        if name != HANDLER {
            return value.serialize(self);
        }
        // The bits fit in a double exactly (see `Handler::GENERATIONS`).
        let cx = self.cx;
        value.serialize(Serializer {
            cx,
            out: self.out.reborrow(),
        })?;
        let bits = self.out.get().to_number().to_u64().unwrap_or(u64::MAX);
        unsafe { super::handler_function(cx, Handler::from_bits(bits), self.out) };
        Ok(())
    }

    fn serialize_newtype_variant<T: Serialize + ?Sized>(
        self,
        name: &'static str,
        _index: u32,
        variant: &'static str,
        _value: &T,
    ) -> Result<()> {
        Err(Error::new(format!("enum {name}::{variant} needs a tag")))
    }

    fn serialize_seq(self, len: Option<usize>) -> Result<ArrayWriter<'a>> {
        let array = unsafe { jsapi::NewArrayObject1(self.cx, len.unwrap_or(0)) };
        if array.is_null() {
            return Err(Error::new("cannot create array"));
        }
        Ok(ArrayWriter {
            cx: self.cx,
            array: RootedTraceableBox::from_box(Heap::boxed(array)),
            index: 0,
            out: self.out,
        })
    }

    fn serialize_tuple(self, len: usize) -> Result<ArrayWriter<'a>> {
        self.serialize_seq(Some(len))
    }

    fn serialize_tuple_struct(self, _name: &'static str, len: usize) -> Result<ArrayWriter<'a>> {
        self.serialize_seq(Some(len))
    }

    fn serialize_tuple_variant(
        self,
        name: &'static str,
        _index: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant> {
        Err(Error::new(format!("enum {name}::{variant} needs a tag")))
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap> {
        Err(Error::new("maps are not supported; use a struct"))
    }

    fn serialize_struct(self, _name: &'static str, _len: usize) -> Result<ObjectWriter<'a>> {
        Ok(ObjectWriter {
            cx: self.cx,
            obj: self.new_object()?,
            out: self.out,
        })
    }

    fn serialize_struct_variant(
        self,
        name: &'static str,
        _index: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant> {
        Err(Error::new(format!("enum {name}::{variant} needs a tag")))
    }
}

pub struct ArrayWriter<'a> {
    cx: *mut RawJSContext,
    array: RootedTraceableBox<Heap<*mut JSObject>>,
    index: u32,
    out: MutableHandle<'a, Value>,
}

impl ArrayWriter<'_> {
    fn push<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<()> {
        let (cx, index) = (self.cx, self.index);
        let array = self.array.handle();
        unsafe {
            with_value(cx, value, |v| {
                jsapi::JS_SetElement(cx, array.into(), index, v.into())
            })?;
        }
        self.index += 1;
        Ok(())
    }

    fn finish(mut self) {
        self.out.set(ObjectValue(self.array.get()));
    }
}

impl ser::SerializeSeq for ArrayWriter<'_> {
    type Ok = ();
    type Error = Error;

    fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<()> {
        self.push(value)
    }

    fn end(self) -> Result<()> {
        self.finish();
        Ok(())
    }
}

impl ser::SerializeTuple for ArrayWriter<'_> {
    type Ok = ();
    type Error = Error;

    fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<()> {
        self.push(value)
    }

    fn end(self) -> Result<()> {
        self.finish();
        Ok(())
    }
}

impl ser::SerializeTupleStruct for ArrayWriter<'_> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<()> {
        self.push(value)
    }

    fn end(self) -> Result<()> {
        self.finish();
        Ok(())
    }
}

pub struct ObjectWriter<'a> {
    cx: *mut RawJSContext,
    obj: RootedTraceableBox<Heap<*mut JSObject>>,
    out: MutableHandle<'a, Value>,
}

impl ser::SerializeStruct for ObjectWriter<'_> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T: Serialize + ?Sized>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<()> {
        let (cx, obj) = (self.cx, self.obj.handle());
        unsafe {
            let id = atom(cx, key)?;
            rooted!(in(cx) let id = jsid_from_bits(id));
            with_value(cx, value, |v| {
                let attrs = u32::from(jsapi::JSPROP_ENUMERATE);
                jsapi::JS_DefinePropertyById2(cx, obj.into(), id.handle().into(), v.into(), attrs)
            })
        }
    }

    fn end(mut self) -> Result<()> {
        self.out.set(ObjectValue(self.obj.get()));
        Ok(())
    }
}
