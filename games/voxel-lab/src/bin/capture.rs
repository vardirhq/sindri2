//! Photograph the blocks from four sides.

use std::{error::Error, path::Path};

fn main() -> Result<(), Box<dyn Error>> {
    let out = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "target/render-artifacts".to_owned());
    pollster::block_on(voxel_lab::capture(Path::new(&out)))
}
