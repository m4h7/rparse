use std::collections::HashMap;

// if the char is A-Z return lowercase a-z
fn asciilowerchar(a : char) -> char {
    if a >= 'A' && a <= 'Z' {
        let ai = a as u8;
        let ax = 'A' as u8;
        let ay = 'a' as u8;
        (ai - ax + ay) as char
    } else {
        a
    }
}

#[derive(Debug)]
pub struct HTMLToken {
    attribs : Option<HashMap<String, String>>,
    pub value : String, // tag name if opens||closes or text
}

impl HTMLToken {

    fn parse_attribs(v : &[char]) -> Option<HashMap<String, String>> {
        let mut j = 0;
        let mut r : HashMap<String, String> = HashMap::new();
        while j < v.len() {
            let k_start = j;
            while j < v.len() && v[j] != '=' && v[j] != ' ' {
                j += 1;
            }
            let k_end = j;

            if j < v.len() && v[j] == '=' {
                // skip the '='
                j += 1;
                // remember the start of the value string
                let mut v_start = j;
                let v_end;
                // check if value is quoted
                if j < v.len() && v[j] == '\"' {
                    v_start += 1;
                    j += 1;
                    while j < v.len() && v[j] != '"' {
                        j += 1;
                    }
                    v_end = j;
                    if j < v.len() && v[j] == '"' {
                        j += 1;
                    }
                } else {
                    // find the end of the value string
                    while j < v.len() && v[j] != ' ' {
                        j += 1;
                    }
                    v_end = j;
                }
                let k : String = v[k_start..k_end].iter().cloned().collect();
                let v : String = v[v_start..v_end].iter().cloned().collect();
                r.insert(k, v);
            } else { // no value, just key
                let k : String = v[k_start..k_end].iter().cloned().collect();
                r.insert(k, String::new());
            }
            // skip whitespace
            while j < v.len() && v[j] == ' ' {
                j += 1;
            }
        }
        if r.len() > 0 {
            Some(r)
        } else {
            None
        }
    }

    pub fn parse(s : &str) -> HTMLToken{
        let mut v : Vec<char> = Vec::new();

        for (_, c) in s.char_indices() {
            v.push(c);
        }
        let mut r = String::new();
        // find space in 'v' if there is any
        let mut j = 0;
        while j < v.len() && v[j] != ' ' {
            r.push(asciilowerchar(v[j]));
            j += 1;
        }
        // skip the whitespace if any
        while j < v.len() && v[j] == ' ' {
            j += 1;
        }
        let attrib_start = j;
        // check for attribs, stop on '>' or '/>'
        while (j < v.len() && v[j] != '>') || (j + 1 < v.len() && v[j] != '/' && v[j+1] != '>') {
            j += 1;
        }
        let attrib_end = j;

        let mut attribs : Option<HashMap<String, String>> = None;

        if attrib_start != attrib_end {
            attribs = HTMLToken::parse_attribs(&v[attrib_start..attrib_end]);
        }
        // get the end of the token
        while j < v.len() {
            r.push(v[j]);
            j += 1;
        }
        HTMLToken {
            attribs : attribs,
            value : String::from(r)
        }
    }

    pub fn text(s : &str) -> HTMLToken {
        HTMLToken {
            attribs : None,
            value : String::from(s)
        }
    }
}

struct Buf {
    // current index into 'v'
    i : usize,
    // indices into 's'
    v : Vec<char>,
}

impl Buf {
    pub fn new(s : & str) -> Buf {
        let mut v : Vec<char> = Vec::new();
        for (_, c) in s.char_indices() {
            v.push(c);
        }
        return Buf {
            v : v,
            i : 0
        }
    }

    pub fn eos(&self) -> bool {
        self.i == self.v.len()
    }

    // pub fn lookahead(&self, find : &str) -> bool {
    //     let mut i = 0;
    //     for c in find.chars() {
    //         if self.v[self.i + i] != c {
    //             return false;
    //         }
    //         i += 1;
    //     }
    //     true
    // }

    /**
     * case insensitive char comparison function
     */
    fn ieq(a : char, b : char) -> bool {
        asciilowerchar(a) == asciilowerchar(b)
    }

    /**
     * Check if next chars match 'find', if so, skip until 'until'
     */
    pub fn lookahead_skip(&mut self, find : &str, until : &str) {
        let mut j = 0;
        for c in find.chars() {
            // exit if at end or not matched 'find'
            if self.i + j >= self.v.len() || !Buf::ieq(self.v[self.i + j], c) {
                return;
            }
            j += 1;
        }
        loop {
            let mut matched = true;
            let mut k = j;
            for c in until.chars() {
                if !Buf::ieq(self.v[self.i + k], c) {
                    matched = false;
                    break;
                }
                k += 1;
            }
            if matched {
                self.i += k;
                return;
            }
            j += 1
        }
    }

    pub fn peek(&self, ch : char) -> bool {
        if self.i < self.v.len() {
            self.v[self.i] == ch
        } else {
            false
        }
    }

    /**
     * Extract until char, include that char in the output string
     */
    pub fn extract_with(&mut self, until : char) -> String {
        let mut s = String::new();
        for j in self.i..self.v.len() {
            s.push(self.v[j]);
            if self.v[j] == until {
                self.i = j + 1;
                break;
            }
        }
        s
    }

    /**
     * extract until char, do not include it in the output string
     */
    pub fn extract_without(&mut self, until : char) -> String {
        let mut s = String::new();
        for j in self.i..self.v.len() {
            if self.v[j] == until {
                self.i = j;
                break;
            }
            s.push(self.v[j]);
        }
        s
    }

}

pub fn tokenize_html(s : &str) -> Vec<HTMLToken> {
    let mut b = Buf::new(s);

    let mut v : Vec<HTMLToken> = Vec::new();
    while !b.eos() {
        // skip comments
        b.lookahead_skip("<!--", "-->");
        // skip scripts
        b.lookahead_skip("<script", "</script>");
        // skip styles (embedded css or links)
        b.lookahead_skip("<style", "</style>");
        if b.peek('<') {
            let s = b.extract_with('>');
            let t = HTMLToken::parse(&s);
            v.push(t);
        } else { // text node
            let s = b.extract_without('<');
            // check is s is empty / whitespace
            let trimmed = s.trim();
            if trimmed.len() > 0 {
                v.push(HTMLToken::text(trimmed));
            }
        }
    }
    v
}
