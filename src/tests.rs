#[cfg(test)]
mod tests {

    extern crate core;

    use std::collections::VecDeque;
    use tokenize::{Tokenizer,Token};
    use compiler::{compile_grammar};
    use htmltokenize::{tokenize_html,HTMLToken};
    use vm::{run, StreamingHandler};

    struct ParsedData {
        counter: usize,
        term_counter: usize,
    }

    impl ParsedData {
        fn new() -> ParsedData {
            ParsedData {
                counter : 0,
                term_counter: 0,
            }
        }
        fn inc(&mut self) {
            self.counter += 1;
        }
        fn dec(&mut self) {
            self.counter -= 1;
        }
        fn inc_term(&mut self) {
            self.term_counter += 1;
        }
        fn count(&self) -> usize {
//            println!("counter ----> {}", self.counter);
            self.counter
        }
        fn term_count(&self) -> usize {
            self.term_counter
        }
    }

    impl StreamingHandler for ParsedData {
        fn start(&mut self, ntname: &String, name: &Option<&String>) {
            self.inc();
//            println!("--- start {} {:?} [{}]", ntname, name, self.count());
        }
        fn end(&mut self, ntname: &String, xname: &Option<&String>) {
            self.dec();
//            println!("--- end {} {:?} [{}]", ntname, xname, self.count());
        }
        fn term(&mut self, tokidx: usize, name: &Option<&String>) {
            self.inc_term();
//            println!("--- term = {} {:?}", tokidx, name);
        }
    }

    #[test]
    fn tokenizer_works() {
        let mut tokens : VecDeque<Token> = VecDeque::new();
        let input_str = "hello (world)+ 'q[ u ]o' \"dq\" \\\"x\\\"";
                       //012345678901234567890123456789

        {
            let mut t = Tokenizer::new(|t| { tokens.push_back(t); } );

            for ch in input_str.chars() {
                t.push(ch);
            }
            t.finish();
        }

        // check first token
        let t0 = tokens.pop_front().unwrap();
        assert_eq!(t0.beg.line, 1);
        assert_eq!(t0.beg.col, 0);
        assert_eq!(t0.end.line,1);
        assert_eq!(t0.end.col, 5);

        // check range returned for the token is correct
        let s0 = String::from(&input_str[t0.beg.col..t0.end.col]);
        assert_eq!(t0.beg.pos, 0);
        assert_eq!(t0.end.pos, 5);
        assert_eq!(s0, "hello");

        let t1 = tokens.pop_front().unwrap();
        let s1 = String::from(&input_str[t1.beg.col..t1.end.col]);
        assert_eq!(s1, "(");

        let t2 = tokens.pop_front().unwrap();
        let s2 = String::from(&input_str[t2.beg.pos..t2.end.pos]);
        assert_eq!(s2, "world");

        let t3 = tokens.pop_front().unwrap();
        let s3 = String::from(&input_str[t3.beg.pos..t3.end.pos]);
        assert_eq!(s3, ")");

        let t4 = tokens.pop_front().unwrap();
        let s4 = String::from(&input_str[t4.beg.pos..t4.end.pos]);
        assert_eq!(s4, "+");

        let t5 = tokens.pop_front().unwrap();
        let s5 = String::from(&input_str[t5.beg.pos..t5.end.pos]);
        assert_eq!(s5, "'q[ u ]o'");

        let t6 = tokens.pop_front().unwrap();
        let s6 = String::from(&input_str[t6.beg.pos..t6.end.pos]);
        assert_eq!(s6, "\"dq\"");

        let t7 = tokens.pop_front().unwrap();
        let s7 = String::from(&input_str[t7.beg.pos..t7.end.pos]);
        assert_eq!(s7, "\\\"");

        let t8 = tokens.pop_front().unwrap();
        let s8 = String::from(&input_str[t8.beg.pos..t8.end.pos]);
        assert_eq!(s8, "x");

        let t9 = tokens.pop_front().unwrap();
        let s9 = String::from(&input_str[t9.beg.pos..t9.end.pos]);
        assert_eq!(s9, "\\\"");

        assert!(tokens.is_empty());
    }

    #[test]
    fn load_grammar_test() {

        let gs = "WORLDTYPE : 'z' 'z' 'z' `z` |
                              'sunny'(sunnyname) 'world'(worldname) `wtyperule`;
                  OTHERTYPE : 'other'(othername) 'another'(anothername) `otherrule`;
                  START : 'begin'(beginname) WORLDTYPE(wtypent) OTHERTYPE 'end'(endname) `startrule`;";
        let c = compile_grammar(gs);
//        c.display();

        let mut tokens = Vec::<String>::new();
        tokens.push("begin".to_string());
        tokens.push("sunny".to_string());
        tokens.push("world".to_string());
        tokens.push("other".to_string());
        tokens.push("another".to_string());
        tokens.push("end".to_string());

        // "Y" - START grammar rule
        // &c - grammar to use
        // 3rd arg: match function
        let parsed_trees = run("START", &c, |s, i| { tokens[i] == s }, tokens.len());

        assert_eq!(parsed_trees.count(), 1);

        let mut d = ParsedData::new();
        parsed_trees.execute(0, &mut d);
        assert_eq!(d.count(), 0);
        assert_eq!(d.term_count(), 6);
//        print_ast(&ast, &tokens, 0);
    }

    #[test]
    fn empty_nt_test() {
        let gs = r#"
          A : 'a';
          E : ;
          Z : A E A A A;
        "#;
        let c = compile_grammar(gs);
        // c.display();

        let mut tokens = Vec::<String>::new();
        tokens.push("a".to_string());
        tokens.push("a".to_string());
        tokens.push("a".to_string());
        tokens.push("a".to_string());

        let pt = run("Z", &c, |s, i| { i < tokens.len() && tokens[i] == s }, 0);

        assert_eq!(pt.count(), 1);
    }

    #[test]
    fn empty2_nt_test() {
        let gs = r#"
          A : 'a';
          E : 'e' | ;
          X : A E A A A;
        "#;
        let c = compile_grammar(gs);
        // c.display();

        let mut tokens = Vec::<String>::new();
        tokens.push("a".to_string());
        tokens.push("a".to_string());
        tokens.push("a".to_string());
        tokens.push("a".to_string());

        let pt = run("X", &c, |s, i| { i < tokens.len() && tokens[i] == s }, 0);

        assert_eq!(pt.count(), 1);
    }

    #[test]
    fn rec_grammar_test() {
        let gs = r#"
          R
            : 'a' R
            | 'b'
            ;
        "#;
        let c = compile_grammar(gs);
        // c.display();

        let mut tokens = Vec::<String>::new();
        tokens.push("a".to_string());
        tokens.push("a".to_string());
        tokens.push("a".to_string());
        tokens.push("a".to_string());
        tokens.push("b".to_string());

        let pt = run("R", &c, |s, i| { tokens[i] == s }, 0);

        assert_eq!(pt.count(), 1);
    }

    #[test]
    fn rec2_grammar_test() {
        let gs = r#"
          A : 'w' ;
          Q : 'a' Q
            |
            ;
        "#;
        let c = compile_grammar(gs);
        c.display();

        let mut tokens = Vec::<String>::new();
        tokens.push("a".to_string());
        tokens.push("a".to_string());
        tokens.push("a".to_string());
        tokens.push("a".to_string());
        tokens.push("a".to_string());
        tokens.push("a".to_string());
        tokens.push("a".to_string());
        tokens.push("a".to_string());
        tokens.push("a".to_string());
        tokens.push("a".to_string());
        tokens.push("a".to_string());
        tokens.push("w".to_string());

        let pt = run("Q", &c, |s, i| { i < tokens.len() && tokens[i] == s }, 0);

        assert_eq!(pt.count_at_n(tokens.len() - 1), 1);
    }

    #[test]
    fn html_token_test() {
        let t0 = HTMLToken::parse("<body class=\"no-js\">");
        assert_eq!(t0.value, "<body>");

        let t1 = HTMLToken::parse("<BR/>");
        assert_eq!(t1.value, "<br/>");

        let t2 = HTMLToken::parse("<A HREF=\"#\">");
        assert_eq!(t2.value, "<a>");
        assert_eq!(t2.get_attrib_value("HREF"), Some("#".to_string()));

        let t3 = HTMLToken::parse("<a href=\"http://www.google.com\" target=\"_blank\">");
        assert_eq!(t3.value, "<a>");

        let t4 = HTMLToken::parse("<a href=\"http://www.bing.com/query?q=query\"/>");
        assert_eq!(t4.value, "<a/>");

        let t5 = HTMLToken::parse("<br >");
        assert_eq!(t5.value, "<br>");

        let t6 = HTMLToken::parse("<br />");
        assert_eq!(t6.value, "<br/>");
    }

    #[test]
    fn html_tokenize_test() {
        let html_tokens = tokenize_html("<html><!--comment--> <head> <SCRIPT>js;</SCRIPT> <title>\nhello world\n</title></head></html>");

        assert_eq!(html_tokens[0].value, "<html>");
        assert_eq!(html_tokens[1].value, "<head>");
        assert_eq!(html_tokens[2].value, "<title>");
        assert_eq!(html_tokens[3].value, "hello world");
        assert_eq!(html_tokens[4].value, "</title>");
        assert_eq!(html_tokens[5].value, "</head>");
        assert_eq!(html_tokens[6].value, "</html>");
    }

    #[test]
    fn html_tokenize_regression() {
        let s = "<b>Fatal error</b>:  Allowed memory size of 8388608 bytes exhausted (tried to allocate 32501 bytes) in <b>/home/path/x.php</b> on line <b>180</b><br />\0";
        let html_tokens = tokenize_html(s);
        assert_eq!(html_tokens[0].value, "<b>");
    }

    #[test]
    fn html_parse_test() {
        let html_tokens = tokenize_html("<html lang=\"en\"><head><TITLE>hello</TITLE></head><body></body></html>");
        let gs = "S : X; X : '<html>' '<head>' '<title>' 'hello' '</title>' '</head>' '<body>' '</body>' '</html>';";
        let cg = compile_grammar(gs);
        let pt = run("S", &cg, |s, i| { html_tokens[i].value == s }, 0);
        assert_eq!(pt.count(), 1);
    }

}
