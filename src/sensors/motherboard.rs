#[cfg(windows)]
use serde::Deserialize;

#[cfg(windows)]
use wmi::{COMLibrary, WMIConnection};

#[cfg(windows)]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct ThermalZoneReading {
    current_temperature: Option<i64>,
}

#[cfg(windows)]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct TemperatureProbeReading {
    current_reading: Option<i64>,
}

#[cfg(windows)]
fn kelvin_tenths_to_celsius(kelvin_tenths: i64) -> f32 {
    (kelvin_tenths as f32 / 10.0) - 273.15
}

#[cfg(windows)]
fn reading_tenths_celsius_to_celsius(tenths_celsius: i64) -> f32 {
    tenths_celsius as f32 / 10.0
}

#[cfg(windows)]
pub fn get_motherboard_temp() -> Option<f32> {
    let com = match COMLibrary::new() {
        Ok(com) => com,
        Err(err) => {
            println!("WMI COM initialization failed: {err}");
            return None;
        }
    };

    let wmi_root = match WMIConnection::with_namespace_path("ROOT\\WMI", com) {
        Ok(conn) => conn,
        Err(err) => {
            println!("WMI connection failed for ROOT\\WMI: {err}");
            return None;
        }
    };

    let acpi_query = "SELECT CurrentTemperature FROM MSAcpi_ThermalZoneTemperature";
    let readings: Vec<ThermalZoneReading> = match wmi_root.raw_query(acpi_query) {
        Ok(rows) => rows,
        Err(err) => {
            println!("WMI query failed for MSAcpi_ThermalZoneTemperature: {err}");
            Vec::new()
        }
    };

    let mut acpi_sum = 0.0_f32;
    let mut acpi_count = 0_u32;
    for zone in &readings {
        if let Some(temp) = zone.current_temperature {
            acpi_sum += kelvin_tenths_to_celsius(temp);
            acpi_count += 1;
        }
    }

    if acpi_count > 0 {
        return Some(acpi_sum / acpi_count as f32);
    }

    let com = match COMLibrary::new() {
        Ok(com) => com,
        Err(err) => {
            println!("WMI COM re-initialization failed: {err}");
            return None;
        }
    };

    let wmi_cimv2 = match WMIConnection::with_namespace_path("ROOT\\CIMV2", com) {
        Ok(conn) => conn,
        Err(err) => {
            println!("WMI connection failed for ROOT\\CIMV2: {err}");
            return None;
        }
    };

    let probe_query = "SELECT CurrentReading FROM Win32_TemperatureProbe";
    let probe_readings: Vec<TemperatureProbeReading> = match wmi_cimv2.raw_query(probe_query) {
        Ok(rows) => rows,
        Err(err) => {
            println!("WMI fallback query failed for Win32_TemperatureProbe: {err}");
            return None;
        }
    };

    let mut probe_sum = 0.0_f32;
    let mut probe_count = 0_u32;
    for probe in probe_readings {
        if let Some(reading) = probe.current_reading {
            probe_sum += reading_tenths_celsius_to_celsius(reading);
            probe_count += 1;
        }
    }

    if probe_count > 0 {
        return Some(probe_sum / probe_count as f32);
    }

    None
}

#[cfg(not(windows))]
pub fn get_motherboard_temp() -> Option<f32> {
    None
}
