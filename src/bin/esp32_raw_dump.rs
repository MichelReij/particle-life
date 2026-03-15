// esp32_raw_dump.rs
// Raw hex dump van wat de ESP32 stuurt, zonder enige verwerking
// Gebruik: cargo run --bin esp32_raw_dump

use serialport::available_ports;
use std::io::Read;
use std::time::Duration;

fn main() {
    println!("🔍 ESP32 Raw Byte Dump - zoekt naar LOLIN-S2-MINI op /dev/ttyACM*");
    println!("======================================================================");

    // Zoek de ESP32 poort
    let port_name = find_esp32_port();
    let port_name = match port_name {
        Some(p) => p,
        None => {
            eprintln!("❌ Geen ESP32 gevonden! Controleer USB-verbinding.");
            std::process::exit(1);
        }
    };

    println!("✅ Gevonden op: {}", port_name);
    println!("📡 Openen op 115200 baud...\n");

    let mut port = serialport::new(&port_name, 115200)
        .timeout(Duration::from_millis(2000))
        .open()
        .expect("Kon seriële poort niet openen");

    println!("📊 Ruwe bytes (eerste 400 bytes, dan pakketten weergeven):");
    println!("----------------------------------------------------------------------");

    // Lees eerst 400 ruwe bytes en dump ze
    let mut raw_buf = [0u8; 400];
    let mut total_read = 0;
    while total_read < 400 {
        match port.read(&mut raw_buf[total_read..]) {
            Ok(0) => break,
            Ok(n) => total_read += n,
            Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => {
                println!("⏱ Timeout na {} bytes", total_read);
                break;
            }
            Err(e) => {
                eprintln!("❌ Leesfout: {}", e);
                break;
            }
        }
    }

    println!("📦 {} bytes ontvangen:", total_read);
    println!();

    // Hex dump met ASCII
    for (i, chunk) in raw_buf[..total_read].chunks(16).enumerate() {
        // Offset
        print!("{:04X}  ", i * 16);
        // Hex bytes
        for (j, byte) in chunk.iter().enumerate() {
            print!("{:02X} ", byte);
            if j == 7 { print!(" "); }
        }
        // Padding als laatste rij korter is
        if chunk.len() < 16 {
            for j in chunk.len()..16 {
                print!("   ");
                if j == 7 { print!(" "); }
            }
        }
        // ASCII
        print!(" |");
        for byte in chunk {
            if *byte >= 32 && *byte < 127 {
                print!("{}", *byte as char);
            } else {
                print!(".");
            }
        }
        println!("|");
    }

    println!();
    println!("======================================================================");
    println!("🔎 Analyse:");

    // Zoek naar 0xAA bytes (mogelijke packet start markers)
    let aa_positions: Vec<usize> = raw_buf[..total_read]
        .iter()
        .enumerate()
        .filter(|(_, &b)| b == 0xAA)
        .map(|(i, _)| i)
        .collect();

    println!("  0xAA (mogelijke start markers) op posities: {:?}", aa_positions);

    // Zoek naar 0x55 bytes (mogelijke packet end markers)
    let x55_positions: Vec<usize> = raw_buf[..total_read]
        .iter()
        .enumerate()
        .filter(|(_, &b)| b == 0x55)
        .map(|(i, _)| i)
        .collect();

    println!("  0x55 (mogelijke end markers) op posities: {:?}", x55_positions);

    // Bereken afstanden tussen opeenvolgende 0xAA's
    if aa_positions.len() >= 2 {
        let distances: Vec<usize> = aa_positions.windows(2).map(|w| w[1] - w[0]).collect();
        println!("  Afstanden tussen 0xAA markers: {:?}", distances);
        println!("  → Pakketgrootte is waarschijnlijk: {} bytes", distances[0]);
    }

    println!();
    println!("======================================================================");
    println!("📋 Volledige bytes als decimaal:");
    for (i, byte) in raw_buf[..total_read].iter().enumerate() {
        print!("{:3}", byte);
        if (i + 1) % 20 == 0 {
            println!("  ← pakket {}", (i + 1) / 20);
        } else {
            print!(", ");
        }
    }
    println!();

    println!();
    println!("======================================================================");
    // Nu continu pakket-voor-pakket weergeven als de pakketgrootte gevonden is
    if aa_positions.len() >= 2 {
        let packet_size = aa_positions[1] - aa_positions[0];
        println!("📡 Live pakketten weergeven ({} bytes per pakket, Ctrl+C om te stoppen):", packet_size);
        println!();

        // Hersynchroniseer op een 0xAA
        let mut sync_buf = [0u8; 1];
        let mut synced = false;
        let mut sync_attempts = 0;
        while !synced && sync_attempts < 1000 {
            if let Ok(1) = port.read(&mut sync_buf) {
                if sync_buf[0] == 0xAA {
                    synced = true;
                    println!("✅ Gesynchroniseerd op 0xAA");
                }
            }
            sync_attempts += 1;
        }

        if !synced {
            println!("⚠️  Kon niet synchroniseren, toch proberen...");
        }

        let mut packet_buf = vec![0u8; packet_size];
        let mut packet_count = 0;

        loop {
            // Lees één volledig pakket
            let mut bytes_read = 0;
            let mut ok = true;
            while bytes_read < packet_size {
                match port.read(&mut packet_buf[bytes_read..]) {
                    Ok(0) => { ok = false; break; }
                    Ok(n) => bytes_read += n,
                    Err(_) => { ok = false; break; }
                }
            }
            if !ok { break; }

            packet_count += 1;

            // Toon pakket
            print!("Pakket {:4}: [", packet_count);
            for (i, byte) in packet_buf.iter().enumerate() {
                if i > 0 { print!(", "); }
                print!("{:3}", byte);
            }
            println!("]");

            // Hex versie
            print!("            [");
            for (i, byte) in packet_buf.iter().enumerate() {
                if i > 0 { print!(", "); }
                print!("{:02X}", byte);
            }
            println!("]");

            // u16 waarden (big-endian paren)
            if packet_size >= 4 {
                print!("            u16 BE: ");
                let mut i = 0;
                // Sla start marker over als het 0xAA is
                let start = if packet_buf[0] == 0xAA { 1 } else { 0 };
                let end = if packet_buf[packet_size - 1] == 0x55 { packet_size - 1 } else { packet_size };
                let data = &packet_buf[start..end];
                while i + 1 < data.len() {
                    let val = u16::from_be_bytes([data[i], data[i+1]]);
                    print!("{}  ", val);
                    i += 2;
                }
                println!();
            }

            println!();

            if packet_count >= 10 {
                println!("(10 pakketten getoond, klaar)");
                break;
            }
        }
    }
}

fn find_esp32_port() -> Option<String> {
    let ports = available_ports().ok()?;
    for port in &ports {
        let name = port.port_name.clone();
        if let serialport::SerialPortType::UsbPort(ref info) = port.port_type {
            let product = info.product.as_deref().unwrap_or("").to_lowercase();
            let manufacturer = info.manufacturer.as_deref().unwrap_or("").to_lowercase();
            if product.contains("lolin") || product.contains("esp32") || product.contains("wemos")
                || manufacturer.contains("wemos") || name.contains("ttyACM") || name.contains("ttyUSB")
            {
                println!("  → {} (VID:{:04X} PID:{:04X} product=\"{}\" mfr=\"{}\")",
                    name, info.vid, info.pid,
                    info.product.as_deref().unwrap_or("?"),
                    info.manufacturer.as_deref().unwrap_or("?"));
                return Some(name);
            }
        }
    }
    // Fallback: probeer ttyACM0
    for port in &ports {
        if port.port_name.contains("ttyACM") || port.port_name.contains("ttyUSB") {
            println!("  → Fallback: {}", port.port_name);
            return Some(port.port_name.clone());
        }
    }
    None
}
