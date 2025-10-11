#[cfg(windows)]
extern crate embed_resource;

use sha2::{Digest, Sha256};
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let now = time::OffsetDateTime::now_utc();
    let format = time::format_description::parse(
        "[year]-[month repr:numerical padding:zero]-[day padding:zero] [hour padding:zero]:[minute padding:zero]:[second padding:zero] UTC",
    )?;
    let stamp = now.format(&format)?;
    println!("cargo:rustc-env=MDH_BUILD_DATE={}", stamp);
    println!("cargo:rerun-if-changed=build.rs");

    // Compute observation schema hash
    compute_schema_hash();

    #[cfg(windows)]
    {
        println!("cargo:rerun-if-changed=res/app.rc");
        println!("cargo:rerun-if-changed=res/app.manifest");
        println!("cargo:rerun-if-changed=res/app.ico");
        embed_resource::compile("res/app.rc", embed_resource::NONE);
    }

    Ok(())
}

fn compute_schema_hash() {
    // Define the observation schema structure as a stable string
    // This must match the actual Observation struct in rl/observation.rs
    let schema_desc = concat!(
        "v1.1.0:",
        "hand_onehot[52],",
        "seen_onehot[52],",
        "trick_led_suit[4],",
        "trick_cards[4][17],",
        "trick_count,",
        "my_trick_position,",
        "trick_pad,",
        "scores_relative[4],",
        "hearts_broken,",
        "tricks_completed,",
        "passing_phase,",
        "passing_direction[4],",
        "opp_voids[12],",
        "last_4_cards[68]"
    );

    let mut hasher = Sha256::new();
    hasher.update(schema_desc.as_bytes());
    let result = hasher.finalize();
    let hash = format!("{:x}", result);

    println!("cargo:rustc-env=SCHEMA_HASH={}", hash);
    println!("cargo:rustc-env=SCHEMA_VERSION=1.1.0");

    // Rerun if observation schema changes
    println!("cargo:rerun-if-changed=src/rl/observation.rs");
}
