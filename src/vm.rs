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
        name: Option<String>,
        ntname: String,        // nonterm name
    },
    RuleTermValue { prev : usize, tokidx : usize, name : Option<String> },
    RuleNonTerm {
      child : usize,
      ntname: String,
      ev_name : Option<String>
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
}

impl ParsedTrees {

    pub fn new(
        fragments : Vec<ParseFragment>,
        tails : Vec<(usize, usize)>
    ) -> ParsedTrees {

        ParsedTrees {
            fragments : fragments,
            tails: tails,
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
                handler.start(&ntname, &name);
                if index < indexes.len() - 1 {
                    self.stream(indexes, index + 1, handler);
                }
                if index == 0 {
                    handler.end(&ntname, &name);
                }
            },
            &ParseFragment::RuleTermValue { tokidx, ref name, .. } => {
                handler.term(tokidx, name);
                if index < indexes.len() - 1 {
                    // output sibling which comes before this term
                    self.stream(indexes, index + 1, handler);
                }
            },
            &ParseFragment::RuleNonTerm { ref ev_name, ref ntname, .. } => {
                handler.end(ntname, ev_name);
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

    let mut threads : Vec<VMThread> = Vec::new();
    let mut fragments = Vec::<ParseFragment>::new();

    // list of finished parses (index into fragments)
    let mut tails : Vec<(usize, usize)> = Vec::new();

    // list of thread ids
    let mut runnable = Vec::new();
    // list of threads that need to perform a MATCH operation
    let mut matchable = Vec::new();

    for initial_thread_addr in cg.lookup_nonterm(nt_start) {
        fragments.push({
            ParseFragment::RuleStart {
                parent: None,
                ntname: nt_start.to_string(),
                name: None,
            }
        });
        threads.push(VMThread {
            state: VMThreadState::Runnable,
            sp: usize::MAX,
            ip : initial_thread_addr,
            fragidx : fragments.len() - 1,
        });
        runnable.push(threads.len() - 1);
    }

    let mut sharedStack = SharedStack::<usize>::new();
    let mut tokidx = 0;

    while runnable.len() > 0  {

        println!("threads {} fragments {} stack {}",
                 threads.len(), fragments.len(), sharedStack.len());

        while runnable.len() > 0 {

            let i = runnable.pop().unwrap();
            assert!(threads[i].state == VMThreadState::Runnable);

//            println!("** executing {}:{:?} (th {})", threads[i].ip, cg.at(threads[i].ip), i);
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
//                    println!("{}: Opcode::Fork {}", threads[i].ip, nt);

                    for initial_thread_addr in cg.lookup_nonterm(nt) {

                        // ordering: [1] depends on [2]
                        fragments.push(ParseFragment::RuleStart {
                            parent: Some(threads[i].fragidx), // [2]
                            ntname: nt.to_string(),
                            name: nameidx.map(
                                    |x| cg.strings[x].clone()),
                        });

                        let cur_fragidx = fragments.len() - 1;
                        let vmt = VMThread {
                            state: VMThreadState::Runnable,
                            // copy stack from parent thread
                            sp: sharedStack.push(threads[i].sp, threads[i].ip),
                            ip : initial_thread_addr,
                            fragidx : cur_fragidx, // [1]
                        };

                        // add new thread
                        threads.push(vmt);

                        // this new thread can run immediately
                        runnable.push(threads.len() - 1);
                    }

                    // parent thread (that forked) is stopped
                    threads[i].state = VMThreadState::ForkParent;
                }
                Opcode::Return { ntname, nameidx } => {
                    // nameidx is the prod name for callback
//                    let ret = threads[i].si.pop();
//                    println!("{}: Opcode::Return {:?}",
//                             threads[i].ip,
//                             ret);
                    // check if the thread has a return value
                    // or whether it is a top-level thread
                    if threads[i].sp != usize::MAX {
                        let ret = sharedStack.top(threads[i].sp);
                        let prev_fragidx = threads[i].fragidx;
                        fragments.push(ParseFragment::RuleNonTerm {
                            child: prev_fragidx,
                            ntname: ntname.clone(),
                            // remap int option to string option
                            ev_name: nameidx.map(|x| cg.strings[x].clone() ),
                        });

                        let cur_fragidx = fragments.len() - 1;

                        threads[i].sp = sharedStack.pop(threads[i].sp);
                        threads[i].ip = ret + 1;
                        threads[i].fragidx = cur_fragidx;
                        runnable.push(i);
                    } else {
                        threads[i].state = VMThreadState::ParseFinished;
                        // add the current fragidx to the list of finished parses
                        let tail = (threads[i].fragidx, tokidx);
                        tails.push(tail);
                    }
                }
            }
        }

        while matchable.len() > 0 {
            let j = matchable.pop().unwrap();
            assert!(threads[j].state  == VMThreadState::Match);

            match cg.at(threads[j].ip) {
                Opcode::Match { validx, nameidx } => {
                    if match_fn(&cg.strings[validx], tokidx) {
                        // allow this thread to proceed
                        threads[j].state = VMThreadState::Runnable;
                        threads[j].ip += 1;
                        runnable.push(j);

                        let prev_fragidx = threads[j].fragidx;
                        fragments.push(ParseFragment::RuleTermValue {
                            prev: prev_fragidx,
                            tokidx: tokidx,
                            name: nameidx.map(|x| cg.strings[x].clone()),
                        });

                        let current_fragidx = fragments.len() - 1;
                        threads[j].fragidx = current_fragidx;
                    } else {
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
