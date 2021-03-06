// Copyright 2013 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

/*!
Weak tasks

Weak tasks are a runtime feature for building global services that
do not keep the runtime alive. Normally the runtime exits when all
tasks exits, but if a task is weak then the runtime may exit while
it is running, sending a notification to the task that the runtime
is trying to shut down.
*/

use option::{Some, None, swap_unwrap};
use private::at_exit::at_exit;
use private::global::global_data_clone_create;
use private::finally::Finally;
use pipes::{Port, Chan, SharedChan, GenericChan, GenericPort,
            GenericSmartChan, stream};
use task::{Task, task, spawn};
use task::rt::{task_id, get_task_id};
use hashmap::linear::LinearMap;
use ops::Drop;

type ShutdownMsg = ();

// FIXME #4729: This could be a PortOne but I've experienced bugginess
// with oneshot pipes and try_send
pub unsafe fn weaken_task(f: &fn(Port<ShutdownMsg>)) {
    let service = global_data_clone_create(global_data_key,
                                           create_global_service);
    let (shutdown_port, shutdown_chan) = stream::<ShutdownMsg>();
    let shutdown_port = ~mut Some(shutdown_port);
    let task = get_task_id();
    // Expect the weak task service to be alive
    assert service.try_send(RegisterWeakTask(task, shutdown_chan));
    unsafe { rust_dec_kernel_live_count(); }
    do fn&() {
        let shutdown_port = swap_unwrap(&mut *shutdown_port);
        f(shutdown_port)
    }.finally || {
        unsafe { rust_inc_kernel_live_count(); }
        // Service my have already exited
        service.send(UnregisterWeakTask(task));
    }
}

type WeakTaskService = SharedChan<ServiceMsg>;
type TaskHandle = task_id;

fn global_data_key(_v: WeakTaskService) { }

enum ServiceMsg {
    RegisterWeakTask(TaskHandle, Chan<ShutdownMsg>),
    UnregisterWeakTask(TaskHandle),
    Shutdown
}

fn create_global_service() -> ~WeakTaskService {

    debug!("creating global weak task service");
    let (port, chan) = stream::<ServiceMsg>();
    let port = ~mut Some(port);
    let chan = SharedChan(chan);
    let chan_clone = chan.clone();

    do task().unlinked().spawn {
        debug!("running global weak task service");
        let port = swap_unwrap(&mut *port);
        let port = ~mut Some(port);
        do fn&() {
            let port = swap_unwrap(&mut *port);
            // The weak task service is itself a weak task
            debug!("weakening the weak service task");
            unsafe { rust_dec_kernel_live_count(); }
            run_weak_task_service(port);
        }.finally {
            debug!("unweakening the weak service task");
            unsafe { rust_inc_kernel_live_count(); }
        }
    }

    do at_exit {
        debug!("shutting down weak task service");
        chan.send(Shutdown);
    }

    return ~chan_clone;
}

fn run_weak_task_service(port: Port<ServiceMsg>) {

    let mut shutdown_map = LinearMap::new();

    loop {
        match port.recv() {
            RegisterWeakTask(task, shutdown_chan) => {
                let previously_unregistered =
                    shutdown_map.insert(task, shutdown_chan);
                assert previously_unregistered;
            }
            UnregisterWeakTask(task) => {
                match shutdown_map.pop(&task) {
                    Some(shutdown_chan) => {
                        // Oneshot pipes must send, even though
                        // nobody will receive this
                        shutdown_chan.send(());
                    }
                    None => fail!()
                }
            }
            Shutdown => break
        }
    }

    do shutdown_map.consume |_, shutdown_chan| {
        // Weak task may have already exited
        shutdown_chan.send(());
    }
}

extern {
    unsafe fn rust_inc_kernel_live_count();
    unsafe fn rust_dec_kernel_live_count();
}

#[test]
fn test_simple() {
    let (port, chan) = stream();
    do spawn {
        unsafe {
            do weaken_task |_signal| {
            }
        }
        chan.send(());
    }
    port.recv();
}

#[test]
fn test_weak_weak() {
    let (port, chan) = stream();
    do spawn {
        unsafe {
            do weaken_task |_signal| {
            }
            do weaken_task |_signal| {
            }
        }
        chan.send(());
    }
    port.recv();
}

#[test]
fn test_wait_for_signal() {
    do spawn {
        unsafe {
            do weaken_task |signal| {
                signal.recv();
            }
        }
    }
}

#[test]
fn test_wait_for_signal_many() {
    use uint;
    for uint::range(0, 100) |_| {
        do spawn {
            unsafe {
                do weaken_task |signal| {
                    signal.recv();
                }
            }
        }
    }
}

#[test]
fn test_select_stream_and_oneshot() {
    use pipes::select2i;
    use either::{Left, Right};

    let (port, chan) = stream();
    let (waitport, waitchan) = stream();
    do spawn {
        unsafe {
            do weaken_task |signal| {
                match select2i(&port, &signal) {
                    Left(*) => (),
                    Right(*) => fail!()
                }
            }
        }
        waitchan.send(());
    }
    chan.send(());
    waitport.recv();
}

