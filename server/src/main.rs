// The contract layer (models, db, spans) is complete and tested ahead of the HTTP layer
// that will consume it. Until those call sites exist, every item reads as dead code.
// Suppressed here so that `cargo build` warnings stay meaningful for the modules being
// written now. Remove this once every module has a caller.
#![allow(dead_code)]

mod db;
mod models;
mod spans;

fn main() {
    println!("inkwell-server");
}
