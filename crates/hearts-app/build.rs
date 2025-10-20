#[cfg(windows)]
extern crate embed_resource;

use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let now = time::OffsetDateTime::now_utc();
    let format = time::format_description::parse(
        "[year]-[month repr:numerical padding:zero]-[day padding:zero] [hour padding:zero]:[minute padding:zero]:[second padding:zero] UTC",
    )?;
    let stamp = now.format(&format)?;
    println!("cargo:rustc-env=MDH_BUILD_DATE={}", stamp);
    println!("cargo:rerun-if-changed=build.rs");

    #[cfg(windows)]
    {
        println!("cargo:rerun-if-changed=res/app.rc");
        println!("cargo:rerun-if-changed=res/app.manifest");
        println!("cargo:rerun-if-changed=res/app.ico");
        embed_resource::compile("res/app.rc", embed_resource::NONE);
    }

    Ok(())
}
