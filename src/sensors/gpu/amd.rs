#[cfg(windows)]
use adlx::helper::AdlxHelper;

#[cfg(windows)]
pub fn get_amd_gpu_temp() -> Option<f32> {
	let helper = AdlxHelper::new().ok()?;
	let system = helper.system();
	let gpu_list = system.gpus().ok()?;
	let perf = system.performance_monitoring_services().ok()?;

	for gpu in gpu_list.iter() {
		let vendor = gpu.vendor_id().ok()?.to_ascii_lowercase();
		let is_amd = vendor.contains("1002") || vendor.contains("amd");
		if !is_amd {
			continue;
		}

		let metrics = perf.current_gpu_metrics(&gpu).ok()?;
		let temp = metrics.temperature().ok()?;
		if temp > 0.0 {
			return Some(temp as f32);
		}
	}

	None
}

#[cfg(not(windows))]
pub fn get_amd_gpu_temp() -> Option<f32> {
	None
}
