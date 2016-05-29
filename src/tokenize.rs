#[derive(PartialEq)]
enum Category {
    Whitespace,
    Delimiter,
    Character,
    Numeric,
}

#[derive(Clone, PartialEq, Debug)]
pub struct Position {
    pub line : usize, // line number
    pub col : usize,  // column number
    pub pos : usize,  // position in input string for indexing
}

impl Position {

    pub fn new() -> Position {
        Position { line: 1, col: 0, pos: 0 }
    }

    pub fn update(&mut self, ch : char) {
        if ch == '\n' {
            self.line += 1;
            self.col = 0;
        } else {
            self.col += 1;
        }
        // increment position in input string
        self.pos += 1;
    }
}

#[derive(Clone,PartialEq)]
pub struct Token {
    pub beg : Position,
    pub end : Position,
}

pub struct Tokenizer<F> where F : FnMut(Token) -> () {
    callback : F,

    // non zero if inside a quoted string
    // quoting contains the char that started the quote
    quoting : char,

    // true if escaping the next char
    escaping: bool,

    // previous char
    prev : char,

    // current position in the string
    pos : Position,

    // current token begin position
    beg : Position,
}

impl<F> Tokenizer<F> where F : FnMut(Token) -> () {

    pub fn new(callback : F) -> Tokenizer<F> {
        Tokenizer {
            quoting: '\0',
            escaping: false,
            prev: '\0',
            callback: callback,
            beg : Position::new(),
            pos : Position::new(),
        }
    }

    fn char_category(ch : char) -> Category {
        match ch {
            '0' | '1' | '2' | '3' | '4' | '5' | '6' | '7' | '8' | '9' => Category::Numeric,
            '(' | ')' | '|' | ':' | '-' | '>' | '<' | ';' | '`' | '"' | '\'' | '\\' => Category::Delimiter,
            ' ' | '\t' | '\n' | '\r' => Category::Whitespace,
            _ => Category::Character,
        }
    }

    /**
     * Add character to current token
     *
     * Increments the current position
     * Saves the char as the previous char
     */
    fn add_char(&mut self, ch : char) {
        self.pos.update(ch);
        self.prev = ch;
    }

    fn flush(&mut self) {
        // do not flush if prev category was a whitespace
        // or if token is empty (zero sized)
        if Tokenizer::<F>::char_category(self.prev) != Category::Whitespace
            && self.beg.pos != self.pos.pos {
            let t = Token {
                beg : self.beg.clone(),
                end : self.pos.clone(),
            };
            let ref mut x = self.callback;
            x(t);
        }
    }

    /**
     * Push char, do not start quoting
     * Flush if necessary
     */
    fn maybe_start_token(&mut self, ch : char) {
        let char_category = Tokenizer::<F>::char_category(ch);
        let prev_category = Tokenizer::<F>::char_category(self.prev);

        match char_category {
            Category::Whitespace => {
                if prev_category != Category::Whitespace {
                    // whitespace, flush token if prev char was not a whitespace
                    self.flush();
                    self.beg = self.pos.clone();
                }
            }
            Category::Delimiter => {
                // delimiter never continues, even if prev char was a delimiter
                // skip flush if prev category was Whitespace
                self.flush();
                self.beg = self.pos.clone();
            }
            Category::Character => {
                if prev_category != Category::Character {
                    self.flush();
                    self.beg = self.pos.clone();
                }
            }
            Category::Numeric => {
                if prev_category != Category::Numeric {
                    self.flush();
                    self.beg = self.pos.clone();
                }
            }
        }
    }

    fn push_normal(&mut self, ch : char) {
        assert!(self.quoting == '\0');
        if ch == '\'' || ch == '"' {
            // flush any previous token since quoting is starting
            // a"b" -> two tokens: a and "b"
            self.flush();
            // token should include the starting quote
            self.beg = self.pos.clone();
            self.add_char(ch);
            // remember the char type that started the quoting
            self.quoting = ch;
        } else {
            self.maybe_start_token(ch);
            self.add_char(ch);
        }
    }

    /**
     * Push a character into the tokenizer
     */
    pub fn push(&mut self, ch : char) {

        // escape changes only the interpretation
        // of the next char (quotes do not start quoting)
        //
        // output token positions will still include ranges with the escape
        // chars, to be consistent with escaping inside quotes
        //
        // to remove escape chars, the output needs to be post-processed
        // above the tokenizer level
        if self.escaping {
            // current char is escaped
            // (quoting is ignored)
            self.maybe_start_token(ch);
            self.add_char('\\');
            self.add_char(ch);
            self.escaping = false;
        } else if ch == '\\' {
            // do not update position yet
            // do not update self.prev to not break the next push_noquote() call
            // next char will be escaped
            self.escaping = true;
        } else {

            if self.quoting != '\0' {
                // the token range will include the ending quote
                self.add_char(ch);

                if ch == self.quoting {
                    self.flush();
                    self.beg = self.pos.clone();
                    self.quoting = '\0';
                }
            } else {
                self.push_normal(ch);
            }
        }
    }

    pub fn finish(&mut self) {
        self.flush();
    }
}
