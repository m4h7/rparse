use std::usize;
use compiler::{CompiledGrammar, Opcode};

struct SharedStackItem<U> {
    u: U,
    prev: usize,
}

struct SharedStack<U> {
    stack: Vec<SharedStackItem<U>>,
}

impl<U> SharedStack<U> {
    fn new() -> SharedStack<U> {
        SharedStack {
            stack: Vec::new(),
        }
    }

    fn len(&self) -> usize {
        self.stack.len()
    }

    // returns new sp
    fn push(&mut self, sp: usize, u: U) -> usize {
        let si = SharedStackItem {
            u: u,
            prev: sp,
        };
        self.stack.push(si);
        self.stack.len() - 1
    }

    fn top(&self, sp: usize) -> &U {
        let si = &self.stack[sp];
        &si.u
    }

    fn pop(&self, sp: usize) -> usize {
        let si = &self.stack[sp];
        si.prev
    }
}

//
// L. RuleNonTerm (prev=M)
//  M. RuleTermValue (prev=N)
//  N. RuleTermValue (prev=O)
//  O. RuleTermValue (prev=P)
//  P. RuleStart (parent=Q)
//  Q. ...

#[derive(Debug)]
pub enum ParseFragment {
    // RuleStart without parent is the top node
    RuleStart {
        parent : Option<usize>,
        name: Option<usize>,
        ntname: String,        // nonterm name
    },
    RuleTermValue {
        prev : usize,
        tokidx : usize,
        name: Option<usize>, // string index
    },
    RuleNonTerm {
        child : usize,
        ntname: String,
        ev_name : Option<usize>
    },
}

pub trait StreamingHandler {
    fn start(&mut self, ntname: &String, name: &Option<String>);
    fn end(&mut self, ntname: &String, xname: &Option<String>);
    fn term(&mut self, tokidx: usize, name: &Option<String>);
}

/**
 * Parse result value
 * contains all parse trees in a linked list in a flat array
 */
pub struct ParsedTrees {
    // fragments vector
    fragments : Vec<ParseFragment>,
    // indexes into 'fragments' that identify the end of an linked list
    tails : Vec<(usize, usize)>,
    // string table
    strings: Vec<String>,
}

impl ParsedTrees {

    pub fn new(
        fragments : Vec<ParseFragment>,
        tails : Vec<(usize, usize)>,
        strings: Vec<String>
    ) -> ParsedTrees {

        ParsedTrees {
            fragments : fragments,
            tails: tails,
            strings: strings,
        }
    }

    /**
     * Returns the number of successful parses
     */
    pub fn count(&self) -> usize {
        self.tails.len()
    }

    /**
     * Return the number of successul parses that
     * cover the tokens 0 to n
     */
    pub fn count_at_n(&self, n: usize) -> usize {
        self.tails
            .iter()
            .filter(|&x| x.1 >= n)
            .count()
    }

    /**
     * Execute the callback on a parse tree
     *
     * Recursive function.
     *
     * fidx: fragment index
     * returns: previous index into fragments
     */
    fn stream<U: StreamingHandler>(
        &self,
        indexes: &Vec<usize>,
        index: usize,
        handler: &mut U)  {

        let fragidx = indexes[index];
//        println!("stream@fragidx = {} -> {:?}", fragidx, self.fragments[fragidx]);
        match &self.fragments[fragidx] {
            // RuleStart
            // current node is the child of parent
            &ParseFragment::RuleStart { ref ntname, ref name, .. } => {
                let name_string = &name.map(|x| self.strings[x].clone());
                handler.start(&ntname, name_string);
                if index < indexes.len() - 1 {
                    self.stream(indexes, index + 1, handler);
                }
                if index == 0 {
                    handler.end(&ntname, name_string);
                }
            },
            &ParseFragment::RuleTermValue { tokidx, ref name, .. } => {
                handler.term(tokidx, &name.map(|x| self.strings[x].clone()));
                if index < indexes.len() - 1 {
                    // output sibling which comes before this term
                    self.stream(indexes, index + 1, handler);
                }
            },
            &ParseFragment::RuleNonTerm { ref ev_name, ref ntname, .. } => {
                handler.end(ntname, &ev_name.map(|x| self.strings[x].clone()));
                if index < indexes.len() - 1 {
                    // output sibling which comes before this term
                    self.stream(indexes, index + 1, handler);
                }
//                self.stream(child, handler);
            },
        }
    }

    fn prev(&self, fragidx: usize, default: usize) -> usize {
        match &self.fragments[fragidx] {
            &ParseFragment::RuleStart { parent, .. } => {
                match parent {
                  Some(parentidx) => parentidx,
                  None => default,
                }
             }
             &ParseFragment::RuleTermValue { prev, .. } => prev,
             &ParseFragment::RuleNonTerm { child, .. } => child,
        }
    }

    /**
     * Execute a callback on a parse tree
     *
     * tidx: parse tree number (0 .. self.count())
     * tokens: input tokens (vec of strings)
     * cb: callback function
     */
    pub fn execute<U: StreamingHandler>(
        &self,
        tidx : usize,
        handler : &mut U) {
        let mut tail = self.tails[tidx];
        let (fragidx, _) = tail;
        let mut curr = fragidx;
        let mut indexes = Vec::<usize>::new();
        while curr != usize::MAX {
            indexes.push(curr);
            curr = self.prev(curr, usize::MAX);
        }
        indexes.reverse();
        self.stream(&indexes, 0, handler);
    }
}

struct VMThread {
    // pointer into return address stack or usize::MAX
    sp: usize,
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
pub fn run<F>(nt_start : &str, cg : &CompiledGrammar, match_fn: F) -> ParsedTrees
    where F : Fn(&str, usize) -> bool {

    let mut fragments = Vec::<ParseFragment>::new();

    // list of finished parses (index into fragments)
    let mut tails : Vec<(usize, usize)> = Vec::new();

    // list of thread ids
    let mut runnable : Vec<VMThread> = Vec::new();
    // list of threads that need to perform a MATCH operation
    let mut matchable : Vec<VMThread> = Vec::new();

    for initial_thread_addr in cg.lookup_nonterm(nt_start) {
        fragments.push({
            ParseFragment::RuleStart {
                parent: None,
                ntname: nt_start.to_string(),
                name: None,
            }
        });
        runnable.push(VMThread {
            sp: usize::MAX,
            ip : initial_thread_addr,
            fragidx : fragments.len() - 1,
        });
    }

    let mut sharedStack = SharedStack::<usize>::new();
    let mut tokidx = 0;

    while runnable.len() > 0  {

        println!("runnable {} fragments {} stack {}",
                 runnable.len(), fragments.len(), sharedStack.len());

        while runnable.len() > 0 {

            let mut thread = runnable.pop().unwrap();

//            println!("** executing {}:{:?} (th {})", threads[i].ip, cg.at(threads[i].ip), i);
            // fetch instruction at 'ip'
            match cg.at(thread.ip) {
                Opcode::Match { validx, nameidx } => {
                    // suspend thread at match
                    matchable.push(thread);
                }
                Opcode::Fork { ntidx, nameidx } => {

                    // get nonterm name
                    let nt = &cg.strings[ntidx];
//                    println!("{}: Opcode::Fork {}", threads[i].ip, nt);

                    let mut created = 0;
                    for initial_thread_addr in cg.lookup_nonterm(nt) {

                        // ordering: [1] depends on [2]
                        fragments.push(ParseFragment::RuleStart {
                            parent: Some(thread.fragidx), // [2]
                            ntname: nt.to_string(),
                            name: nameidx,
                        });

                        let cur_fragidx = fragments.len() - 1;
                        let vmt = VMThread {
                            // copy stack from parent thread
                            sp: sharedStack.push(thread.sp, thread.ip),
                            ip : initial_thread_addr,
                            fragidx : cur_fragidx, // [1]
                        };

                        // this new thread can run immediately
                        runnable.push(vmt);
                        created += 1;
                    }
                    println!("created {}", created);
                }
                Opcode::Return { ntname, nameidx } => {
                    // nameidx is the prod name for callback
//                    let ret = threads[i].si.pop();
//                    println!("{}: Opcode::Return {:?}",
//                             threads[i].ip,
//                             ret);
                    // check if the thread has a return value
                    // or whether it is a top-level thread
                    if thread.sp != usize::MAX {
                        let ret = sharedStack.top(thread.sp);
                        let prev_fragidx = thread.fragidx;
                        fragments.push(ParseFragment::RuleNonTerm {
                            child: prev_fragidx,
                            ntname: ntname.clone(),
                            // remap int option to string option
                            ev_name: nameidx,
                        });

                        let cur_fragidx = fragments.len() - 1;

                        thread.sp = sharedStack.pop(thread.sp);
                        thread.ip = ret + 1;
                        thread.fragidx = cur_fragidx;
                        runnable.push(thread);
                    } else {
                        // add the current fragidx to the list of finished parses
                        let tail = (thread.fragidx, tokidx);
                        tails.push(tail);
                    }
                }
            }
        }
        assert_eq!(runnable.len(), 0);

        while matchable.len() > 0 {
            let mut thread = matchable.pop().unwrap();

            match cg.at(thread.ip) {
                Opcode::Match { validx, nameidx } => {
                    if match_fn(&cg.strings[validx], tokidx) {
                        // allow this thread to proceed
                        thread.ip += 1;
                        let prev_fragidx = thread.fragidx;
                        runnable.push(thread);

                        fragments.push(ParseFragment::RuleTermValue {
                            prev: prev_fragidx,
                            tokidx: tokidx,
                            name: nameidx,
                        });

                        let current_fragidx = fragments.len() - 1;
                        thread.fragidx = current_fragidx;
                    }
                },
                _ => {
                    panic!("matchable not at Match instruction");
                }
            }
        }
        assert_eq!(matchable.len(), 0);

        tokidx += 1;
    }

    ParsedTrees::new(fragments, tails, cg.strings.clone())
}
