fn main() -> anyhow::Result<()> {
    let config = meson_ext_rs::find_meson()?;
    println!("Found meson: {}", config.meson_version());

    Ok(())
}
