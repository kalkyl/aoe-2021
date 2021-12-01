use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{BufRead, BufReader, Error};
use std::{thread, time::Duration};
const PORT: &'static str = "/dev/tty.usbmodemTES1";

#[derive(Serialize, Deserialize, Clone, Copy)]
struct Data(u16);

fn main() -> Result<(), Error> {
    let entries = BufReader::new(File::open("./input/1.txt")?)
        .lines()
        .map(|l| l.map(|v| Data(v.parse().unwrap())))
        .collect::<Result<Vec<Data>, _>>()?;

    // let ports = serialport::available_ports().expect("No ports found!");
    // for p in ports {
    //     println!("{}", p.port_name);
    // }

    let mut port = serialport::new(PORT, 1_000_000)
        .timeout(Duration::from_millis(10))
        .open()
        .expect("Failed to open port");

    println!("Sending {} measurements to {}", entries.len(), PORT);

    let mut buf = [0_u8; 4];
    for entry in entries
        .iter()
        .flat_map(|x| postcard::to_slice_cobs(&x, &mut buf).unwrap().to_vec())
        .collect::<Vec<_>>()
        .chunks(64)
    {
        port.write(entry).expect("Write failed!");
        thread::sleep(Duration::from_millis(1));
    }

    println!("Transfer completed!");
    Ok(())
}
