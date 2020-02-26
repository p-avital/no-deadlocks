# `no_deadlocks`: a Runtime Deadlock Debugger
Rust is awesome, but the current `std::sync` API doesn't contain deadlock-safe functions to avoid deadlocks. This crate aims to provide an identical API for ease of switch, but instead of Rust's usual locks, you get anti-deadlock ones.

## Why should I use this crate?
It's rather easy to use, since the API is the same as Rust's `std::sync`, but you get self-debugging locks, hurray! You may use it preventively while creating your program, or you may replace your locks with these whenever you suspect a deadlock has happened and you want to inquire.

## Why should I keep using Rust's `std::sync` then?
Because the Law of Equivalent Exchange is very specific: by getting these awesome self-debugging features, you lose some performance: the locks I provide you WILL be slower (much slower, in fact), as they all report to the same manager in order to be debuggable.

As a rule of thumb, I suggest using `no_deadlocks` when debugging your application, before switching your `use no_deadlocks::...` to their `std::sync` equivalents. 

Remember that deadlocks are very much tied to timing issues which you might not encounter systematically, so please be thorough with your testing :)

## How does it work?
All locks from `no_deadlocks` are actually handles to elements of a global `LockManager`. Their state is stored in a single set, which is locked and mutated any time your locks are locked or unlocked.

When a lock is taken, an unresolved trace is saved in case debugging is needed, and stored together with the thread's id. These informations are dropped upon unlocking.  
When a lock is inaccessible, the request will be stored with an unresolved trace, and an analysis will be run.

This analysis builds a graph where locks point toward threads that currently own them, and threads point toward locks they have requested.  
A simple backtracking algorithm is used to search for loops within the graph.  
If a loop is detected, that means a deadlock exists: the relevant traces are then resolved to help you figure out why the deadlock happened.

## What about reentrance?
While this crate could handle reentrance, `std::sync`'s locks don't. Reentrance is actually the simplest deadlock you can find when working with locks, and can (should) usually be avoided. It is however an easy enough mistake to make, especially when working with recursion.

`no_deadlock` detects reentrance deadlocks the same way it does for all other deadlocks, but will log it slightly differently since it's easily distinguishable (a reentrance deadlock is modeled by a 2 node cycle, whereas any other deadlock would require more nodes to be modeled).

## What's next for this crate?
For now, I want to find the nicest way I can to relay the Deadlock Error to the user. I opted for `panic` because deadlocks are usually non-recoverable states anyway, and mostly because I wanted to keep `std::sync`'s signatures as much as possible, but I'm open to suggestions.

I also need to get the debugging experience to be nice. As of writing this README, the infrastructure to give great traces on the deadlocks is here, but the formatting is very lacking.

Currently, all of the information is immedately written to `stderr` upon finding a deadlock. Rather soon, I'm thinking of adding an environment variable to allow you to set a file for all of this information to be dumped into.

As for future progress, feel free to write up an issue to let me know what you'd like :)