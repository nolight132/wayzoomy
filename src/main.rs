mod app;
mod canvas;
mod capture;
mod input;
mod instance;
mod overlay;
mod view;

use instance::InstanceGuard;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let Some(_instance_guard) = InstanceGuard::acquire("wayzoomy")? else {
        println!("Toggling Wayzoomy off");
        return Ok(());
    };

    app::run()
}
