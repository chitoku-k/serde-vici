use std::{collections::VecDeque, io, ops::Deref, str};

use crate::{
    error::{Error, ErrorCode},
    ElementType,
};

pub trait Read<'de> {
    fn position(&self) -> usize;
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

    fn parse_key<'s>(&mut self, scratch: &'s mut Vec<u8>) -> Result<Reference<'de, 's, str>, Error> {
        loop {
            if let Some(&v) = self.buf.get(0) {
                let size = v as usize;
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
            if let Some((&h, &l)) = self.buf.get(0).zip(self.buf.get(1)) {
                let h = h as usize;
                let l = l as usize;

                let size = (h << 8) + l;
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

    fn parse_key<'s>(&mut self, _scratch: &'s mut Vec<u8>) -> Result<Reference<'a, 's, str>, Error> {
        if let Some(&v) = self.slice.get(self.pos) {
            let size = v as usize;
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
        if let Some((&h, &l)) = self.slice.get(self.pos).zip(self.slice.get(self.pos + 1)) {
            let h = h as usize;
            let l = l as usize;

            let size = (h << 8) + l;
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
