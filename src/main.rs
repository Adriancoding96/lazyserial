mod app;
mod ui;
mod serial;

use anyhow::Result;

fn main() -> Result<()> {
    app::run()
}
