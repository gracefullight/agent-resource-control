use crate::{
    budget::{Budget, Checked},
    lexer::{Lexer, Token},
    rules, Classification, Reason,
};

pub(crate) struct Parser<'a> {
    lexer: Lexer<'a>,
    look: Option<Token>,
    loaded: bool,
}
pub(crate) struct Invocation {
    pub words: Vec<String>,
    pub depth: usize,
}
enum Output<'a> {
    Classes(&'a mut Vec<Classification>),
    Commands(&'a mut Vec<Invocation>),
}
impl<'a> Parser<'a> {
    pub(crate) fn new(source: &'a str) -> Self {
        Self {
            lexer: Lexer::new(source),
            look: None,
            loaded: false,
        }
    }
    fn peek(&mut self, b: &mut Budget) -> Checked<&Option<Token>> {
        b.charge(1)?;
        if !self.loaded {
            self.look = self.lexer.next(b)?;
            self.loaded = true;
        }
        Ok(&self.look)
    }
    fn take(&mut self, b: &mut Budget) -> Checked<Option<Token>> {
        self.peek(b)?;
        b.charge(1)?;
        self.loaded = false;
        Ok(self.look.take())
    }
    fn blank_lines(&mut self, b: &mut Budget) -> Checked<()> {
        while matches!(self.peek(b)?, Some(Token::Newline)) {
            self.take(b)?;
        }
        Ok(())
    }
    pub(crate) fn parse(
        &mut self,
        depth: usize,
        b: &mut Budget,
        leaves: &mut Vec<Classification>,
    ) -> Checked<()> {
        self.list(depth, false, b, &mut Output::Classes(leaves))
    }
    /// The same restricted grammar, with recognition deferred to the script
    /// resolver. Neither parser entry point evaluates shell syntax.
    pub(crate) fn commands(
        &mut self,
        depth: usize,
        b: &mut Budget,
        commands: &mut Vec<Invocation>,
    ) -> Checked<()> {
        self.list(depth, false, b, &mut Output::Commands(commands))
    }
    fn list(
        &mut self,
        depth: usize,
        grouped: bool,
        b: &mut Budget,
        output: &mut Output<'_>,
    ) -> Checked<()> {
        b.depth(depth)?;
        self.blank_lines(b)?;
        if self.peek(b)?.is_none() && !grouped {
            return Ok(());
        }
        self.primary(depth, b, output)?;
        loop {
            match self.take(b)? {
                None if !grouped => return Ok(()),
                Some(Token::Close) if grouped => return Ok(()),
                Some(Token::Semi | Token::Newline) => {
                    self.blank_lines(b)?;
                    if matches!(self.peek(b)?, None | Some(Token::Close)) {
                        continue;
                    }
                }
                Some(Token::Pipe | Token::And | Token::Or) => {}
                _ => return Err(Reason::UnsupportedSyntax),
            }
            self.primary(depth, b, output)?;
        }
    }
    fn primary(&mut self, depth: usize, b: &mut Budget, output: &mut Output<'_>) -> Checked<()> {
        match self.take(b)? {
            Some(Token::Open) => {
                b.depth(depth + 1)?;
                b.charge(1)?; // group node
                self.list(depth + 1, true, b, output)
            }
            Some(Token::Word(first, plain)) => {
                b.invocation()?;
                if plain
                    && b.one_of(
                        &first,
                        &[
                            "if", "then", "else", "elif", "fi", "for", "while", "until", "do",
                            "done", "case", "esac", "function", "select", "in", "!",
                        ],
                    )?
                {
                    return Err(Reason::UnsupportedSyntax);
                }
                let mut words = vec![first];
                while matches!(self.peek(b)?, Some(Token::Word(..))) {
                    if let Some(Token::Word(word, _)) = self.take(b)? {
                        words.push(word);
                    }
                }
                // Every recognition visit is charged, including opaque arguments.
                b.charge(words.len())?;
                let Output::Classes(leaves) = output else {
                    let Output::Commands(commands) = output else {
                        unreachable!()
                    };
                    commands.push(Invocation { words, depth });
                    return Ok(());
                };
                if b.one_of(&words[0], &["sh", "bash", "zsh"])? {
                    if words.len() == 3 && b.one_of(&words[1], &["-c", "-lc"])? {
                        b.depth(depth + 1)?;
                        // Borrow decoded storage: no additional nested-input copy.
                        let before = leaves.len();
                        Parser::new(&words[2]).parse(depth + 1, b, leaves)?;
                        if leaves.len() == before {
                            b.charge(1)?;
                            leaves.push(Classification::unknown(Reason::EmptyCommand));
                        }
                    } else {
                        b.charge(1)?;
                        leaves.push(Classification::unknown(Reason::UnresolvedScript));
                    }
                } else {
                    let result = rules::recognize(&words, b)?;
                    b.charge(1)?; // effective child consumed
                    leaves.push(result);
                }
                Ok(())
            }
            _ => Err(Reason::UnsupportedSyntax),
        }
    }
}
