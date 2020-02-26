# `no_deadlocks`: a Runtime Deadlock Debugger
Rust is awesome, but the current `std::sync` API doesn't contain deadlock-safe functions to avoid deadlocks. This crate aims to provide an identical API for ease of switch, but instead of Rust's usual locks, you get anti-deadlock ones.

By default, debug information is writen to `stderr` when a deadlock is found. If you want `no_deadlock` reports to be written to a specific file, you can specify its path in the `NO_DEADLOCKS` environment variable.

## Why should I use this crate?
It's rather easy to use, since the API is the same as Rust's `std::sync`, but you get self-debugging locks, hurray!  
You may use it preventively while creating your program, or you may replace your locks with these whenever you suspect a deadlock has happened and you want to inquire.

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

## Why do you use `vector-map` by default?
Because in most programs, there are actually rather few locks. `vector-map`'s `VecMap` was built as a vector of tuples equivalent to `std::collections::HashMap`, which is more efficient for small collections.

The `use_vecmap` feature (on by default) switches between `VecMap` and `HashMap`. If your program uses many locks (about a hundred), feel free to toggle it off.

## What's next for this crate?
I'm satisfied with this crate's current state (read: "I don't have a plan"), but feel free to write up an issue to let me know what you'd like :)