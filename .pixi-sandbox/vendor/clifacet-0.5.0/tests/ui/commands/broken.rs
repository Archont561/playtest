//! The file the case in `commands_module_without_struct.rs` points at: a command module
//! that does not hold its struct. `broken.rs` has to define `Broken`, and this defines
//! `SomethingElse` instead — which is exactly the mistake the case is about.

pub struct SomethingElse;
