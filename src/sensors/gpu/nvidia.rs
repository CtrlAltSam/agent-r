#[cfg(windows)]
use nvml_wrapper::{enum_wrappers::device::TemperatureSensor, Nvml};

#[cfg(windows)]
pub fn get_nvidia_gpu_temp() -> Option<f32> {
	let nvml = Nvml::init().ok()?;
	let count = nvml.device_count().ok()?;

	for index in 0..count {
		let device = match nvml.device_by_index(index) {
			Ok(device) => device,
			Err(_) => continue,
		};

		let temp = match device.temperature(TemperatureSensor::Gpu) {
			Ok(temp) => temp,
			Err(_) => continue,
		};

		if temp > 0 {
			return Some(temp as f32);
		}
	}

	None
}

#[cfg(not(windows))]
pub fn get_nvidia_gpu_temp() -> Option<f32> {
	None
}
