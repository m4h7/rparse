use std::collections::HashMap;
use std::fs::File;
use std::path::Path;
use std::io::prelude::*;
use grammar::{RuleId,load_grammar_str};

#[derive(Debug, Clone)]
pub enum Opcode {
    // Return: (from nonterminal)
    //   nameidx - production name
    Return { ntnameidx: usize, nameidx: Option<usize> },
    // Fork:
    //   ntidx - nonterminal name index
    //   nameidx - variable name
    Fork { ntidx: usize, nameidx : Option<usize> },
    // Match:
    //   validx - value to match
    //   nameidx - variable name
    Match { validx : usize, nameidx : Option<usize> },
}

pub struct CompiledGrammar {
    // nonterm str name -> addrs
    nt_names : HashMap<usize, Vec<usize>>,
    pub strings : Vec<String>,
    opcodes : Vec<Opcode>,
}

impl CompiledGrammar {

    pub fn new() -> CompiledGrammar {
        CompiledGrammar {
            strings : Vec::new(),
            opcodes : Vec::new(),
            nt_names : HashMap::new(),
        }
    }

    // return the opcode at given address
    pub fn at(&self, ip : usize) -> Opcode {
        self.opcodes[ip].clone()
    }

    pub fn debug_lookup(&self, idx: usize) -> String {
       self.strings[idx].clone()
    }

    // return a list of addresses associated with a nonterm name
    // TODO: remove the .clone()
    pub fn lookup_nonterm_idx(&self, ntidx: usize) -> Vec<usize> {
        match self.nt_names.get(&ntidx) {
            Some(v) => v.clone(),
            None => Vec::new()
        }
    }

    // print all opcodes in this grammar
    pub fn display(&self) {
        let mut ip = 0;
        for op in &self.opcodes {
            println!("__ {} = {:?}", ip, op);
            ip += 1;
        }
    }

    pub fn lookup_string(&self, s: &str) -> Option<usize> {
        self.strings.iter().position(|x| x == s)
    }

    fn add_string(&mut self, s : &str) -> usize {
        let idx = self.strings.iter().position(|x| x == s);
        match idx {
            Some(i) => i,
            None => {
                self.strings.push(s.to_string());
                self.strings.len() - 1
            }
        }
    }

    // add the current address to the list of addresses for the nonterminal 'nt_name'
    fn add_nonterm_prod(&mut self, nt_name : &str) {
        // convert nt_name to string index
        let nameidx = self.add_string(nt_name);

        // function to create the empty vector if no entry exists under 'nameidx'
        fn empty_vec() -> Vec<usize> {
            Vec::new()
        }
        let addrs = self.nt_names.entry(nameidx).or_insert_with(empty_vec);
        // address of the next production for nonterminal with name 'nt_name'
        let addr = self.opcodes.len();
        addrs.push(addr);
    }

    //
    // Generate RETURN instruction
    //
    // name: optional name for the production
    //
    fn op_return(&mut self, ntname: &String, name : Option<&String>) {
        let nameidx = name.map(|s| self.add_string(s));
        let ntnameidx = self.add_string(ntname);
        self.opcodes.push(Opcode::Return {
            ntnameidx: ntnameidx,
            nameidx : nameidx
        });
    }

    //
    // Generate FORK instruction
    //
    // nonterm_name - nonterm to call
    // var_name_opt - name for the variable to assign the result of the nonterm
    //
    fn op_fork(&mut self, nonterm_name : &str, var_name_opt : Option<&String>) {
        let fork_id = self.add_string(nonterm_name);
        let var_id = var_name_opt.map(|v| self.add_string(v));
        self.opcodes.push(Opcode::Fork {
            ntidx: fork_id,
            nameidx: var_id
        });
    }

    //
    // Generate MATCH instruction
    //
    // value - value to be matched
    // var_name_opt - name for the value
    //
    fn op_match(&mut self, value : &str, var_name_opt : Option<&String>) {
        let value_id = self.add_string(value);
        let var_name_id = var_name_opt.map(|v| { self.add_string(&v) });
        self.opcodes.push(Opcode::Match { validx: value_id, nameidx: var_name_id } );
    }

}

pub fn compile_grammar(gs : &str) -> CompiledGrammar {
    // compile string to a structured grammar
    let g = load_grammar_str(gs);
    let mut cg = CompiledGrammar::new();

    // compile nonterminals
    for nt in g.nonterminals() {
        // return all productions for this nonterm
        let prods = g.iter_over_nonterm(&nt);
        for prod in &prods {
            // remember the nonterminal start address
            // if first seen or add current address to the list
            cg.add_nonterm_prod(&nt);
            for com in &prod.components {
                // production component is either a terminal or a non-terminal
                match com.rule {
                    RuleId::Nonterminal(ref s) => {
                        // nonterminal -> fork instruction
                        cg.op_fork(s, com.name.as_ref());
                    }
                    RuleId::Terminal(ref s) => {
                        cg.op_match(s, com.name.as_ref());
                    }
                }
            }
            cg.op_return(&nt, prod.name.as_ref());
        }
    }

    cg
}

pub fn compile_grammar_file<S : Into<String>>(filename: S) -> CompiledGrammar
{
    let name = filename.into();
    let path = Path::new(name.as_str());
    let mut file = match File::open(&path) {
        Err(why) => panic!("couldn't open file: {}", why),
        Ok(file) => file
    };
    let mut s = String::new();
    match file.read_to_string(&mut s) {
        Err(why) => {
            panic!("Error reading file");
        },
        Ok(_) => compile_grammar(&s)
    }
}
