mod capture;
mod instance;

use instance::InstanceGuard;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let Some(_instance_guard) = InstanceGuard::acquire("wayzoomy")? else {
        println!("Wayzoomy is already running");
        return Ok(());
    };

    println!("Wayzoomy started");

    capture::run()
}
