mod app;
mod capture;
mod input;
mod instance;
mod overlay;
mod view;

use instance::InstanceGuard;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let Some(_instance_guard) = InstanceGuard::acquire("wayzoomy")? else {
        println!("Wayzoomy is already running");
        return Ok(());
    };

    app::run()
}
