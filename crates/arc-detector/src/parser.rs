use crate::{
    budget::{Budget, Checked},
    lexer::{Lexer, Token},
    rules, Classification, Reason,
};

pub(crate) struct Parser<'a> {
    lexer: Lexer<'a>,
    look: Option<Token>,
    loaded: bool,
    context: Option<&'a crate::EndpointContext>,
    changed_directory: bool,
    caller_directory_fact_used: bool,
}
pub(crate) struct Invocation {
    pub words: Vec<String>,
    pub depth: usize,
    pub first_plain: bool,
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
            context: None,
            changed_directory: false,
            caller_directory_fact_used: false,
        }
    }
    pub(crate) fn with_context(source: &'a str, context: &'a crate::EndpointContext) -> Self {
        Self {
            context: Some(context),
            ..Self::new(source)
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
        let before = leaves.len();
        self.list(depth, false, b, &mut Output::Classes(leaves))?;
        // Endpoint facts describe the caller's directory, not the directory
        // reached by a shell builtin. Only retain wholly local workloads.
        if self.changed_directory
            && (self.caller_directory_fact_used
                || leaves.len() == before
                || leaves[before..]
                    .iter()
                    .any(|leaf| leaf.locality != crate::Locality::HostLocal))
        {
            return Err(Reason::UnresolvedScript);
        }
        Ok(())
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
                let mut assignments = usize::from(plain && assignment(&first, b)?);
                let mut words = vec![first];
                while matches!(self.peek(b)?, Some(Token::Word(..))) {
                    if let Some(Token::Word(word, plain)) = self.take(b)? {
                        if assignments == words.len() && plain && assignment(&word, b)? {
                            assignments += 1;
                        }
                        words.push(word);
                    }
                }
                // Every recognition visit is charged, including opaque arguments.
                b.charge(words.len())?;
                let Output::Classes(leaves) = output else {
                    let Output::Commands(commands) = output else {
                        unreachable!()
                    };
                    commands.push(Invocation {
                        words,
                        depth,
                        first_plain: plain,
                    });
                    return Ok(());
                };
                if self.context.is_some()
                    && assignments == 0
                    && plain
                    && words.len() == 2
                    && b.eq(&words[0], "cd")?
                    && !words[1].is_empty()
                    && !words[1].starts_with('-')
                {
                    self.changed_directory = true;
                    return Ok(());
                }
                if b.one_of(&words[0], &["sh", "bash", "zsh"])? {
                    if words.len() == 3 && b.one_of(&words[1], &["-c", "-lc"])? {
                        // Noninteractive shells may source startup files that
                        // change command resolution and endpoint selectors.
                        if self.context.is_some_and(|context| {
                            words[0] != "sh"
                                && (words[1] != "-c"
                                    || !crate::locality::safe_shell_startup(context, &words[0]))
                        }) {
                            b.charge(1)?;
                            leaves.push(Classification::unknown(Reason::UnresolvedScript));
                            return Ok(());
                        }
                        b.depth(depth + 1)?;
                        // Borrow decoded storage: no additional nested-input copy.
                        let before = leaves.len();
                        let mut nested = match self.context {
                            Some(context) => Parser::with_context(&words[2], context),
                            None => Parser::new(&words[2]),
                        };
                        nested.parse(depth + 1, b, leaves)?;
                        self.caller_directory_fact_used |= nested.caller_directory_fact_used;
                        if leaves.len() == before {
                            b.charge(1)?;
                            leaves.push(Classification::unknown(Reason::EmptyCommand));
                        }
                    } else {
                        b.charge(1)?;
                        leaves.push(Classification::unknown(Reason::UnresolvedScript));
                    }
                } else {
                    let result = match self.context {
                        Some(context) => {
                            crate::locality::recognize_shell(&words, assignments, context, b)?
                        }
                        None => rules::recognize(&words, b)?,
                    };
                    if words.first().is_some_and(|word| word == "make")
                        && result.locality == crate::Locality::HostLocal
                    {
                        self.caller_directory_fact_used = true;
                    }
                    b.charge(1)?; // effective child consumed
                    leaves.push(result);
                }
                Ok(())
            }
            _ => Err(Reason::UnsupportedSyntax),
        }
    }
}

fn assignment(word: &str, b: &mut Budget) -> Checked<bool> {
    b.charge(1 + word.len())?;
    let Some((name, _)) = word.split_once('=') else {
        return Ok(false);
    };
    let mut bytes = name.bytes();
    Ok(bytes
        .next()
        .is_some_and(|ch| ch == b'_' || ch.is_ascii_alphabetic())
        && bytes.all(|ch| ch == b'_' || ch.is_ascii_alphanumeric()))
}
