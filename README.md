# Deadlocks Debugger
Rust is awesome, but the current `std::sync` API doesn't contain deadlock-safe functions to avoid deadlocks. This crate aims to provide an identical API for ease of switch, but instead of Rust's usual locks, you get anti-deadlock ones.

## Why should I use this crate?
It's rather easy to use, since the API is the same as Rust's `std::sync`, but you get self-debugging locks, hurray!

## Why should I keep using Rust's `std::sync` then?
Because the Law of Equivalent Exchange is very specific: by getting these awesome self-debugging features, you lose some performance: the locks I provide you WILL be slower, as they all report to the same manager in order to be debuggable.

As a rule of thumb, I suggest using `deadlocks_debugger` when debugging your application, before switching your `use deadlocks_debugger::...` to their `std::sync` equivalents. 

Remember that deadlocks are very much tied to timing issues which you might not encounter systematically, so please be thorough with your testing :)