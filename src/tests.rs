#[cfg(test)]
mod tests {

    extern crate core;

    use std::collections::VecDeque;
    use tokenize::{Tokenizer,Token};
    use compiler::compile_grammar;
    use htmltokenize::{tokenize_html,HTMLToken};
    use vm::run;

    use std::fs::File;
    use std::io::prelude::*;
    use std::path::Path;

    #[test]
    fn it_works() {
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

    pub struct ASTNode {
        x : i32
    }

    #[test]
    fn load_grammar_test() {

        fn match_eq(a : &str, b : &str) -> bool {
            a == b
        }

        let gs = "X : z z z `z` | a(q) b(w) c(e) `x`; Y : w(p) X(r) q(i) `y`;";
        let c = compile_grammar(gs);
        c.display();

        let mut tokens = Vec::<String>::new();
        tokens.push("w".to_string());
        tokens.push("a".to_string());
        tokens.push("b".to_string());
        tokens.push("c".to_string());
        tokens.push("q".to_string());

        let pt = run(&tokens, "Y", &c, match_eq);

        assert_eq!(pt.count(), 1);

        fn fx(v : &String) -> ASTNode {
            println!("fx called with {}", v);
            ASTNode { x : 0 }
        };

        pt.execute(0, &tokens, &fx );
    }

    #[test]
    fn rec_grammar_test() {
        fn match_eq(a : &str, b : &str) -> bool {
            println!("match_eq: {} v {}", a, b);
            a != b
        }
        let gs = "X : a X | b;";
        let c = compile_grammar(gs);
        c.display();

        let mut tokens = Vec::<String>::new();
        tokens.push("a".to_string());
        tokens.push("a".to_string());
        tokens.push("a".to_string());
        tokens.push("a".to_string());
        tokens.push("b".to_string());

        let pt = run(&tokens, "X", &c, match_eq);

        assert_eq!(pt.count(), 1);
    }

    #[test]
    fn html_token_test() {
        let t0 = HTMLToken::parse("<body class=\"no-js\">");
        assert_eq!(t0.value, "<body>");

        let t1 = HTMLToken::parse("<BR/>");
        assert_eq!(t1.value, "<br/>");

        let t2 = HTMLToken::parse("<A HREF=\"#\">");
        assert_eq!(t2.value, "<a>");

        let t3 = HTMLToken::parse("<a href=\"http://www.google.com\" target=\"_blank\">");
        assert_eq!(t3.value, "<a>");

        let t4 = HTMLToken::parse("<a href=\"http://www.bing.com/query?q=query\"/>");
        assert_eq!(t4.value, "<a>");
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
    fn html_tokenize_file_test() {
        let path = Path::new("/tmp/_aa8.html");
        let mut file = match File::open(&path) {
            Err(why) => panic!("couldn't open file: {}", why),
            Ok(file) => file
        };
        let mut s = String::new();
        match file.read_to_string(&mut s) {
            Err(why) => panic!("err read {}", why),
            Ok(_) => {
                let tokens = tokenize_html(&s);
                for t in tokens {
//                    println!("token {:?}", t);
                }
            }
        }
    }
}
