// xfail-fast

// Copyright 2012 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

extern mod std;
use libc, sys, cast;
use std::arena::Arena;

struct Bcx {
    fcx: &Fcx
}

struct Fcx {
    arena: &Arena,
    ccx: &Ccx
}

struct Ccx {
    x: int
}

fn h(bcx : &r/Bcx) -> &r/Bcx {
    return bcx.fcx.arena.alloc(|| Bcx { fcx: bcx.fcx });
}

fn g(fcx : &Fcx) {
    let bcx = Bcx { fcx: fcx };
    h(&bcx);
}

fn f(ccx : &Ccx) {
    let a = Arena();
    let fcx = &Fcx { arena: &a, ccx: ccx };
    return g(fcx);
}

pub fn main() {
    let ccx = Ccx { x: 0 };
    f(&ccx);
}

