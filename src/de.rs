//! Deserialize VICI data to a Rust data structure.

use core::result::Result::Ok;
use std::{io, str};

use serde::de::{self, IntoDeserializer};

use crate::{
    error::{Error, ErrorCode, Result},
    read::{IoRead, Read, Reference, SliceRead},
    value::ListElement,
    ElementType,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum State {
    None,
    Key,
    Value,
    SectionKey,
    ListName,
    ListItem(ListElement),
}

/// A structure for deserializing into Rust values using the VICI protocol.
///
/// # Example
///
/// ```
/// use anyhow::Result;
/// use serde::Deserialize;
/// use std::collections::BTreeMap;
///
/// fn main() -> Result<()> {
///     let input = vec![
///         3, 4, b'k', b'e', b'y', b'1', 0, 6, b'v', b'a', b'l', b'u', b'e', b'1',
///         3, 4, b'k', b'e', b'y', b'2', 0, 6, b'v', b'a', b'l', b'u', b'e', b'2',
///     ];
///     let mut de = serde_vici::Deserializer::from_slice(&input);
///     let value = BTreeMap::deserialize(&mut de)?;
///
///     assert_eq!(
///         value,
///         {
///             let mut object = BTreeMap::new();
///             object.insert("key1", "value1");
///             object.insert("key2", "value2");
///             object
///         }
///     );
///     Ok(())
/// }
/// ```
pub struct Deserializer<R> {
    read: R,
    level: Option<usize>,
    state: State,
    scratch: Vec<u8>,
}

/// Deserialize an instance of type `T` from an IO stream of the VICI protocol.
///
/// The content of the IO Stream is deserialized directly from the stream while being buffered in memory by serde_vici.
///
/// # Errors
/// Deserialization can fail if the structure of the input does not match the structure expected by `T`, for example if `T` is a struct type
/// but the input contains something other than a VICI section. It can also fail if the structure is correct but `T`'s implementation of
/// `Deserialize` decides that something is wrong with the data, for example required struct fields are missing from a VICI section.
pub fn from_reader<R, T>(reader: R) -> Result<T>
where
    R: io::Read,
    T: de::DeserializeOwned,
{
    let mut deserializer = Deserializer::new(IoRead::new(reader));
    let value = de::Deserialize::deserialize(&mut deserializer)?;
    Ok(value)
}

/// Deserialize an instance of type `T` from bytes of the VICI protocol.
///
/// # Errors
/// Deserialization can fail if the structure of the input does not match the structure expected by `T`, for example if `T` is a struct type
/// but the input contains something other than a VICI section. It can also fail if the structure is correct but `T`'s implementation of
/// `Deserialize` decides that something is wrong with the data, for example required struct fields are missing from a VICI section.
pub fn from_slice<'a, T>(slice: &'a [u8]) -> Result<T>
where
    T: de::Deserialize<'a>,
{
    let mut deserializer = Deserializer::new(SliceRead::new(slice));
    let value = de::Deserialize::deserialize(&mut deserializer)?;
    Ok(value)
}

impl<'de, R> Deserializer<R>
where
    R: Read<'de>,
{
    /// Creates a VICI deserializer from one of the possible serde_vici input sources.
    ///
    /// Typically it is more convenient to use either of the following methods instead:
    ///
    /// - Deserializer::from_reader
    /// - Deserializer::from_slice
    pub fn new(read: R) -> Self {
        let level = None;
        let state = State::None;
        let scratch = vec![];
        Self {
            read,
            level,
            state,
            scratch,
        }
    }

    #[inline]
    fn parse_element_type(&mut self) -> Result<ElementType> {
        self.read.parse_element_type()
    }

    #[inline]
    fn parse_str(&mut self) -> Result<Reference<'de, '_, str>> {
        match &self.state {
            State::Key | State::SectionKey | State::ListName => {
                self.scratch.clear();
                self.read.parse_key(&mut self.scratch)
            },
            State::Value | State::ListItem(_) => {
                self.scratch.clear();
                self.read.parse_value(&mut self.scratch)
            },
            State::None => Err(Error::io(io::Error::from(io::ErrorKind::InvalidData), Some(self.read.position()))),
        }
    }

    #[inline]
    fn parse_raw_value(&mut self) -> Result<Reference<'de, '_, [u8]>> {
        match &self.state {
            State::Value => {
                self.scratch.clear();
                self.read.parse_value_raw(&mut self.scratch)
            },
            _ => Err(Error::io(io::Error::from(io::ErrorKind::InvalidData), Some(self.read.position())))
        }
    }

    #[inline]
    fn peek(&mut self) -> Result<usize> {
        match &self.state {
            State::Key | State::SectionKey | State::ListName => {
                self.read.peek_key()
            },
            State::Value | State::ListItem(_) => {
                self.read.peek_value()
            },
            State::None => Err(Error::io(io::Error::from(io::ErrorKind::InvalidData), Some(self.read.position()))),
        }
    }
}

impl<R> Deserializer<IoRead<R>>
where
    R: io::Read,
{
    /// Creates a VICI deserializer from an `io::Read`.
    pub fn from_reader(reader: R) -> Self {
        Deserializer::new(IoRead::new(reader))
    }
}

impl<'a> Deserializer<SliceRead<'a>> {
    /// Creates a VICI deserializer from a `&[u8]`.
    pub fn from_slice(slice: &'a [u8]) -> Self {
        Deserializer::new(SliceRead::new(slice))
    }
}

macro_rules! deserialize_number {
    ($method:ident => $visit:ident) => {
        #[inline]
        fn $method<V>(self, visitor: V) -> Result<V::Value>
        where
            V: de::Visitor<'de>,
        {
            let result = self.parse_str()?;
            let value = result.parse().map_err::<Self::Error, _>(de::Error::custom)?;
            visitor.$visit(value)
        }
    };
}

impl<'de, R> de::Deserializer<'de> for &mut Deserializer<R>
where
    R: Read<'de>,
{
    type Error = Error;

    #[inline]
    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        match &self.state {
            State::Value => self.deserialize_byte_buf(visitor),
            State::Key | State::SectionKey | State::ListName => self.deserialize_str(visitor),
            State::ListItem(ListElement::String) => self.deserialize_seq(visitor),
            State::ListItem(ListElement::Section) | State::None => self.deserialize_map(visitor),
        }
    }

    #[inline]
    fn deserialize_bool<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        let input = self.parse_str()?;
        match &*input {
            "yes" => visitor.visit_bool(true),
            "no" => visitor.visit_bool(false),
            _ => Err(Error::io(io::Error::from(io::ErrorKind::InvalidData), Some(self.read.position()))),
        }
    }

    deserialize_number!(deserialize_i8 => visit_i8);
    deserialize_number!(deserialize_i16 => visit_i16);
    deserialize_number!(deserialize_i32 => visit_i32);
    deserialize_number!(deserialize_i64 => visit_i64);
    deserialize_number!(deserialize_u8 => visit_u8);
    deserialize_number!(deserialize_u16 => visit_u16);
    deserialize_number!(deserialize_u32 => visit_u32);
    deserialize_number!(deserialize_u64 => visit_u64);
    deserialize_number!(deserialize_f32 => visit_f32);
    deserialize_number!(deserialize_f64 => visit_f64);
    deserialize_number!(deserialize_char => visit_char);

    #[inline]
    fn deserialize_str<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        match self.parse_str()? {
            Reference::Borrowed(s) => visitor.visit_borrowed_str(s),
            Reference::Copied(s) => visitor.visit_str(s),
        }
    }

    #[inline]
    fn deserialize_string<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        self.deserialize_str(visitor)
    }

    #[inline]
    fn deserialize_bytes<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        match self.parse_raw_value()? {
            Reference::Borrowed(s) => visitor.visit_borrowed_bytes(s),
            Reference::Copied(s) => visitor.visit_bytes(s),
        }
    }

    #[inline]
    fn deserialize_byte_buf<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        self.deserialize_bytes(visitor)
    }

    #[inline]
    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        if self.state == State::None {
            return visitor.visit_some(self);
        }

        match self.peek()? {
            0 => {
                self.parse_str()?;
                visitor.visit_none()
            },
            _ => visitor.visit_some(self),
        }
    }

    #[inline]
    fn deserialize_unit<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_unit()
    }

    #[inline]
    fn deserialize_unit_struct<V>(self, _: &'static str, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        self.deserialize_unit(visitor)
    }

    #[inline]
    fn deserialize_newtype_struct<V>(self, _: &'static str, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_newtype_struct(self)
    }

    #[inline]
    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_seq(self)
    }

    #[inline]
    fn deserialize_tuple<V>(self, _: usize, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_seq(self)
    }

    #[inline]
    fn deserialize_tuple_struct<V>(self, _: &'static str, _: usize, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_seq(self)
    }

    #[inline]
    fn deserialize_map<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_map(self)
    }

    #[inline]
    fn deserialize_struct<V>(self, _: &'static str, _: &'static [&'static str], visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_map(self)
    }

    #[inline]
    fn deserialize_enum<V>(self, _: &'static str, _: &'static [&'static str], visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        let input = self.parse_str()?;
        visitor.visit_enum(input.into_deserializer())
    }

    #[inline]
    fn deserialize_identifier<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        self.deserialize_str(visitor)
    }

    #[inline]
    fn deserialize_ignored_any<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        self.deserialize_any(visitor)
    }

    #[inline]
    fn is_human_readable(&self) -> bool {
        false
    }
}

#[doc(hidden)]
impl<'de, R> de::SeqAccess<'de> for &mut Deserializer<R>
where
    R: Read<'de>,
{
    type Error = Error;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>>
    where
        T: de::DeserializeSeed<'de>,
    {
        match self.parse_element_type()? {
            ElementType::ListItem if matches!(self.state, State::ListItem(ListElement::String)) => {
                self.state = State::Value;
                let value = seed.deserialize(&mut **self).map(Some)?;

                self.state = State::ListItem(ListElement::String);
                Ok(value)
            },
            ElementType::ListEnd if matches!(self.state, State::ListItem(ListElement::String)) => {
                self.level = self.level.map(|l| l - 1).filter(|&l| l > 0);
                self.state = State::None;
                Ok(None)
            },
            ElementType::SectionEnd if matches!(self.state, State::ListItem(ListElement::Section)) => {
                self.level = self.level.map(|l| l - 1).filter(|&l| l > 0);
                self.state = State::None;
                Ok(None)
            },
            ElementType::SectionStart => {
                self.level = Some(self.level.map_or(1, |l| l + 1));

                self.state = State::SectionKey;
                self.parse_str()?;

                self.state = State::ListItem(ListElement::Section);
                let value = seed.deserialize(&mut **self).map(Some)?;

                self.state = State::ListItem(ListElement::Section);
                Ok(value)
            },
            v => Err(Error::data(
                ErrorCode::Message("unexpected element type".into()),
                Some(v as u8),
                Some(self.read.position()),
            )),
        }
    }
}

#[doc(hidden)]
impl<'de, R> de::MapAccess<'de> for &mut Deserializer<R>
where
    R: Read<'de>,
{
    type Error = Error;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>>
    where
        K: de::DeserializeSeed<'de>,
    {
        match self.parse_element_type() {
            Ok(ElementType::SectionStart) => {
                self.level = Some(self.level.map_or(1, |l| l + 1));
                self.state = State::SectionKey;
                seed.deserialize(&mut **self).map(Some)
            },
            Ok(ElementType::ListStart) => {
                self.level = Some(self.level.map_or(1, |l| l + 1));
                self.state = State::ListName;
                seed.deserialize(&mut **self).map(Some)
            },
            Ok(ElementType::KeyValue) => {
                self.state = State::Key;
                seed.deserialize(&mut **self).map(Some)
            },
            Ok(ElementType::SectionEnd) if self.level.is_some() => {
                self.level = self.level.map(|l| l - 1).filter(|&l| l > 0);
                self.state = State::None;
                Ok(None)
            },
            Ok(v) => Err(Error::data(
                ErrorCode::Message("unexpected element type".into()),
                Some(v as u8),
                Some(self.read.position()),
            )),
            Err(e) if e.is_eof() && self.level.is_none() => Ok(None),
            Err(e) => Err(e),
        }
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value>
    where
        V: de::DeserializeSeed<'de>,
    {
        match &self.state {
            State::ListName => {
                self.state = State::ListItem(ListElement::String);
                seed.deserialize(&mut **self)
            },
            State::Key => {
                self.state = State::Value;
                seed.deserialize(&mut **self)
            },
            _ => {
                self.state = State::None;
                seed.deserialize(&mut **self)
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use indexmap::{indexmap, IndexMap};
    use pretty_assertions::assert_eq;
    use serde_derive::Deserialize;

    use super::*;

    #[test]
    fn deserialize_reader_example() {
        #[derive(Debug, Deserialize, Eq, PartialEq)]
        struct RootSection {
            key1: String,
            section1: MainSection,
        }

        #[derive(Debug, Deserialize, Eq, PartialEq)]
        struct MainSection {
            #[serde(rename = "sub-section")]
            sub_section: Option<SubSection>,
            list1: Vec<String>,
        }

        #[derive(Debug, Deserialize, Eq, PartialEq)]
        struct SubSection {
            key2: String,
        }

        #[rustfmt::skip]
        let data: &[_] = &[
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
        ];

        let actual: RootSection = from_reader(data).unwrap();
        assert_eq!(
            actual,
            RootSection {
                key1: "value1".to_string(),
                section1: MainSection {
                    sub_section: Some(SubSection {
                        key2: "value2".to_string(),
                    }),
                    list1: vec!["item1".to_string(), "item2".to_string()],
                },
            }
        );
    }

    #[test]
    fn deserialize_reader_none() {
        #[derive(Debug, Deserialize, Eq, PartialEq)]
        struct RootSection {
            key1: String,
            section1: MainSection,
        }

        #[derive(Debug, Deserialize, Eq, PartialEq)]
        struct MainSection {
            #[serde(rename = "sub-section")]
            sub_section: Option<SubSection>,
            list1: Vec<String>,
        }

        #[derive(Debug, Deserialize, Eq, PartialEq)]
        struct SubSection {
            key2: String,
        }

        #[rustfmt::skip]
        let data: &[_] = &[
            // key1 = value1
            3, 4, b'k', b'e', b'y', b'1', 0, 6, b'v', b'a', b'l', b'u', b'e', b'1',
            // section1
            1, 8, b's', b'e', b'c', b't', b'i', b'o', b'n', b'1',
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
        ];

        let actual: RootSection = from_reader(data).unwrap();
        assert_eq!(
            actual,
            RootSection {
                key1: "value1".to_string(),
                section1: MainSection {
                    sub_section: None,
                    list1: vec!["item1".to_string(), "item2".to_string()],
                },
            }
        );
    }

    #[test]
    fn deserialize_reader_pools() {
        #[derive(Debug, Deserialize, Eq, PartialEq)]
        struct Pool {
            base: String,
            size: u32,
            online: u32,
            offline: u32,
            leases: Vec<Lease>,
        }

        #[derive(Debug, Deserialize, Eq, PartialEq)]
        struct Lease {
            address: String,
            identity: Option<String>,
            status: Status,
        }

        #[derive(Debug, Deserialize, Eq, PartialEq)]
        enum Status {
            #[serde(rename = "online")]
            Online,
            #[serde(rename = "offline")]
            Offline,
        }

        #[rustfmt::skip]
        let data: &[_] = &[
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
        ];

        let actual: IndexMap<String, Pool> = from_reader(data).unwrap();
        assert_eq!(
            actual,
            indexmap! {
                "pool-01".to_string() => Pool {
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
            }
        );
    }

    #[test]
    fn deserialize_slice_example() {
        #[derive(Debug, Deserialize, Eq, PartialEq)]
        struct RootSection<'a> {
            key1: &'a str,
            section1: MainSection<'a>,
        }

        #[derive(Debug, Deserialize, Eq, PartialEq)]
        struct MainSection<'a> {
            #[serde(borrow, rename = "sub-section")]
            sub_section: Option<SubSection<'a>>,
            list1: Vec<&'a str>,
        }

        #[derive(Debug, Deserialize, Eq, PartialEq)]
        struct SubSection<'a> {
            key2: &'a str,
        }

        #[rustfmt::skip]
        let data = &[
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
        ];

        let actual: RootSection = from_slice(data).unwrap();
        assert_eq!(
            actual,
            RootSection {
                key1: "value1",
                section1: MainSection {
                    sub_section: Some(SubSection { key2: "value2" }),
                    list1: vec!["item1", "item2",],
                },
            }
        );
    }

    #[test]
    fn deserialize_slice_none() {
        #[derive(Debug, Deserialize, Eq, PartialEq)]
        struct RootSection<'a> {
            key1: &'a str,
            section1: MainSection<'a>,
        }

        #[derive(Debug, Deserialize, Eq, PartialEq)]
        struct MainSection<'a> {
            #[serde(borrow, rename = "sub-section")]
            sub_section: Option<SubSection<'a>>,
            list1: Vec<&'a str>,
        }

        #[derive(Debug, Deserialize, Eq, PartialEq)]
        struct SubSection<'a> {
            key2: &'a str,
        }

        #[rustfmt::skip]
        let data = &[
            // key1 = value1
            3, 4, b'k', b'e', b'y', b'1', 0, 6, b'v', b'a', b'l', b'u', b'e', b'1',
            // section1
            1, 8, b's', b'e', b'c', b't', b'i', b'o', b'n', b'1',
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
        ];

        let actual: RootSection = from_slice(data).unwrap();
        assert_eq!(
            actual,
            RootSection {
                key1: "value1",
                section1: MainSection {
                    sub_section: None,
                    list1: vec!["item1", "item2",],
                },
            }
        );
    }

    #[test]
    fn deserialize_slice_pools() {
        #[derive(Debug, Deserialize, Eq, PartialEq)]
        struct Pool<'a> {
            base: &'a str,
            size: u32,
            online: u32,
            offline: u32,
            #[serde(borrow)]
            leases: Vec<Lease<'a>>,
        }

        #[derive(Debug, Deserialize, Eq, PartialEq)]
        struct Lease<'a> {
            address: &'a str,
            identity: Option<&'a str>,
            status: Status,
        }

        #[derive(Debug, Deserialize, Eq, PartialEq)]
        enum Status {
            #[serde(rename = "online")]
            Online,
            #[serde(rename = "offline")]
            Offline,
        }

        #[rustfmt::skip]
        let data = &[
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
        ];

        let actual: IndexMap<&str, Pool> = from_slice(data).unwrap();
        assert_eq!(
            actual,
            indexmap! {
                "pool-01" => Pool {
                    base: "192.0.2.1",
                    size: 4,
                    online: 3,
                    offline: 1,
                    leases: vec![
                        Lease {
                            address: "192.0.2.2",
                            identity: Some("identity-01"),
                            status: Status::Online,
                        },
                        Lease {
                            address: "192.0.2.3",
                            identity: Some("identity-02"),
                            status: Status::Online,
                        },
                        Lease {
                            address: "192.0.2.4",
                            identity: Some("identity-03"),
                            status: Status::Online,
                        },
                        Lease {
                            address: "192.0.2.5",
                            identity: None,
                            status: Status::Offline,
                        },
                    ],
                },
            }
        );
    }
}
