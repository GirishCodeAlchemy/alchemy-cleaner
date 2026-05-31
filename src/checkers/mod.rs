/// Checker registry.
///
/// # Adding a new checker
/// 1. Create a new file `src/checkers/my_feature.rs`
/// 2. Implement the `Checker` trait for your struct
/// 3. Add `pub mod my_feature;` below
/// 4. Add `Box::new(my_feature::MyChecker)` to the `all_checkers()` vector
///
/// The runner, reporter, and scorer pick it up automatically — no other changes needed.

pub mod system_info;
pub mod cpu;
pub mod memory;
pub mod disk;
pub mod processes;
pub mod battery;
pub mod network;
pub mod startup;
pub mod logs;

use crate::checker::Checker;

/// Returns every registered checker in display order.
///
/// Add new checkers here to include them in the default run.
pub fn all_checkers() -> Vec<Box<dyn Checker>> {
    vec![
        Box::new(system_info::SystemInfoChecker),
        Box::new(cpu::CpuChecker),
        Box::new(memory::MemoryChecker),
        Box::new(disk::DiskChecker),
        Box::new(processes::ProcessChecker),
        Box::new(startup::StartupChecker),
        Box::new(battery::BatteryChecker),
        Box::new(network::NetworkChecker),
        Box::new(logs::LogsChecker),
    ]
}
