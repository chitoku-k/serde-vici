use std::{collections::VecDeque, io, ops::Deref, str};

use crate::{
    error::{Error, ErrorCode},
    ElementType,
};

pub trait Read<'de> {
    fn position(&self) -> usize;
    fn peek_key(&mut self) -> Result<usize, Error>;
    fn peek_value(&mut self) -> Result<usize, Error>;
    fn parse_key<'s>(&mut self, scratch: &'s mut Vec<u8>) -> Result<Reference<'de, 's, str>, Error>;
    fn parse_value<'s>(&mut self, scratch: &'s mut Vec<u8>) -> Result<Reference<'de, 's, str>, Error>;
    fn parse_element_type(&mut self) -> Result<ElementType, Error>;
}

pub enum Reference<'b, 'c, T>
where
    T: ?Sized + 'static,
{
    Borrowed(&'b T),
    Copied(&'c T),
}

impl<'b, 'c, T> Deref for Reference<'b, 'c, T>
where
    T: ?Sized + 'static,
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        match *self {
            Reference::Borrowed(b) => b,
            Reference::Copied(c) => c,
        }
    }
}

pub struct IoRead<R> {
    reader: R,
    buf: VecDeque<u8>,
    pos: usize,
}

pub struct SliceRead<'a> {
    slice: &'a [u8],
    pos: usize,
}

fn key_size(b: Option<&u8>) -> Option<usize> {
    b.map(|&b| b as usize)
}

fn value_size(h: Option<&u8>, l: Option<&u8>) -> Option<usize> {
    match (h, l) {
        (Some(&h), Some(&l)) => {
            let h = h as usize;
            let l = l as usize;
            Some((h << 8) + l)
        },
        _ => None,
    }
}

impl<R> IoRead<R>
where
    R: io::Read,
{
    pub fn new(reader: R) -> Self {
        let buf = VecDeque::with_capacity(128);
        let pos = 0;
        Self { reader, buf, pos }
    }

    fn fill_buf(&mut self) -> Result<Option<()>, Error> {
        let mut buf = [0; 128];
        match self.reader.read(&mut buf)? {
            0 => Ok(None),
            size => {
                for v in buf[..size].iter() {
                    self.buf.push_back(*v);
                }
                Ok(Some(()))
            },
        }
    }

    #[inline]
    fn as_str<'s>(&self, s: &'s [u8]) -> Result<&'s str, Error> {
        str::from_utf8(s).map_err(|e| {
            Error::data(
                ErrorCode::InvalidUnicodeCodePoint,
                s.get(e.valid_up_to()).copied(),
                Some(self.pos + e.valid_up_to()),
            )
        })
    }
}

impl<'de, R> Read<'de> for IoRead<R>
where
    R: io::Read,
{
    fn position(&self) -> usize {
        self.pos
    }

    fn peek_key(&mut self) -> Result<usize, Error> {
        loop {
            if let Some(size) = key_size(self.buf.get(0)) {
                return Ok(size);
            }

            self.fill_buf()?
                .ok_or_else(|| Error::data(ErrorCode::EofWhileParsingKey, None, Some(self.pos)))?;
        }
    }

    fn peek_value<'s>(&mut self) -> Result<usize, Error> {
        loop {
            if let Some(size) = value_size(self.buf.get(0), self.buf.get(1)) {
                return Ok(size);
            }

            self.fill_buf()?
                .ok_or_else(|| Error::data(ErrorCode::EofWhileParsingValue, None, Some(self.pos)))?;
        }
    }

    fn parse_key<'s>(&mut self, scratch: &'s mut Vec<u8>) -> Result<Reference<'de, 's, str>, Error> {
        loop {
            if let Some(size) = key_size(self.buf.get(0)) {
                if size < self.buf.len() {
                    self.buf.drain(..1);
                    self.pos += 1;

                    for v in self.buf.drain(..size) {
                        scratch.push(v);
                    }

                    let key = self.as_str(scratch)?;
                    self.pos += size;

                    return Ok(Reference::Copied(key));
                }
            }

            self.fill_buf()?
                .ok_or_else(|| Error::data(ErrorCode::EofWhileParsingKey, None, Some(self.pos)))?;
        }
    }

    fn parse_value<'s>(&mut self, scratch: &'s mut Vec<u8>) -> Result<Reference<'de, 's, str>, Error> {
        loop {
            if let Some(size) = value_size(self.buf.get(0), self.buf.get(1)) {
                if size < self.buf.len() - 1 {
                    self.buf.drain(..2);
                    self.pos += 2;

                    for v in self.buf.drain(..size) {
                        scratch.push(v);
                    }

                    let value = self.as_str(scratch)?;
                    self.pos += size;

                    return Ok(Reference::Copied(value));
                }
            }

            self.fill_buf()?
                .ok_or_else(|| Error::data(ErrorCode::EofWhileParsingValue, None, Some(self.pos)))?;
        }
    }

    fn parse_element_type(&mut self) -> Result<ElementType, Error> {
        loop {
            if let Some(result) = self.buf.pop_front().map(ElementType::try_from) {
                match result {
                    Ok(v) => {
                        self.pos += 1;
                        return Ok(v);
                    },
                    Err(e) => {
                        return Err(Error::data(
                            ErrorCode::Message("invalid element type".into()),
                            Some(e.number),
                            Some(self.pos),
                        ));
                    },
                }
            }

            self.fill_buf()?
                .ok_or_else(|| Error::data(ErrorCode::EofWhileParsingElementType, None, Some(self.pos)))?;
        }
    }
}

impl<'a> SliceRead<'a> {
    pub fn new(slice: &'a [u8]) -> Self {
        let pos = 0;
        Self { slice, pos }
    }

    #[inline]
    fn as_str<'s>(&self, s: &'s [u8]) -> Result<&'s str, Error> {
        str::from_utf8(s).map_err(|e| {
            Error::data(
                ErrorCode::InvalidUnicodeCodePoint,
                s.get(e.valid_up_to()).copied(),
                Some(self.pos + e.valid_up_to()),
            )
        })
    }
}

impl<'a> Read<'a> for SliceRead<'a> {
    fn position(&self) -> usize {
        self.pos
    }

    fn peek_key(&mut self) -> Result<usize, Error> {
        if let Some(size) = key_size(self.slice.get(self.pos)) {
            return Ok(size);
        }

        Err(Error::data(ErrorCode::EofWhileParsingKey, None, Some(self.pos)))
    }

    fn peek_value(&mut self) -> Result<usize, Error> {
        if let Some(size) = value_size(self.slice.get(self.pos), self.slice.get(self.pos + 1)) {
            return Ok(size);
        }

        Err(Error::data(ErrorCode::EofWhileParsingValue, None, Some(self.pos)))
    }

    fn parse_key<'s>(&mut self, _scratch: &'s mut Vec<u8>) -> Result<Reference<'a, 's, str>, Error> {
        if let Some(size) = key_size(self.slice.get(self.pos)) {
            if let Some(s) = self.slice.get((self.pos + 1)..(self.pos + 1 + size)) {
                self.pos += 1;

                let key = self.as_str(s)?;
                self.pos += size;

                return Ok(Reference::Borrowed(key));
            }
        }

        Err(Error::data(ErrorCode::EofWhileParsingKey, None, Some(self.pos)))
    }

    fn parse_value<'s>(&mut self, _scratch: &'s mut Vec<u8>) -> Result<Reference<'a, 's, str>, Error> {
        if let Some(size) = value_size(self.slice.get(self.pos), self.slice.get(self.pos + 1)) {
            if let Some(s) = self.slice.get((self.pos + 2)..(self.pos + 2 + size)) {
                self.pos += 2;

                let key = self.as_str(s)?;
                self.pos += size;

                return Ok(Reference::Borrowed(key));
            }
        }

        Err(Error::data(ErrorCode::EofWhileParsingValue, None, Some(self.pos)))
    }

    fn parse_element_type(&mut self) -> Result<ElementType, Error> {
        if let Some(result) = self.slice.get(self.pos).copied().map(ElementType::try_from) {
            match result {
                Ok(v) => {
                    self.pos += 1;
                    return Ok(v);
                },
                Err(e) => {
                    return Err(Error::data(
                        ErrorCode::Message("invalid element type".into()),
                        Some(e.number),
                        Some(self.pos),
                    ));
                },
            }
        }

        Err(Error::data(ErrorCode::EofWhileParsingElementType, None, Some(self.pos)))
    }
}
