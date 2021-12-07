use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{BufRead, BufReader, Error};
use std::{thread, time::Duration};
const PORT: &'static str = "/dev/tty.usbmodemTES1";

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub enum Direction {
    Up(u8),
    Down(u8),
    Forward(u8),
}

impl Direction {
    pub fn from_string(s: String) -> Self {
        match s.split(' ').collect::<Vec<_>>().as_slice() {
            ["up", d] => Direction::Up(d.parse().unwrap()),
            ["down", d] => Direction::Down(d.parse().unwrap()),
            ["forward", d] => Direction::Forward(d.parse().unwrap()),
            _ => unreachable!(),
        }
    }
}

fn main() -> Result<(), Error> {
    let entries = BufReader::new(File::open("./input/2.txt")?)
        .lines()
        .map(|l| l.map(Direction::from_string))
        .collect::<Result<Vec<Direction>, _>>()?;

    let ports = serialport::available_ports().expect("No ports found!");
    for p in ports {
        println!("{}", p.port_name);
    }

    let mut port = serialport::new(PORT, 1_000_000)
        .timeout(Duration::from_millis(10))
        .open()
        .expect("Failed to open port");

    println!("Sending {} directions to {}", entries.len(), PORT);

    let mut buf = [0_u8; 4];
    for entry in entries
        .iter()
        .flat_map(|x| postcard::to_slice_cobs(&x, &mut buf).unwrap().to_vec())
        .collect::<Vec<_>>()
        .chunks(8)
    {
        port.write(entry).expect("Write failed!");
        thread::sleep(Duration::from_millis(2));
    }

    println!("Transfer completed!");
    Ok(())
}
