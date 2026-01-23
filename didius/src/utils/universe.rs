// use anyhow::{anyhow, Result};
use std::io::Read;
use zip::ZipArchive;
use encoding_rs::EUC_KR;
use pyo3::prelude::*;
use std::io::Cursor;

const KOSPI_MST_URL: &str = "https://new.real.download.dws.co.kr/common/master/kospi_code.mst.zip";

// Need to return PyResult for pyfunction
use pyo3::exceptions::PyRuntimeError;

/// Downloads KOSPI Master file and parses KOSPI50 constituents.
/// Returns a list of short stock codes.
#[pyfunction]
pub fn download_kospi_50() -> PyResult<Vec<String>> {
    println!("Downloading KOSPI Master file from {}...", KOSPI_MST_URL);
    
    // 1. Download
    let resp = reqwest::blocking::get(KOSPI_MST_URL)
        .map_err(|e| PyRuntimeError::new_err(format!("Failed to download KOSPI master: {}", e)))?;
    
    let bytes = resp.bytes()
        .map_err(|e| PyRuntimeError::new_err(format!("Failed to get bytes from response: {}", e)))?;
    
    // 2. Unzip
    let reader = Cursor::new(bytes);
    let mut zip = ZipArchive::new(reader)
        .map_err(|e| PyRuntimeError::new_err(format!("Failed to open zip archive: {}", e)))?;
        
    let mut file = zip.by_name("kospi_code.mst")
        .map_err(|e| PyRuntimeError::new_err(format!("kospi_code.mst not found in zip: {}", e)))?;
        
    let mut content = Vec::new();
    file.read_to_end(&mut content)
        .map_err(|e| PyRuntimeError::new_err(format!("Failed to read file content: {}", e)))?;
    
    // 3. Decode CP949 (EUC-KR)
    let (decoded, _, had_errors) = EUC_KR.decode(&content);
    if had_errors {
        println!("Warning: Encoding errors detected while decoding CP949.");
    }
    
    // 4. Parse
    let mut kospi50_codes = Vec::new();
    
    for line in decoded.lines() {
        let len = line.chars().count();
        if len <= 228 {
            continue;
        }
        
        // Part 1 is line[0..len-228]
        // Part 2 is line[len-228..]
        
        // KOSPI50 flag is at offset 20 in Part 2.
        // Offsets in Part 2:
        // GroupCode(2) + MktCap(1) + SecL(4) + SecM(4) + SecS(4) + Mfg(1) + LowLiq(1) + Gov(1) + K200(1) + K100(1) = 20 chars.
        // So the 21st char (index 20) is KOSPI50.
        
        // However, we need to be careful about char indices vs byte indices if there were multibyte chars in Part 2.
        // Fortunately, Part 2 seems to be mostly flags and numbers, which are ASCII.
        // But to be safe, we use chars iterator.
        
        let part2_start_index = len - 228;
        // Optimization: We could just iterate from end?
        // Or just map chars.
        
        // Let's get the chars of the line.
        let chars: Vec<char> = line.chars().collect();
        if chars.len() <= 228 { continue; }
        
        let part2 = &chars[chars.len()-228..];
        
        // Check index 20
        if part2.len() > 20 {
            let k50_flag = part2[20];
            // Check for '1' or 'Y' (Common affirmative flags)
            // Debug print if needed: println!("{} -> {}", chars.iter().take(9).collect::<String>(), k50_flag);
            
            if k50_flag == '1' || k50_flag == 'Y' {
                // Extract Short Code (first 9 chars of Part 1, trimmed)
                let code_part: String = chars.iter().take(9).collect();
                let short_code = code_part.trim().to_string();
                
                // Usually stock codes are 6 digits.
                // Sometimes "A" + 6 digits?
                // Standard code is 6 digits.
                if !short_code.is_empty() {
                    kospi50_codes.push(short_code);
                }
            }
        }
    }
    
    println!("Found {} KOSPI50 constituents.", kospi50_codes.len());
    
    Ok(kospi50_codes)
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(download_kospi_50, m)?)?;
    Ok(())
}
