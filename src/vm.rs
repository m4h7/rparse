use std::collections::{HashMap};
use compiler::{CompiledGrammar, Opcode};

#[derive(Debug)]
pub enum ParseFragment {
    // final (top-level) root node
    Root { nonterm : String },
    CreateNode { parent : usize },
    Sibling { prev : usize, tokidx : usize, name : Option<String> },
    CloseNode { prev : usize, ev_name : Option<String> },
}

/**
 * Parse result value
 * contains all parse trees in a linked list in a flat array
 */
pub struct ParsedTrees {
    // fragments vector
    fragments : Vec<ParseFragment>,
    // indexes into 'fragments' that identify the end of an linked list
    tails : Vec<usize>,
}

impl ParsedTrees {

    pub fn new(
        fragments : Vec<ParseFragment>,
        tails : Vec<usize>) -> ParsedTrees {

        ParsedTrees {
            fragments : fragments,
            tails : tails,
        }
    }

    /**
     * Returns the number of successful parses
     */
    pub fn count(&self) -> usize {
        self.tails.len()
    }

    /**
     * Execute the callback on a parse tree
     *
     * Recursive function.
     *
     * fidx: fragment index
     * tokens: tokenized input that was parsed
     * returns: previous index into fragments
     */
    fn get_value<U>(&self, fidx : usize, tokens : &Vec<String>, cb : &Fn(&String) -> U) -> usize {
        let mut fragidx = fidx;
        let mut r = HashMap::<String, String>::new();
        loop {
            println!("fragidx = {} -> {:?}", fragidx, self.fragments[fragidx]);
            match &self.fragments[fragidx] {
                &ParseFragment::Root { ref nonterm } => {
//                    println!("root cb! nt {} r {:?}", nonterm, r);
                    // do not change 'fragidx'
                    // return value should be an index that points
                    // to ParseFragment::Root
                    cb(nonterm);
                    break;
                },
                &ParseFragment::CreateNode { parent } => {
                    // go up one level in the parse tree
//                    println!("CB! r = {:?}", r);
                    fragidx = parent;
                    // stop searching this list
                    break;
                },
                &ParseFragment::Sibling { prev, tokidx, ref name } => {
//                    println!("sibling {} {}", prev, tokens[tokidx]);
                    if name.is_some() {
                        r.insert(name.as_ref().unwrap().clone(), tokens[tokidx].clone());
                    }
                    fragidx = prev;
                },
                &ParseFragment::CloseNode { prev, ref ev_name } => {
                    println!("close node {:?} {}", ev_name, fragidx);
                    println!("... getting values for {:?}", ev_name);
                    let prev = self.get_value(prev, tokens, cb);
                    println!("::: got values");
                    let name = match *ev_name {
                        Some(ref x) => format!("{}", x),
                        None => format!("_{}", prev)
                    };
                    cb(&name);
                    r.insert(name, "<composite>".to_string());
                    fragidx = prev;
                },
            }
        }
        fragidx
    }

    /**
     * Execute a callback on a parse tree
     *
     * tidx: parse tree number (0 .. count())
     * tokens: input tokens
     * cb: callback function
     */
    pub fn execute<U>(&self, tidx : usize, tokens : &Vec<String>, cb : &Fn(&String) -> U) {
        self.get_value(self.tails[tidx], tokens, cb);
    }
}


#[derive(PartialEq, Debug)]
enum VMThreadState {
    ForkParent,
    MatchFailed,
    Runnable,
    Match,
    ParseFinished,
}

struct VMThread {
    // thread state
    state: VMThreadState,
    // return address stack
    ret : Vec<usize>,
    // instruction pointer
    ip : usize,
    // fragment index
    fragidx : usize,
}

//
// tokens: tokenized input string
// nt_start: nonterminal
// cg: grammar to use
//
pub fn run(tokens : &Vec<String>, nt_start : &str, cg : &CompiledGrammar, 
    match_fn: fn(&str, &str) -> bool
) -> ParsedTrees {

    let mut threads : Vec<VMThread> = Vec::new();
    let mut fragments = Vec::<ParseFragment>::new();

    // list of finished parses (index into fragments)
    let mut tails = Vec::new();

    // list of thread ids
    let mut runnable = Vec::new();
    // list of threads that need to perform a MATCH operation
    let mut matchable = Vec::new();

    for initial_thread_addr in cg.lookup_nonterm(nt_start) {
        fragments.push(ParseFragment::Root { nonterm : nt_start.to_string() });
        threads.push(VMThread {
            state: VMThreadState::Runnable,
            ret : Vec::new(),
            ip : initial_thread_addr,
            fragidx : fragments.len() - 1,
        });
        runnable.push(threads.len() - 1);
    }

    let mut tokidx = 0;

    while runnable.len() > 0  {

        while runnable.len() > 0 {

            let i = runnable.pop().unwrap();
            assert!(threads[i].state == VMThreadState::Runnable);

            println!("executing {:?}", cg.at(threads[i].ip));
            // fetch instruction at 'ip'
            match cg.at(threads[i].ip) {
                Opcode::Match { validx, nameidx } => {
                    // suspend thread at match
                    threads[i].state = VMThreadState::Match;
                    matchable.push(i);
                }
                Opcode::Fork { ntidx, nameidx } => {

                    // get nonterm name
                    let nt = &cg.strings[ntidx];
                    println!("{}: Opcode::Fork {}", threads[i].ip, nt);

                    for initial_thread_addr in cg.lookup_nonterm(nt) {

                        // ordering: [1] depends on [2]
                        fragments.push(ParseFragment::CreateNode {
                            parent: threads[i].fragidx // [2]
                        });

                        let mut vmt = VMThread {
                            state: VMThreadState::Runnable,
                            // copy stack from parent thread
                            ret : threads[i].ret.clone(),
                            ip : initial_thread_addr,
                            fragidx : fragments.len() - 1, // [1]
                        };

                        // push return value
                        vmt.ret.push(threads[i].ip);

                        threads.push(vmt);

                        // this new thread can run immediately
                        runnable.push(threads.len() - 1);
                    }

                    // parent thread (that forked) is stopped
                    threads[i].state = VMThreadState::ForkParent;
                }
                Opcode::Return { nameidx } => {
                    // nameidx is the prod name for callback
                    let ret = threads[i].ret.pop();
                    println!("{}: Opcode::Return {:?}",
                             threads[i].ip,
                             ret);
                    // check if the thread has a return value
                    // or whether it is a top-level thread
                    if ret.is_some() {
                        fragments.push(ParseFragment::CloseNode {
                            prev: threads[i].fragidx,
                            // remap int option to string option
                            ev_name: nameidx.map(|x| cg.strings[x].clone() ),
                        });
                        threads[i].ip = ret.unwrap() + 1;
                        threads[i].fragidx = fragments.len() - 1;
                        runnable.push(i);
                    } else {
                        threads[i].state = VMThreadState::ParseFinished;
                        // add the current fragidx to the list of finished parses
                        tails.push(threads[i].fragidx);
                    }
                }
            }
        }

        while matchable.len() > 0 {
            let j = matchable.pop().unwrap();
            assert!(threads[j].state  == VMThreadState::Match);

            match cg.at(threads[j].ip) {
                Opcode::Match { validx, nameidx } => {
                    if match_fn(&tokens[tokidx], &cg.strings[validx]) {
                        // allow this thread to proceed
                        threads[j].state = VMThreadState::Runnable;
                        threads[j].ip += 1;
                        runnable.push(j);

                        fragments.push(ParseFragment::Sibling {
                            prev: threads[j].fragidx,
                            tokidx: tokidx,
                            name: nameidx.map(|x| cg.strings[x].clone()),
                        });
                        threads[j].fragidx = fragments.len() - 1;
                    } else {
                        println!("{}: th {} match failed (tok '{}' == exp '{}')",
                                 threads[j].ip,
                                 j,
                                 tokens[tokidx],
                                 cg.strings[validx]);
                        threads[j].state = VMThreadState::MatchFailed;
                    }
                },
                _ => {}
            }
        }

        tokidx += 1;
    }

    ParsedTrees::new(fragments, tails)
}
