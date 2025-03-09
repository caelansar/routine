#![allow(unsafe_op_in_unsafe_fn)]
#![feature(naked_functions)]

use std::{
    arch::{asm, naked_asm},
    sync::OnceLock,
};

const DEFAULT_STACK_SIZE: usize = 1024 * 1024 * 2;
const MAX_ROUTINES: usize = 4;

static RUNTIME: OnceLock<RuntimePtr> = OnceLock::new();

#[derive(Debug)]
struct RuntimePtr(*mut Runtime);
unsafe impl Send for RuntimePtr {}
unsafe impl Sync for RuntimePtr {}

pub struct Runtime {
    routines: Vec<Routine>,
    current: usize,
}

/// State is an enum representing the states our routines can be in:
///
/// - Idle means the routine is idle and ready to be assigned a task if needed
/// - Running means the routine is running
/// - Ready means the routine is ready to move forward and resume execution
#[derive(PartialEq, Eq, Debug)]
enum State {
    Idle,
    Running,
    Ready,
}

/// Routine is a struct that represents a routine in our runtime.
struct Routine {
    // once a stack is allocated it must not move
    // we can avoid the reallocation by using a Box<[u8]>.
    stack: Box<[u8]>,
    ctx: RoutineContext,
    state: State,
}

/// RoutineContext holds data for the registers that the CPU needs to resume execution on a stack.
#[derive(Debug, Default)]
#[repr(C)]
struct RoutineContext {
    rsp: u64,
    r15: u64,
    r14: u64,
    r13: u64,
    r12: u64,
    rbx: u64,
    rbp: u64,
}

impl Default for Routine {
    fn default() -> Self {
        Routine {
            stack: Box::new([0_u8; DEFAULT_STACK_SIZE]),
            ctx: RoutineContext::default(),
            state: State::Idle,
        }
    }
}

impl Runtime {
    pub fn new() -> Self {
        let main_routine = Routine {
            stack: Box::new([0_u8; DEFAULT_STACK_SIZE]),
            ctx: RoutineContext::default(),
            state: State::Running,
        };
        let mut routines = vec![main_routine];
        let mut available_routines: Vec<Routine> =
            (1..MAX_ROUTINES).map(|_| Routine::default()).collect();
        routines.append(&mut available_routines);

        Runtime {
            routines,
            current: 0,
        }
    }

    pub fn init(&self) {
        let r_ptr: *mut Runtime = self as *const Runtime as *mut Runtime;
        RUNTIME
            .set(RuntimePtr(r_ptr))
            .expect("Runtime has already been initialized");
    }

    pub fn run(&mut self) -> ! {
        while self.park() {}
        std::process::exit(0);
    }

    fn routine_return(&mut self) {
        if self.current != 0 {
            self.routines[self.current].state = State::Idle;
            self.park();
        }
    }

    #[inline(never)]
    fn park(&mut self) -> bool {
        let mut pos = self.current;
        while self.routines[pos].state != State::Ready {
            pos += 1;
            if pos == self.routines.len() {
                pos = 0;
            }
            if pos == self.current {
                return false;
            }
        }
        if self.routines[self.current].state == State::Running {
            self.routines[self.current].state = State::Ready;
        }
        self.routines[pos].state = State::Running;

        let old_pos = self.current;
        self.current = pos;

        unsafe {
            let old: *mut RoutineContext = &mut self.routines[old_pos].ctx;
            let new: *const RoutineContext = &self.routines[pos].ctx;
            asm!(
            "call swap_context",
             in("rdi") old,
             in("rsi") new,
              clobber_abi("C")
            );
        }
        true
    }

    /// go spawns a new routine.
    ///
    // the stack layout is as follows:
    // High Address
    // +------------------+ <- stack_bottom (aligned to 16 bytes with & !15)
    // |                  |
    // |                  |
    // +------------------+ <- sp - 16
    // | guard function   |    First function to be called when routine ends
    // +------------------+ <- sp - 24
    // | skip function    |    Trampoline function that just does 'ret'
    // +------------------+ <- sp - 32
    // | f function       |    The actual routine function we want to run
    // +------------------+ <- sp (stored in ctx.rsp)
    // |                  |
    // |     Stack        |    Growing downward
    // |     Space        |
    // |                  |
    // +------------------+ <- stack start
    // Low Address
    pub fn go(&mut self, f: fn()) {
        let available = self
            .routines
            .iter_mut()
            .find(|t| t.state == State::Idle)
            .expect("no available routine.");
        let size = available.stack.len();
        unsafe {
            let stack_bottom = available.stack.as_mut_ptr().add(size);
            // aligned to 16 bytes with & !15
            let sp = (stack_bottom as usize & !15) as *mut u8;
            std::ptr::write(sp.offset(-16) as *mut u64, guard as u64);
            std::ptr::write(sp.offset(-24) as *mut u64, skip as u64);
            std::ptr::write(sp.offset(-32) as *mut u64, f as u64);
            available.ctx.rsp = sp.offset(-32) as u64;
        }
        available.state = State::Ready;
    }
}

fn guard() {
    let rt_ptr = RUNTIME.get().expect("Runtime not initialized");
    unsafe {
        (*rt_ptr.0).routine_return();
    }
}

pub fn go(f: fn()) {
    let rt_ptr = RUNTIME.get().expect("Runtime not initialized");
    unsafe {
        (*rt_ptr.0).go(f);
    }
}

#[naked]
#[unsafe(no_mangle)]
unsafe extern "C" fn skip() {
    naked_asm!("ret")
}

#[naked]
#[unsafe(no_mangle)]
unsafe extern "C" fn swap_context() {
    naked_asm!(
        "mov [rdi + 0x00], rsp",
        "mov [rdi + 0x08], r15",
        "mov [rdi + 0x10], r14",
        "mov [rdi + 0x18], r13",
        "mov [rdi + 0x20], r12",
        "mov [rdi + 0x28], rbx",
        "mov [rdi + 0x30], rbp",
        "mov rsp, [rsi + 0x00]",
        "mov r15, [rsi + 0x08]",
        "mov r14, [rsi + 0x10]",
        "mov r13, [rsi + 0x18]",
        "mov r12, [rsi + 0x20]",
        "mov rbx, [rsi + 0x28]",
        "mov rbp, [rsi + 0x30]",
        "ret"
    );
}

pub fn park() {
    let rt_ptr = RUNTIME.get().expect("Runtime not initialized");
    unsafe {
        (*rt_ptr.0).park();
    }
}

fn main() {
    let mut runtime = Runtime::new();
    runtime.init();

    go(|| {
        println!("Routine 1 STARTING");
        let id = 1;
        for i in 0..10 {
            println!("routine: {} counter: {}", id, i);
            park();
        }
        println!("Routine: {} FINISHED", id);
    });

    go(|| {
        println!("Routine 2 STARTING");
        let id = 2;
        for i in 0..15 {
            println!("routine: {} counter: {}", id, i);
            park();
        }
        println!("Routine: {} FINISHED", id);
    });
    runtime.run();
}
