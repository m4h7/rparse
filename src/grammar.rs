/**
 * Simple parser for a BNF-like grammar
 */

use std::fmt;
use std::collections::{HashMap,VecDeque};

use tokenize::{Tokenizer,Token};

#[derive(PartialEq)]
enum State {
    Nonterminal,      // -> FirstComponent
    FirstComponent,   // ':|;' -> Nonterminal|Components
    Components,       // -> Nonterminal
                      // ` -> EventName
                      // '(' - ComponentName
    ComponentName,    // str -> ComponentNameEnd
    ComponentNameEnd, // ')' -> Components
    EventName,        // String -> EventNameEnd
    EventNameEnd,     // ` -> ComponentsEnd
    ComponentsEnd,    // ; -> Nonterminal
}

type NontermId = usize;

#[derive(Debug, Clone)]
pub enum RuleId {
    Terminal(String),
    Nonterminal(String),
}

#[derive(Debug, Clone)]
pub struct Component {
    pub rule : RuleId,
    pub name : Option<String>,
}

impl Component {
    pub fn new(r : RuleId) -> Component {
        Component { rule : r, name : None }
    }
}

#[derive(Debug, Clone)]
pub struct Production {
    // name of production: x -> a b c `prodname`; name is 'prodname'
    pub name : Option<String>,
    // list of components for this production
    pub components : Vec<Component>,
}

impl Production {
    pub fn new() -> Production {
        Production { name : None, components : Vec::new() }
    }
}

impl fmt::Display for Production {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.name {
            Some(ref s) => {
                write!(f, "Production<{}>", s)
            }
            None => {
                write!(f, "Production<noname>")
            }
        }
    }
}

pub type ProductionId = usize;
type ProductionIDList = Vec<ProductionId>;

#[derive(Debug)]
pub struct Grammar {
    // id for generating production ids
    prod_seq_no : ProductionId,

    // nonterm name -> list of production IDs
    nonterm_prod_map : HashMap<String, ProductionIDList>,

    // nonterms: id -> production
    productions : HashMap<ProductionId, Production>,
}

impl Grammar {

    pub fn nonterminals(&self) -> Vec<String> {
        let mut v : Vec<String> = Vec::new();
        for k in self.nonterm_prod_map.keys() {
            v.push(k.clone());
        }
        v
    }

    pub fn iter_over_nonterm(&self, name : &str) -> Vec<Production> {
        let mut prods : Vec<Production> = Vec::new();
        let pids_opt = self.nonterm_prod_map.get(name);
        match pids_opt {
            Some(pids) => {
                for pid in pids {
                    match self.productions.get(pid) {
                        Some(p) => {
                            let cl : Production = p.clone();
                            prods.push(cl);
                        }
                        None => {}
                    }
                }
            }
            None => {}
        }
        prods
    }

    pub fn new() -> Grammar {
        Grammar {
            prod_seq_no : 0,
            nonterm_prod_map : HashMap::new(),
            productions : HashMap::new(),
        }
    }

    fn create_prodlist() -> ProductionIDList {
        ProductionIDList::new()
    }

    /*
     * Add a rule to the grammar structure
     *
     * nonterm_name - nonterminal
     * prod         - production
     */
    pub fn add_rule(
        &mut self,
        nonterm_name : &String,
        prod : Production,
    ) {
        let prod_id = self.prod_seq_no;
        self.prod_seq_no += 1;
        self.productions.insert(prod_id, prod);

        // create or update mapping
        //   nonterm_name -> productions list [..., prod_id]
        let prodlist = self.nonterm_prod_map.entry(nonterm_name.clone())
            .or_insert_with(Grammar::create_prodlist);
        prodlist.push(prod_id);
    }

    /*
     * If a component is a terminal and there is
     * a nonterm named as the value, convert the
     * component to a nonterm
     */
    fn resolve(&mut self) {
        for (_, prod) in self.productions.iter_mut() {
            for val in prod.components.iter_mut() {
                let repl = match val.rule {
                    RuleId::Terminal(ref s) => {
                        if self.nonterm_prod_map.contains_key(s) {
                            Some(RuleId::Nonterminal(s.clone()))
                        } else {
                            let mut start = 0;
                            let mut end = s.len();
                            for (i, c) in s.char_indices() {
                                if i == 0 && c == '\'' {
                                    start = i + 1;
                                }
                                if i == s.len() - 1 && c == '\'' {
                                    end = i;
                                }
                            }
                            let ns = s[start..end].to_string();
                            Some(RuleId::Terminal(ns))
                        }
                    }
                    _ => None
                };
                if repl.is_some() {
                    val.rule = repl.unwrap();
                }
            }
        }
    }
}

impl fmt::Display for Grammar {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Grammar<{} nonterms>", self.nonterm_prod_map.len())
    }
}

// load grammar from string and produce a grammar structure
pub fn load_grammar_str(input_str : &str) -> Grammar {
    let mut tokens : VecDeque<Token> = VecDeque::new();

    {
        let mut t = Tokenizer::new(|t| { tokens.push_back(t); });

        // push chars from s into the tokenizer
        for ch in input_str.chars() {
            t.push(ch);
        }

        // signal eos to the tokenizer
        t.finish();
    }

    let mut nonterminal : Option<String> = None;
    let mut production = Production::new();

    let mut grammar = Grammar::new();
    // initial state
    let mut state = State::Nonterminal;
    let mut failed = false;
    while !failed && !tokens.is_empty() {
        let s = tokens.pop_front().unwrap();
        let value = String::from(&input_str[s.beg.pos..s.end.pos]);
        match state {
            State::Nonterminal => {
                nonterminal = Some(value.clone());
                state = State::FirstComponent;
            },
            // expecting : then first component
            State::FirstComponent => {
                if value == ":" {
                    state = State::Components;
                } else if value == ";" {
                    // finished one nonterminal
                    // expect another nonterminal or eof
                    state == State::Nonterminal;
                } else {
                    // error: exp : or ;
                    println!("expected : or ;, not {}", value);
                    failed = true;
                }
            },
            State::Components => {
                if value == "`" {
                    state = State::EventName;
                } else if value == "(" {
                    state = State::ComponentName;
                } else if value == "|" {
                    let nonterm = nonterminal.as_ref().unwrap();
                    grammar.add_rule(
                        &nonterm,
                        production
                    );
                    production = Production::new();
                    // expect another production
                    state = State::Components;
                } else if value == ";" {
                    let nonterm = nonterminal.as_ref().unwrap();
                    grammar.add_rule(
                        &nonterm,
                        production
                    );
                    production = Production::new();
                    // expect another nonterminal or eos
                    state = State::Nonterminal;
                } else {
                    // save s to components for current nt
                    production.components.push(
                        Component::new(
                            RuleId::Terminal(
                                value.clone()))
                    );
                }
            },
            State::ComponentName => {
                let last_com = production.components.last_mut().unwrap();
                last_com.name = Some(value.clone());
                state = State::ComponentNameEnd;
            },
            State::ComponentNameEnd => {
                if value == ")" {
                    state = State::Components;
                } else {
                    // report error
                    println!("expecting ')' to end component name");
                    failed = true;
                }
            },
            State::EventName => {
                production.name = Some(value.clone());
                // save s
                state = State::EventNameEnd;
            },
            State::ComponentsEnd => {
                if value == ";" {
                    let nonterm = nonterminal.as_ref().unwrap();
                    grammar.add_rule(
                        &nonterm,
                        production
                    );
                    production = Production::new();
                    // expect another nonterminal or eos
                    state = State::Nonterminal;
                } else if value == "|" {
                    let nonterm = nonterminal.as_ref().unwrap();
                    grammar.add_rule(
                        &nonterm,
                        production
                    );
                    production = Production::new();
                    state = State::Components;
                } else {
                    println!("exp ';' not {}", value);
                    failed = true;
                }
            },
            State::EventNameEnd => {
                if value == "`" {
                    state = State::ComponentsEnd;
                } else {
                    // error: exp ` to end the event name
                }
            },
        }
    }
    grammar.resolve();
    grammar
}
