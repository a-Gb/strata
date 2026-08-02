//! Writes the checked, license-safe binary inputs used by curated video presets.

use std::{error::Error, fs, io, path::Path};

use strata_test_support::{
    poc_fixtures::{composite_firmware, interleaved_sensor_image, investigation_binary},
    projection_fixtures::projection_golden_fixtures,
};

fn main() -> Result<(), Box<dyn Error>> {
    let destination = Path::new("fixtures/video");
    fs::create_dir_all(destination)?;

    write(
        destination.join("composite-firmware-v1.bin"),
        &composite_firmware()?.bytes,
    )?;
    write(
        destination.join("investigation-xor-v1.bin"),
        &investigation_binary()?.bytes,
    )?;
    write(
        destination.join("interleaved-sensor-v1.bin"),
        &interleaved_sensor_image()?.bytes,
    )?;

    let bitplane = projection_golden_fixtures()
        .into_iter()
        .find(|fixture| fixture.id == "projection-planar-image-v1")
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "missing planar image fixture"))?;
    write(destination.join("bitplane-image-v1.bin"), &bitplane.bytes)?;
    Ok(())
}

fn write(path: impl AsRef<Path>, bytes: &[u8]) -> Result<(), io::Error> {
    let path = path.as_ref();
    fs::write(path, bytes)?;
    println!("wrote {} bytes to {}", bytes.len(), path.display());
    Ok(())
}
