use crate::{
    budget::{Budget, Checked},
    Reason,
};

#[derive(Debug)]
pub(crate) enum Token {
    Word(String, bool),
    Open,
    Close,
    Pipe,
    And,
    Or,
    Semi,
    Newline,
}

pub(crate) struct Lexer<'a> {
    source: &'a [u8],
    pos: usize,
}
impl<'a> Lexer<'a> {
    pub(crate) fn new(source: &'a str) -> Self {
        Self {
            source: source.as_bytes(),
            pos: 0,
        }
    }
    fn byte(&self, at: usize, b: &mut Budget) -> Checked<Option<u8>> {
        b.charge(1)?;
        Ok(self.source.get(at).copied())
    }
    fn copy(out: &mut Vec<u8>, value: u8, b: &mut Budget) -> Checked<()> {
        b.charge(1)?;
        out.push(value);
        Ok(())
    }
    fn newline(&mut self, b: &mut Budget) -> Checked<()> {
        if self.byte(self.pos, b)? == Some(b'\r') {
            if self.byte(self.pos + 1, b)? != Some(b'\n') {
                return Err(Reason::UnsupportedSyntax);
            }
            self.pos += 1;
        }
        self.pos += 1;
        Ok(())
    }
    pub(crate) fn next(&mut self, b: &mut Budget) -> Checked<Option<Token>> {
        use Token::*;
        let first = loop {
            match self.byte(self.pos, b)? {
                Some(b' ' | b'\t') => self.pos += 1,
                Some(b'\\') if matches!(self.byte(self.pos + 1, b)?, Some(b'\n' | b'\r')) => {
                    self.pos += 1;
                    self.newline(b)?;
                }
                other => break other,
            }
        };
        let Some(first) = first else {
            return Ok(None);
        };
        // The token slot is reserved before decoding/allocating the next word.
        b.token()?;
        let token = match first {
            b'(' => {
                self.pos += 1;
                Open
            }
            b')' => {
                self.pos += 1;
                Close
            }
            b';' => {
                self.pos += 1;
                Semi
            }
            b'\n' | b'\r' => {
                self.newline(b)?;
                Newline
            }
            b'|' | b'&' => {
                self.pos += 1;
                if self.byte(self.pos, b)? == Some(first) {
                    self.pos += 1;
                    if first == b'|' {
                        Or
                    } else {
                        And
                    }
                } else if first == b'|' {
                    Pipe
                } else {
                    return Err(Reason::UnsupportedSyntax);
                }
            }
            _ => {
                let (word, plain) = self.word(b)?;
                Word(word, plain)
            }
        };
        Ok(Some(token))
    }
    fn word(&mut self, b: &mut Budget) -> Checked<(String, bool)> {
        let mut plain = true;
        let mut out = Vec::new();
        let mut quote = None;
        loop {
            let Some(ch) = self.byte(self.pos, b)? else {
                if quote.is_some() {
                    return Err(Reason::UnsupportedSyntax);
                }
                break;
            };
            if ch < 32 && !matches!(ch, b'\t' | b'\r' | b'\n') || ch == 127 {
                return Err(Reason::UnsupportedSyntax);
            }
            if ch == b'\r' && self.byte(self.pos + 1, b)? != Some(b'\n') {
                return Err(Reason::UnsupportedSyntax);
            }
            if quote == Some(b'\'') {
                self.pos += 1;
                if ch == b'\'' {
                    quote = None;
                } else {
                    Self::copy(&mut out, ch, b)?;
                }
                continue;
            }
            if quote.is_none()
                && matches!(
                    ch,
                    b' ' | b'\t' | b'\n' | b'\r' | b'(' | b')' | b';' | b'|' | b'&'
                )
            {
                break;
            }
            if ch == b'"' || ch == b'\'' && quote.is_none() {
                plain = false;
                self.pos += 1;
                quote = if quote.is_some() { None } else { Some(ch) };
                continue;
            }
            if matches!(ch, b'$' | b'`') {
                return Err(Reason::UnresolvedExpansion);
            }
            if quote.is_none() {
                if matches!(ch, b'*' | b'?' | b'[' | b'~') {
                    return Err(Reason::UnresolvedExpansion);
                }
                if matches!(ch, b'<' | b'>' | b'#' | b'{' | b'}') {
                    return Err(Reason::UnsupportedSyntax);
                }
            }
            self.pos += 1;
            if ch == b'\\' {
                let next = self.byte(self.pos, b)?.ok_or(Reason::UnsupportedSyntax)?;
                if matches!(next, b'\n' | b'\r') {
                    self.newline(b)?;
                    continue;
                }
                plain = false;
                if quote.is_none() || matches!(next, b'\\' | b'"' | b'$' | b'`') {
                    if next < 32 && next != b'\t' || next == 127 {
                        return Err(Reason::UnsupportedSyntax);
                    }
                    self.pos += 1;
                    Self::copy(&mut out, next, b)?;
                    continue;
                }
            }
            Self::copy(&mut out, ch, b)?;
        }
        // UTF-8 validation also inspects bytes and is charged before library work.
        b.charge(out.len())?;
        String::from_utf8(out)
            .map(|word| (word, plain))
            .map_err(|_| Reason::UnsupportedSyntax)
    }
}
