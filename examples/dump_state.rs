//! Prints the current device state without changing anything.
//!
//! cargo run --example dump_state

use linuxblaster_control::BlasterXG6;

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .init();

    let blaster = BlasterXG6::init();
    for feature in &blaster.features {
        println!("{:<32} {}", feature.id.display_name(), feature.value());
    }
}
