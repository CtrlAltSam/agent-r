use gfxinfo::active_gpu;

use super::{amd, nvidia};

// Returns the GPU temperature in Celsius.
pub fn get_gpu_temp() -> Option<f32> {
    let gpu = active_gpu().ok()?;
    let vendor = gpu.vendor().to_ascii_lowercase();

    if vendor.contains("amd") || vendor.contains("ati") {
        if let Some(temp) = amd::get_amd_gpu_temp() {
            return Some(temp);
        }
    }

    if vendor.contains("nvidia") {
        if let Some(temp) = nvidia::get_nvidia_gpu_temp() {
            return Some(temp);
        }
    }

    let raw_temp = gpu.info().temperature();

    if raw_temp == 0 {
        return None;
    }

    Some(raw_temp as f32 / 1000.0)
}
