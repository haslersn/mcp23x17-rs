use spidev::Spidev;
use spidev::SpidevOptions;
use spidev::SpidevTransfer;
use spidev::SPI_MODE_0;
use std::fmt::Debug;
use std::fmt::Formatter;
use std::io::Write;
use std::sync::Arc;
use std::sync::Mutex;

type Result<T = ()> = std::result::Result<T, Box<std::error::Error>>;

const HARDWARE_ADDRESS: u8 = 0;

#[derive(Clone, Copy)]
pub enum IoValue {
    Low,
    High,
}

#[derive(Clone, Copy)]
enum PortLabel {
    Out,
    In,
}

impl PortLabel {
    fn address(&self) -> u8 {
        match self {
            PortLabel::Out => 0x12,
            PortLabel::In => 0x13,
        }
    }
}

#[derive(Clone)]
pub struct Expander {
    spi: Arc<Mutex<Spidev>>,
}

impl Expander {
    pub fn new(device: &str) -> Result<Self> {
        let mut spi = Spidev::open(device)?;
        spi.configure(
            SpidevOptions::new()
                .bits_per_word(8)
                .max_speed_hz(100_000)
                .mode(SPI_MODE_0),
        )?;
        Ok(Expander {
            spi: Arc::new(Mutex::new(spi)),
        })
    }

    pub fn output(&self, pin_num: u8) -> Output {
        Output {
            pin: Pin {
                spi: self.spi.clone(),
                label: PortLabel::Out,
                num: pin_num,
            },
        }
    }

    pub fn input(&self, pin_num: u8) -> Input {
        Input {
            pin: Pin {
                spi: self.spi.clone(),
                label: PortLabel::In,
                num: pin_num,
            },
        }
    }

    pub fn output_byte(&self) -> Result<u8> {
        let spi = &self.spi.lock().unwrap();
        Ok(read_byte(spi, PortLabel::Out)?)
    }

    pub fn input_byte(&self) -> Result<u8> {
        let spi = &self.spi.lock().unwrap();
        Ok(read_byte(spi, PortLabel::In)?)
    }
}

impl Debug for Expander {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        let spi = &self.spi.lock().unwrap();
        let in_byte = read_byte(spi, PortLabel::In)
            .map(|b| b.to_string())
            .unwrap_or("NONE".to_string());
        let out_byte = read_byte(spi, PortLabel::Out)
            .map(|b| b.to_string())
            .unwrap_or("NONE".to_string());
        write!(f, "{{ In: {}, Out: {} }}", in_byte, out_byte)
    }
}

#[derive(Clone)]
pub struct Output {
    pin: Pin,
}

impl Output {
    pub fn to_input(&self) -> Input {
        Input {
            pin: self.pin.clone(),
        }
    }
}

impl Output {
    pub fn set_low(&self) -> Result {
        self.set_value(IoValue::Low)
    }

    pub fn set_high(&self) -> Result {
        self.set_value(IoValue::High)
    }

    pub fn set_value(&self, value: IoValue) -> Result {
        let mut spi = self.pin.spi.lock().unwrap();
        let read_byte = read_byte(&spi, self.pin.label)?;

        // calculate the state to write (write_byte)
        let mask = 1 << self.pin.num;
        let write_byte = match value {
            IoValue::Low => read_byte & !mask,
            IoValue::High => read_byte | mask,
        };

        // write
        if read_byte != write_byte {
            let tx_buf = [write_cmd(), self.pin.label.address(), write_byte];
            spi.write(&tx_buf)?;
        }

        Ok(())
    }
}

#[derive(Clone)]
pub struct Input {
    pin: Pin,
}

impl Input {
    pub fn read_value(&self) -> Result<IoValue> {
        let mask = 1 << self.pin.num;
        let spi = self.pin.spi.lock().unwrap();
        let read = read_byte(&spi, self.pin.label)?;
        Ok(match read & mask {
            0_u8 => IoValue::Low,
            _ => IoValue::High,
        })
    }
}

#[derive(Clone)]
struct Pin {
    spi: Arc<Mutex<Spidev>>,
    label: PortLabel,
    num: u8,
}

fn read_byte(spi: &Spidev, label: PortLabel) -> Result<u8> {
    let tx_buf = [read_cmd(), label.address(), 0];
    let mut rx_buf = [0; 3];
    let mut transfer = SpidevTransfer::read_write(&tx_buf, &mut rx_buf);
    spi.transfer(&mut transfer)?;
    Ok(rx_buf[2])
}

fn write_cmd() -> u8 {
    0x40 | (HARDWARE_ADDRESS << 1) | 0
}

fn read_cmd() -> u8 {
    0x40 | (HARDWARE_ADDRESS << 1) | 1
}
