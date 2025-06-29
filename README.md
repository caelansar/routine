# Rust Coroutine Implementation

A lightweight coroutine implementation in Rust that demonstrates cooperative multitasking using a custom runtime system.

## How it works

```
Current Routine A      swap_context()           New Routine B
    |                       |                         |
    |                       |                         |
park() --------------> save registers ----------------|
                        (rsp,r15...)                  |
                            |                         |
                    load new registers -------------->|
                            |                         |
                           ret ---------------> execute function f
                                                      |
                                                (after completion)
                                                      |
                                                   skip()
                                                      |
                                                   guard()
```

## Features

- Custom runtime system for managing coroutines
- Stack-based context switching using assembly
- Support for up to 4 concurrent coroutines
- 2MB stack size per coroutine
- Cooperative scheduling with explicit `park()` points

## Implementation Details

The implementation consists of several key components:

### Runtime

The `Runtime` struct manages the coroutine scheduling and execution. It maintains:
- A vector of threads (coroutines)
- Current executing thread index
- Thread state management

### Thread States

Threads can be in one of three states:
- `Idle`: Available for new tasks
- `Running`: Currently executing
- `Ready`: Prepared to execute but waiting for scheduler

### Context Switching

Context switching is implemented using naked functions and inline assembly to save and restore register states:
- Preserves essential registers (rsp, r15, r14, r13, r12, rbx, rbp)
- Implements zero-cost context switches
- Uses a thread-safe global runtime instance

## Usage Example

```rust
fn main() {
    let mut runtime = Runtime::new();
    runtime.init();

    // Create first coroutine
    go(|| {
        println!("Routine 1 STARTING");
        let id = 1;
        for i in 0..10 {
            println!("routine: {} counter: {}", id, i);
            park();
        }
        println!("Routine: {} FINISHED", id);
    });

    // Create second coroutine
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
```

## Requirements

- x86_64 architecture *only*

## Safety Notes

This implementation uses unsafe Rust features:
- Naked functions
- Inline assembly
- Raw pointers manipulation

These are necessary for low-level control over the execution context but should be used with caution.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
