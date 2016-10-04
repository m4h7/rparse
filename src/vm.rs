use std::usize;
use std::env;
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
        ntname: usize, // nonterm name
    },
    RuleTermValue {
        prev : usize,
        tokidx : usize,
        name: Option<usize>, // string index
    },
    RuleNonTerm {
        child : usize,
        ntnameidx: usize,
        ev_name : Option<usize>
    },
}

pub trait StreamingHandler {
    fn start(&mut self, ntname: &String, name: &Option<&String>);
    fn end(&mut self, ntname: &String, xname: &Option<&String>);
    fn term(&mut self, tokidx: usize, name: &Option<&String>);
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
            &ParseFragment::RuleStart { ntname, name, .. } => {
                let name_string = name.map(|x| &self.strings[x]);
                let ntname_string = &self.strings[ntname];
                handler.start(ntname_string, &name_string);
                if index < indexes.len() - 1 {
                    self.stream(indexes, index + 1, handler);
                }
                if index == 0 {
                    handler.end(ntname_string, &name_string);
                }
            },
            &ParseFragment::RuleTermValue { tokidx, name, .. } => {
                let name_string = name.map(|x| &self.strings[x]);
                handler.term(tokidx, &name_string);
                if index < indexes.len() - 1 {
                    // output sibling which comes before this term
                    self.stream(indexes, index + 1, handler);
                }
            },
            &ParseFragment::RuleNonTerm { ev_name, ntnameidx, .. } => {
                let ntname_string = &self.strings[ntnameidx];
                let evname = ev_name.map(|x| &self.strings[x]);
                handler.end(ntname_string, &evname);
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
        let tail = self.tails[tidx];
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

    let debug_level = match env::var("PARSERDEBUG") {
        Ok(s) => {
            match s.parse::<usize>() {
                Ok(n) => n,
                Err(why) => {
                    println!("Unable to parse $PARSEDEBUG as an uint {:?}", why);
                    0
                }
            }
        },
        Err(_) => 0,
    };

    // allocate enough space to store all possible
    // matches within one token
    let mut matched = Vec::<isize>::with_capacity(cg.strings.len());
    for n in 0..cg.strings.len() {
        matched.push(0);
    }

    let mut fragments = Vec::<ParseFragment>::new();

    // list of finished parses (index into fragments)
    let mut tails : Vec<(usize, usize)> = Vec::new();

    // list of thread ids
    let mut runnable : Vec<VMThread> = Vec::new();

    // list of threads that need to perform a MATCH operation
    // sorted by first
    let mut matchable : Vec<(usize, VMThread)> = Vec::new();

    let nt_start_idx: Option<usize> = cg.lookup_string(nt_start);

    for initial_thread_addr in cg.lookup_nonterm_idx(nt_start_idx.unwrap()) {
        fragments.push({
            ParseFragment::RuleStart {
                parent: None,
                ntname: nt_start_idx.unwrap(),
                name: None,
            }
        });
        runnable.push(VMThread {
            sp: usize::MAX,
            ip: initial_thread_addr,
            fragidx: fragments.len() - 1,
        });
    }

    let mut sharedStack = SharedStack::<usize>::new();
    let mut tokidx = 0;

    while runnable.len() > 0  {
        if debug_level > 2 {
            println!("at tokidx {} running {} threads",
                     tokidx, runnable.len());
        }
        while runnable.len() > 0 {
            let mut thread = runnable.pop().unwrap();
            if debug_level > 3 {
                match cg.at(thread.ip) {
                    Opcode::Match { validx, .. } => {
                        println!("** {} Match '{}' (runnable {} matchable {})",
                                 thread.ip,
                                 cg.debug_lookup(validx),
                                 runnable.len(),
                                 matchable.len());
                    }
                    Opcode::Fork { ntidx, nameidx } => {
                        println!("** {} Fork '{}/{}' (runnable {} matchable {})",
                                 thread.ip,
                                 cg.debug_lookup(ntidx),
                                 nameidx.map_or("".to_string(),
                                                |x| cg.debug_lookup(x)),
                                 runnable.len(),
                                 matchable.len());
                    }
                    Opcode::Return { ntnameidx, nameidx } => {
                        println!("** {} Return '{}/{}' (runnable {} matchable {})",
                                 thread.ip,
                                 cg.debug_lookup(ntnameidx),
                                 nameidx.map_or("".to_string(),
                                                |x| cg.debug_lookup(x)),
                                 runnable.len(),
                                 matchable.len());
                    }
                }
            }
            // fetch instruction at 'ip'
            match cg.at(thread.ip) {
                Opcode::Match { validx, .. } => {
                    // maintain a sorted order in matchable
                    // on the first item of the tuple (validx)
                    match matchable.binary_search_by_key(&validx, |&(a, ref b)| a) {
                        Ok(pos) => matchable.insert(pos, (validx, thread)),
                        Err(pos) => matchable.insert(pos, (validx, thread))
                    }
                }
                Opcode::Fork { ntidx, nameidx } => {
                    // ordering: [1] depends on [2]
                    fragments.push(ParseFragment::RuleStart {
                        parent: Some(thread.fragidx), // [2]
                        ntname: ntidx,
                        name: nameidx,
                    });

                    for initial_thread_addr in cg.lookup_nonterm_idx(ntidx) {
                        if debug_level > 4 {
                            println!("forking '{}' -> addr {} fragidx {}",
                                     cg.debug_lookup(ntidx),
                                     initial_thread_addr,
                                     fragments.len() - 1);
                        }
                        let vmt = VMThread {
                            // continue stack from parent thread
                            sp: sharedStack.push(thread.sp, thread.ip),
                            ip: initial_thread_addr,
                            fragidx: fragments.len() - 1, // [1]
                        };
                        // this new thread can run immediately
                        runnable.push(vmt);
                    }
                }
                Opcode::Return { ntnameidx, nameidx } => {
                    // check if the thread has a return value
                    // or whether it is a top-level thread
                    if thread.sp != usize::MAX {
                        let ret = sharedStack.top(thread.sp);
                        let prev_fragidx = thread.fragidx;
                        fragments.push(ParseFragment::RuleNonTerm {
                            child: prev_fragidx,
                            ntnameidx: ntnameidx,
                            // remap int option to string option
                            ev_name: nameidx,
                        });

                        thread.sp = sharedStack.pop(thread.sp);
                        thread.ip = ret + 1;
                        thread.fragidx = fragments.len() - 1;
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

        matchable.reverse();
        for n in 0..cg.strings.len() {
            matched[n] = 0;
        }
        if debug_level > 1 && matchable.len() > 0 {
            println!("matching {} threads at token index {}",
                     matchable.len(), tokidx);
        }
        let mut prev_validx = usize::MAX;
        while matchable.len() > 0 {
            let tuple = matchable.pop().unwrap();
            // check that the matchable array is sorted
            assert!(prev_validx == usize::MAX ||
                    prev_validx <= tuple.0);
            prev_validx = tuple.0;
            let mut thread = tuple.1;

            match cg.at(thread.ip) {
                Opcode::Match { validx, nameidx } => {
                    let match_result;
                    // reuse previous match result if there is one
                    if matched[validx] == 1 {
                        match_result = true;
                    } else if matched[validx] == -1 {
                        match_result = false;
                    } else {
                        match_result = match_fn(&cg.strings[validx], tokidx);
                        if match_result {
                            matched[validx] = 1;
                        } else {
                            matched[validx] = -1;
                        }
                    }

                    if match_result {
                        // allow this thread to proceed
                        thread.ip += 1;
                        let prev_fragidx = thread.fragidx;
                        runnable.push(thread);

                        fragments.push(ParseFragment::RuleTermValue {
                            prev: prev_fragidx,
                            tokidx: tokidx,
                            name: nameidx,
                        });

                        thread.fragidx = fragments.len() - 1;
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
