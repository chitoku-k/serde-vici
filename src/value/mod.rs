use serde::{ser, Serialize};

use crate::error::{Error, Result};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum FieldType {
    None,
    String,
    Section,
    List(ListElement),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ListElement {
    String,
    Section,
}

impl FieldType {
    #[inline]
    pub fn from(s: impl Serialize) -> Result<Self> {
        let mut serializer = FieldTypeSerializer { item: None };
        s.serialize(&mut serializer)
    }
}

struct FieldTypeSerializer {
    item: Option<FieldType>,
}

impl<'a> ser::Serializer for &'a mut FieldTypeSerializer {
    type Ok = FieldType;
    type Error = Error;

    type SerializeSeq = Self;
    type SerializeTuple = Self;
    type SerializeTupleStruct = Self;
    type SerializeTupleVariant = Self;
    type SerializeMap = Self;
    type SerializeStruct = Self;
    type SerializeStructVariant = Self;

    #[inline]
    fn serialize_bool(self, _: bool) -> Result<Self::Ok> {
        Ok(FieldType::String)
    }

    #[inline]
    fn serialize_i8(self, _: i8) -> Result<Self::Ok> {
        Ok(FieldType::String)
    }

    #[inline]
    fn serialize_i16(self, _: i16) -> Result<Self::Ok> {
        Ok(FieldType::String)
    }

    #[inline]
    fn serialize_i32(self, _: i32) -> Result<Self::Ok> {
        Ok(FieldType::String)
    }

    #[inline]
    fn serialize_i64(self, _: i64) -> Result<Self::Ok> {
        Ok(FieldType::String)
    }

    #[inline]
    fn serialize_u8(self, _: u8) -> Result<Self::Ok> {
        Ok(FieldType::String)
    }

    #[inline]
    fn serialize_u16(self, _: u16) -> Result<Self::Ok> {
        Ok(FieldType::String)
    }

    #[inline]
    fn serialize_u32(self, _: u32) -> Result<Self::Ok> {
        Ok(FieldType::String)
    }

    #[inline]
    fn serialize_u64(self, _: u64) -> Result<Self::Ok> {
        Ok(FieldType::String)
    }

    #[inline]
    fn serialize_f32(self, _: f32) -> Result<Self::Ok> {
        Ok(FieldType::String)
    }

    #[inline]
    fn serialize_f64(self, _: f64) -> Result<Self::Ok> {
        Ok(FieldType::String)
    }

    #[inline]
    fn serialize_char(self, _: char) -> Result<Self::Ok> {
        Ok(FieldType::String)
    }

    #[inline]
    fn serialize_str(self, _: &str) -> Result<Self::Ok> {
        Ok(FieldType::String)
    }

    #[inline]
    fn serialize_bytes(self, _: &[u8]) -> Result<Self::Ok> {
        Ok(FieldType::String)
    }

    #[inline]
    fn serialize_none(self) -> Result<Self::Ok> {
        Ok(FieldType::String)
    }

    #[inline]
    fn serialize_some<T: ?Sized>(self, _: &T) -> Result<Self::Ok> {
        Ok(FieldType::String)
    }

    #[inline]
    fn serialize_unit(self) -> Result<Self::Ok> {
        Ok(FieldType::None)
    }

    #[inline]
    fn serialize_unit_struct(self, _: &'static str) -> Result<Self::Ok> {
        Ok(FieldType::None)
    }

    #[inline]
    fn serialize_unit_variant(self, _: &'static str, _: u32, _: &'static str) -> Result<Self::Ok> {
        Ok(FieldType::String)
    }

    #[inline]
    fn serialize_newtype_struct<T: ?Sized>(self, _: &'static str, _: &T) -> Result<Self::Ok> {
        Ok(FieldType::String)
    }

    #[inline]
    fn serialize_newtype_variant<T: ?Sized>(self, _: &'static str, _: u32, _: &'static str, _: &T) -> Result<Self::Ok> {
        Ok(FieldType::String)
    }

    #[inline]
    fn serialize_seq(self, _: Option<usize>) -> Result<Self::SerializeSeq> {
        Ok(self)
    }

    #[inline]
    fn serialize_tuple(self, _: usize) -> Result<Self::SerializeTuple> {
        Ok(self)
    }

    #[inline]
    fn serialize_tuple_struct(self, _: &'static str, _: usize) -> Result<Self::SerializeTupleStruct> {
        Ok(self)
    }

    #[inline]
    fn serialize_tuple_variant(self, _: &'static str, _: u32, _: &'static str, _: usize) -> Result<Self::SerializeTupleVariant> {
        Ok(self)
    }

    #[inline]
    fn serialize_map(self, _: Option<usize>) -> Result<Self::SerializeMap> {
        Ok(self)
    }

    #[inline]
    fn serialize_struct(self, _: &'static str, _: usize) -> Result<Self::SerializeStruct> {
        Ok(self)
    }

    #[inline]
    fn serialize_struct_variant(self, _: &'static str, _: u32, _: &'static str, _: usize) -> Result<Self::SerializeStructVariant> {
        Ok(self)
    }

    #[inline]
    fn collect_str<T: ?Sized>(self, _: &T) -> Result<Self::Ok> {
        Ok(FieldType::String)
    }
}

impl<'a> ser::SerializeSeq for &'a mut FieldTypeSerializer {
    type Ok = FieldType;
    type Error = Error;

    #[inline]
    fn serialize_element<T: ?Sized>(&mut self, value: &T) -> Result<()>
    where
        T: serde::Serialize,
    {
        self.item = Some(value.serialize(&mut **self)?);
        Ok(())
    }

    #[inline]
    fn end(self) -> Result<Self::Ok> {
        match self.item {
            Some(FieldType::Section) => Ok(FieldType::List(ListElement::Section)),
            _ => Ok(FieldType::List(ListElement::String)),
        }
    }
}

impl<'a> ser::SerializeTuple for &'a mut FieldTypeSerializer {
    type Ok = FieldType;
    type Error = Error;

    #[inline]
    fn serialize_element<T: ?Sized>(&mut self, value: &T) -> Result<()>
    where
        T: serde::Serialize,
    {
        ser::SerializeSeq::serialize_element(self, value)
    }

    #[inline]
    fn end(self) -> Result<Self::Ok> {
        ser::SerializeSeq::end(self)
    }
}

impl<'a> ser::SerializeTupleStruct for &'a mut FieldTypeSerializer {
    type Ok = FieldType;
    type Error = Error;

    #[inline]
    fn serialize_field<T: ?Sized>(&mut self, value: &T) -> Result<()>
    where
        T: serde::Serialize,
    {
        ser::SerializeSeq::serialize_element(self, value)
    }

    #[inline]
    fn end(self) -> Result<Self::Ok> {
        ser::SerializeSeq::end(self)
    }
}

impl<'a> ser::SerializeTupleVariant for &'a mut FieldTypeSerializer {
    type Ok = FieldType;
    type Error = Error;

    #[inline]
    fn serialize_field<T: ?Sized>(&mut self, value: &T) -> Result<()>
    where
        T: serde::Serialize,
    {
        ser::SerializeSeq::serialize_element(self, value)
    }

    #[inline]
    fn end(self) -> Result<Self::Ok> {
        ser::SerializeSeq::end(self)
    }
}

impl<'a> ser::SerializeMap for &'a mut FieldTypeSerializer {
    type Ok = FieldType;
    type Error = Error;

    #[inline]
    fn serialize_key<T: ?Sized>(&mut self, _: &T) -> Result<()> {
        Ok(())
    }

    #[inline]
    fn serialize_value<T: ?Sized>(&mut self, _: &T) -> Result<()> {
        Ok(())
    }

    #[inline]
    fn end(self) -> Result<Self::Ok> {
        Ok(FieldType::Section)
    }
}

impl<'a> ser::SerializeStruct for &'a mut FieldTypeSerializer {
    type Ok = FieldType;
    type Error = Error;

    #[inline]
    fn serialize_field<T: ?Sized>(&mut self, _: &'static str, _: &T) -> Result<()> {
        Ok(())
    }

    #[inline]
    fn end(self) -> Result<Self::Ok> {
        Ok(FieldType::Section)
    }
}

impl<'a> ser::SerializeStructVariant for &'a mut FieldTypeSerializer {
    type Ok = FieldType;
    type Error = Error;

    #[inline]
    fn serialize_field<T: ?Sized>(&mut self, _: &'static str, _: &T) -> Result<()> {
        Ok(())
    }

    #[inline]
    fn end(self) -> Result<Self::Ok> {
        Ok(FieldType::Section)
    }
}
