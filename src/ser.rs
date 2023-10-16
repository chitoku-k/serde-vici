//! Serialize a Rust data structure using the VICI protocol.

use std::{io, str};

use bytes::BufMut;
use serde::{ser, Serialize};

use crate::{
    error::{Error, Result},
    value::{FieldType, ListElement},
    ElementType,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum State {
    None,
    Key(FieldType),
    Value,
    ListItem(ListElement, Option<usize>),
}

/// A structure for serializing Rust values using the VICI protocol.
///
/// # Example
///
/// ```
/// use anyhow::Result;
/// use serde::Serialize;
/// use std::collections::BTreeMap;
///
/// fn main() -> Result<()> {
///     let mut buffer = Vec::new();
///     let mut ser = serde_vici::Serializer::new(&mut buffer);
///
///     let mut object = BTreeMap::new();
///     object.insert("key1", "value1");
///     object.insert("key2", "value2");
///     object.serialize(&mut ser)?;
///
///     assert_eq!(
///         buffer,
///         vec![
///             3, 4, b'k', b'e', b'y', b'1', 0, 6, b'v', b'a', b'l', b'u', b'e', b'1',
///             3, 4, b'k', b'e', b'y', b'2', 0, 6, b'v', b'a', b'l', b'u', b'e', b'2',
///         ]
///     );
///     Ok(())
/// }
/// ```
pub struct Serializer<'a, W> {
    writer: &'a mut W,
    level: Option<usize>,
    state: State,
}

/// Serialize the given data structure as a VICI byte vector.
///
/// # Errors
/// Serialization can fail if `T`'s implementation of `Serialize` decides to return an error.
pub fn to_vec<T: ?Sized>(value: &T) -> Result<Vec<u8>>
where
    T: ser::Serialize,
{
    let mut buf = vec![];
    let mut serializer = Serializer::new(&mut buf);
    value.serialize(&mut serializer)?;
    Ok(buf)
}

/// Serialize the given data structure as VICI into the IO stream.
///
/// # Errors
/// Serialization can fail if `T`'s implementation of `Serialize` decides to return an error.
pub fn to_writer<W, T: ?Sized>(writer: &mut W, value: &T) -> Result<()>
where
    W: io::Write,
    T: ser::Serialize,
{
    let mut serializer = Serializer::new(writer);
    value.serialize(&mut serializer)?;
    Ok(())
}

impl<'a, W> Serializer<'a, W>
where
    W: io::Write,
{
    /// Creates a new VICI serializer.
    pub fn new(writer: &'a mut W) -> Self {
        let level = None;
        let state = State::None;
        Self { writer, level, state }
    }
}

macro_rules! serialize_integer {
    ($method:ident => $type:ident) => {
        #[inline]
        fn $method(self, v: $type) -> Result<Self::Ok> {
            let mut buf = itoa::Buffer::new();
            let s = buf.format(v);
            self.serialize_bytes(s.as_bytes())
        }
    };
}

macro_rules! serialize_float {
    ($method:ident => $type:ident) => {
        #[inline]
        fn $method(self, v: $type) -> Result<Self::Ok> {
            let mut buf = ryu::Buffer::new();
            let s = buf.format_finite(v);
            self.serialize_bytes(s.as_bytes())
        }
    };
}

impl<'a, 'se, W> ser::Serializer for &'a mut Serializer<'se, W>
where
    W: io::Write,
{
    type Ok = ();
    type Error = Error;

    type SerializeSeq = Self;
    type SerializeTuple = Self;
    type SerializeTupleStruct = Self;
    type SerializeTupleVariant = Self;
    type SerializeMap = Self;
    type SerializeStruct = Self;
    type SerializeStructVariant = Self;

    #[inline]
    fn serialize_bool(self, v: bool) -> Result<Self::Ok> {
        let s = if v { b"yes" as &[u8] } else { b"no" as &[u8] };
        self.serialize_bytes(s)
    }

    serialize_integer!(serialize_i8 => i8);
    serialize_integer!(serialize_i16 => i16);
    serialize_integer!(serialize_i32 => i32);
    serialize_integer!(serialize_i64 => i64);
    serialize_integer!(serialize_u8 => u8);
    serialize_integer!(serialize_u16 => u16);
    serialize_integer!(serialize_u32 => u32);
    serialize_integer!(serialize_u64 => u64);
    serialize_float!(serialize_f32 => f32);
    serialize_float!(serialize_f64 => f64);

    #[inline]
    fn serialize_char(self, v: char) -> Result<Self::Ok> {
        let mut buffer = [0; 4];
        self.serialize_str(v.encode_utf8(&mut buffer))
    }

    #[inline]
    fn serialize_str(self, v: &str) -> Result<Self::Ok> {
        self.serialize_bytes(v.as_bytes())
    }

    #[inline]
    fn serialize_bytes(self, v: &[u8]) -> Result<Self::Ok> {
        let mut buf = vec![];
        match self.state {
            State::None => {
                return Err(io::Error::from(io::ErrorKind::InvalidData).into());
            },
            State::Key(_) | State::ListItem(_, None) => {
                buf.put_u8(v.len() as u8);
                self.writer.write_all(&buf)?;
            },
            State::Value | State::ListItem(_, _) => {
                buf.put_u16(v.len() as u16);
                self.writer.write_all(&buf)?;
            },
        }

        self.writer.write_all(v)?;
        Ok(())
    }

    #[inline]
    fn serialize_none(self) -> Result<Self::Ok> {
        match self.state {
            State::None => {
                return Err(io::Error::from(io::ErrorKind::InvalidData).into());
            },
            State::Key(_) | State::ListItem(_, None) => {
                self.writer.write_all(&[0])?;
            },
            State::Value | State::ListItem(_, _) => {
                self.writer.write_all(&[0, 0])?;
            },
        }

        Ok(())
    }

    #[inline]
    fn serialize_some<T: ?Sized>(self, value: &T) -> Result<Self::Ok>
    where
        T: serde::Serialize,
    {
        value.serialize(self)
    }

    #[inline]
    fn serialize_unit(self) -> Result<Self::Ok> {
        Ok(())
    }

    #[inline]
    fn serialize_unit_struct(self, _: &'static str) -> Result<Self::Ok> {
        Ok(())
    }

    #[inline]
    fn serialize_unit_variant(self, _: &'static str, _: u32, value: &'static str) -> Result<Self::Ok> {
        value.serialize(self)
    }

    #[inline]
    fn serialize_newtype_struct<T: ?Sized>(self, _: &'static str, value: &T) -> Result<Self::Ok>
    where
        T: serde::Serialize,
    {
        value.serialize(self)
    }

    #[inline]
    fn serialize_newtype_variant<T: ?Sized>(self, _: &'static str, _: u32, _: &'static str, value: &T) -> Result<Self::Ok>
    where
        T: serde::Serialize,
    {
        value.serialize(self)
    }

    #[inline]
    fn serialize_seq(self, _: Option<usize>) -> Result<Self::SerializeSeq> {
        Ok(self)
    }

    #[inline]
    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple> {
        self.serialize_seq(Some(len))
    }

    #[inline]
    fn serialize_tuple_struct(self, _: &'static str, len: usize) -> Result<Self::SerializeTupleStruct> {
        self.serialize_seq(Some(len))
    }

    #[inline]
    fn serialize_tuple_variant(self, _: &'static str, _: u32, _: &'static str, len: usize) -> Result<Self::SerializeTupleVariant> {
        self.serialize_seq(Some(len))
    }

    #[inline]
    fn serialize_map(self, _: Option<usize>) -> Result<Self::SerializeMap> {
        self.level = Some(self.level.map_or(1, |l| l + 1));
        Ok(self)
    }

    #[inline]
    fn serialize_struct(self, _: &'static str, len: usize) -> Result<Self::SerializeStruct> {
        self.serialize_map(Some(len))
    }

    #[inline]
    fn serialize_struct_variant(self, _: &'static str, _: u32, _: &'static str, len: usize) -> Result<Self::SerializeStructVariant> {
        self.serialize_map(Some(len))
    }
}

#[doc(hidden)]
impl<'a, 'se, W> ser::SerializeSeq for &'a mut Serializer<'se, W>
where
    W: io::Write,
{
    type Ok = ();
    type Error = Error;

    #[inline]
    fn serialize_element<T: ?Sized>(&mut self, value: &T) -> Result<()>
    where
        T: serde::Serialize,
    {
        let mut buf = vec![];
        match self.state {
            State::ListItem(ListElement::String, o) => {
                buf.put_u8(ElementType::ListItem as u8);
                self.writer.write_all(&buf)?;

                self.state = State::ListItem(ListElement::String, Some(o.map_or(0, |n| n + 1)));
                value.serialize(&mut **self)?;
            },
            State::ListItem(ListElement::Section, o) => {
                buf.put_u8(ElementType::SectionStart as u8);
                self.writer.write_all(&buf)?;

                self.state = State::Key(FieldType::Section);
                let index = o.unwrap_or_default();
                index.serialize(&mut **self)?;

                self.state = State::ListItem(ListElement::Section, Some(index));
                value.serialize(&mut **self)?;

                self.state = State::ListItem(ListElement::Section, Some(index + 1));
            },
            _ => return Err(io::Error::from(io::ErrorKind::InvalidInput).into()),
        }

        Ok(())
    }

    #[inline]
    fn end(self) -> Result<Self::Ok> {
        let mut buf = vec![];
        match &self.state {
            State::ListItem(ListElement::String, _) => {
                buf.put_u8(ElementType::ListEnd as u8);
            },
            State::ListItem(ListElement::Section, _) => {
                buf.put_u8(ElementType::SectionEnd as u8);
            },
            _ => {},
        }
        self.writer.write_all(&buf)?;
        Ok(())
    }
}

#[doc(hidden)]
impl<'a, 'se, W> ser::SerializeTuple for &'a mut Serializer<'se, W>
where
    W: io::Write,
{
    type Ok = ();
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

#[doc(hidden)]
impl<'a, 'se, W> ser::SerializeTupleStruct for &'a mut Serializer<'se, W>
where
    W: io::Write,
{
    type Ok = ();
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

#[doc(hidden)]
impl<'a, 'se, W> ser::SerializeTupleVariant for &'a mut Serializer<'se, W>
where
    W: io::Write,
{
    type Ok = ();
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

#[doc(hidden)]
impl<'a, 'se, W> ser::SerializeMap for &'a mut Serializer<'se, W>
where
    W: io::Write,
{
    type Ok = ();
    type Error = Error;

    #[inline]
    fn serialize_key<T: ?Sized>(&mut self, key: &T) -> Result<()>
    where
        T: serde::Serialize,
    {
        let field = match self.state.clone() {
            State::Key(field) => field,
            _ => return Err(io::Error::from(io::ErrorKind::InvalidInput).into()),
        };

        let element = match field {
            FieldType::None => return Err(io::Error::from(io::ErrorKind::InvalidData).into()),
            FieldType::List(ListElement::Section) => {
                self.state = State::ListItem(ListElement::Section, None);
                ElementType::SectionStart
            },
            FieldType::List(ListElement::String) => {
                self.state = State::ListItem(ListElement::String, None);
                ElementType::ListStart
            },
            FieldType::Section => ElementType::SectionStart,
            FieldType::String => ElementType::KeyValue,
        };

        let mut buf = vec![];
        buf.put_u8(element as u8);
        self.writer.write_all(&buf)?;

        key.serialize(&mut **self)
    }

    #[inline]
    fn serialize_value<T: ?Sized>(&mut self, value: &T) -> Result<()>
    where
        T: serde::Serialize,
    {
        value.serialize(&mut **self)
    }

    #[inline]
    fn serialize_entry<K: ?Sized, V: ?Sized>(&mut self, key: &K, value: &V) -> Result<()>
    where
        K: serde::Serialize,
        V: serde::Serialize,
    {
        self.state = State::Key(FieldType::from(value)?);
        self.serialize_key(key)?;

        match self.state {
            State::ListItem(_, _) => {},
            _ => {
                self.state = State::Value;
            },
        }
        self.serialize_value(value)?;

        Ok(())
    }

    #[inline]
    fn end(self) -> Result<Self::Ok> {
        self.level = match self.level {
            Some(level) if level > 1 => Some(level - 1),
            _ => return Ok(()),
        };

        let mut buf = vec![];
        buf.put_u8(ElementType::SectionEnd as u8);
        self.writer.write_all(&buf)?;

        Ok(())
    }
}

#[doc(hidden)]
impl<'a, 'se, W> ser::SerializeStruct for &'a mut Serializer<'se, W>
where
    W: io::Write,
{
    type Ok = ();
    type Error = Error;

    #[inline]
    fn serialize_field<T: ?Sized>(&mut self, key: &'static str, value: &T) -> Result<()>
    where
        T: serde::Serialize,
    {
        ser::SerializeMap::serialize_entry(self, key, value)
    }

    #[inline]
    fn end(self) -> Result<Self::Ok> {
        ser::SerializeMap::end(self)
    }
}

#[doc(hidden)]
impl<'a, 'se, W> ser::SerializeStructVariant for &'a mut Serializer<'se, W>
where
    W: io::Write,
{
    type Ok = ();
    type Error = Error;

    #[inline]
    fn serialize_field<T: ?Sized>(&mut self, key: &'static str, value: &T) -> Result<()>
    where
        T: serde::Serialize,
    {
        ser::SerializeMap::serialize_entry(self, key, value)
    }

    #[inline]
    fn end(self) -> Result<Self::Ok> {
        ser::SerializeMap::end(self)
    }
}

#[cfg(test)]
mod tests {
    use indexmap::indexmap;
    use pretty_assertions::assert_eq;
    use serde_derive::Serialize;

    use super::*;

    #[test]
    fn serialize_example() {
        #[derive(Serialize)]
        struct RootSection {
            key1: String,
            section1: MainSection,
        }

        #[derive(Serialize)]
        struct MainSection {
            #[serde(rename = "sub-section")]
            sub_section: SubSection,
            list1: Vec<String>,
        }

        #[derive(Serialize)]
        struct SubSection {
            key2: String,
        }

        let data = RootSection {
            key1: "value1".to_string(),
            section1: MainSection {
                sub_section: SubSection {
                    key2: "value2".to_string(),
                },
                list1: vec!["item1".to_string(), "item2".to_string()],
            },
        };

        let actual = to_vec(&data).unwrap();

        #[rustfmt::skip]
        assert_eq!(
            actual,
            vec![
                // key1 = value1
                3, 4, b'k', b'e', b'y', b'1', 0, 6, b'v', b'a', b'l', b'u', b'e', b'1',
                // section1
                1, 8, b's', b'e', b'c', b't', b'i', b'o', b'n', b'1',
                // sub-section
                1, 11, b's', b'u', b'b', b'-', b's', b'e', b'c', b't', b'i', b'o', b'n',
                // key2 = value2
                3, 4, b'k', b'e', b'y', b'2', 0, 6, b'v', b'a', b'l', b'u', b'e', b'2',
                // sub-section end
                2,
                // list1
                4, 5, b'l', b'i', b's', b't', b'1',
                // item1
                5, 0, 5, b'i', b't', b'e', b'm', b'1',
                // item2
                5, 0, 5, b'i', b't', b'e', b'm', b'2',
                // list1 end
                6,
                // section1 end
                2,
            ]
        );
    }

    #[test]
    fn serialize_pools() {
        #[derive(Serialize)]
        struct Pool {
            base: String,
            size: u32,
            online: u32,
            offline: u32,
            leases: Vec<Lease>,
        }

        #[derive(Serialize)]
        struct Lease {
            address: String,
            identity: Option<String>,
            status: Status,
        }

        #[derive(Serialize)]
        enum Status {
            #[serde(rename = "online")]
            Online,
            #[serde(rename = "offline")]
            Offline,
        }

        let data = indexmap! {
            "pool-01" => Pool {
                base: "192.0.2.1".to_string(),
                size: 4,
                online: 3,
                offline: 1,
                leases: vec![
                    Lease {
                        address: "192.0.2.2".to_string(),
                        identity: Some("identity-01".to_string()),
                        status: Status::Online,
                    },
                    Lease {
                        address: "192.0.2.3".to_string(),
                        identity: Some("identity-02".to_string()),
                        status: Status::Online,
                    },
                    Lease {
                        address: "192.0.2.4".to_string(),
                        identity: Some("identity-03".to_string()),
                        status: Status::Online,
                    },
                    Lease {
                        address: "192.0.2.5".to_string(),
                        identity: None,
                        status: Status::Offline,
                    },
                ],
            },
        };

        let actual = to_vec(&data).unwrap();

        #[rustfmt::skip]
        assert_eq!(
            actual,
            vec![
                // pool-01
                1, 7, b'p', b'o', b'o', b'l', b'-', b'0', b'1',
                // base = 192.0.2.1
                3, 4, b'b', b'a', b's', b'e', 0, 9, b'1', b'9', b'2', b'.', b'0', b'.', b'2', b'.', b'1',
                // size = 4
                3, 4, b's', b'i', b'z', b'e', 0, 1, b'4',
                // online = 3,
                3, 6, b'o', b'n', b'l', b'i', b'n', b'e', 0, 1, b'3',
                // offline = 1,
                3, 7, b'o', b'f', b'f', b'l', b'i', b'n', b'e', 0, 1, b'1',
                // leases
                1, 6, b'l', b'e', b'a', b's', b'e', b's',
                // 0
                1, 1, b'0',
                // address = 192.0.2.2
                3, 7, b'a', b'd', b'd', b'r', b'e', b's', b's', 0, 9, b'1', b'9', b'2', b'.', b'0', b'.', b'2', b'.', b'2',
                // identity = identity-01
                3, 8, b'i', b'd', b'e', b'n', b't', b'i', b't', b'y', 0, 11, b'i', b'd', b'e', b'n', b't', b'i', b't', b'y', b'-', b'0', b'1',
                // status = online
                3, 6, b's', b't', b'a', b't', b'u', b's', 0, 6, b'o', b'n', b'l', b'i', b'n', b'e',
                // 0 end
                2,
                // 1
                1, 1, b'1',
                // address = 192.0.2.3
                3, 7, b'a', b'd', b'd', b'r', b'e', b's', b's', 0, 9, b'1', b'9', b'2', b'.', b'0', b'.', b'2', b'.', b'3',
                // identity = identity-02
                3, 8, b'i', b'd', b'e', b'n', b't', b'i', b't', b'y', 0, 11, b'i', b'd', b'e', b'n', b't', b'i', b't', b'y', b'-', b'0', b'2',
                // status = online
                3, 6, b's', b't', b'a', b't', b'u', b's', 0, 6, b'o', b'n', b'l', b'i', b'n', b'e',
                // 1 end
                2,
                // 2
                1, 1, b'2',
                // address = 192.0.2.4
                3, 7, b'a', b'd', b'd', b'r', b'e', b's', b's', 0, 9, b'1', b'9', b'2', b'.', b'0', b'.', b'2', b'.', b'4',
                // identity = identity-03
                3, 8, b'i', b'd', b'e', b'n', b't', b'i', b't', b'y', 0, 11, b'i', b'd', b'e', b'n', b't', b'i', b't', b'y', b'-', b'0', b'3',
                // status = online
                3, 6, b's', b't', b'a', b't', b'u', b's', 0, 6, b'o', b'n', b'l', b'i', b'n', b'e',
                // 2 end
                2,
                // 3
                1, 1, b'3',
                // address = 192.0.2.5
                3, 7, b'a', b'd', b'd', b'r', b'e', b's', b's', 0, 9, b'1', b'9', b'2', b'.', b'0', b'.', b'2', b'.', b'5',
                // identity =
                3, 8, b'i', b'd', b'e', b'n', b't', b'i', b't', b'y', 0, 0,
                // status = offline
                3, 6, b's', b't', b'a', b't', b'u', b's', 0, 7, b'o', b'f', b'f', b'l', b'i', b'n', b'e',
                // 3 end
                2,
                // leases end
                2,
                // pool-01 end
                2,
            ]
        );
    }
}
