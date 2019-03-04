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

// Mcp23s17 register addresses -- Source:
// https://github.com/piface/pifacecommon/blob/006bca14c18d43ba2d9eafaa84ef83b512c51cf6/pifacecommon/mcp23s17.py#L17
const IODIRA: u8 = 0x0; // I/O direction A
const IODIRB: u8 = 0x1; // I/O direction B
const GPPUB: u8 = 0xD; // port B pullups
const GPIOA: u8 = 0x12; // port A
const GPIOB: u8 = 0x13; // port B

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
            PortLabel::Out => GPIOA,
            PortLabel::In => GPIOB,
        }
    }
}

pub trait Reader {
    fn read_value(&self) -> Result<IoValue>;
}

pub trait Writer: Reader {
    fn set_low(&self) -> Result;
    fn set_high(&self) -> Result;
    fn set_value(&self, value: IoValue) -> Result;
}

pub type Input = Box<Reader + Send>;
pub type Output = Box<Writer + Send>;

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
        write_byte(&mut spi, GPIOA, 0)?;
        write_byte(&mut spi, IODIRA, 0)?; // GPIOA are outputs
        write_byte(&mut spi, IODIRB, 0xFF)?; // GPIOB are input
        write_byte(&mut spi, GPPUB, 0xFF)?; // Enable input pullups
        Ok(Expander {
            spi: Arc::new(Mutex::new(spi)),
        })
    }

    pub fn output(&self, pin_num: u8) -> Output {
        Box::new(Pin {
            spi: self.spi.clone(),
            label: PortLabel::Out,
            num: pin_num,
        })
    }

    pub fn input(&self, pin_num: u8) -> Input {
        Box::new(Pin {
            spi: self.spi.clone(),
            label: PortLabel::In,
            num: pin_num,
        })
    }

    pub fn output_byte(&self) -> Result<u8> {
        let spi = &self.spi.lock().unwrap();
        Ok(read_port(spi, PortLabel::Out)?)
    }

    pub fn input_byte(&self) -> Result<u8> {
        let spi = &self.spi.lock().unwrap();
        Ok(read_port(spi, PortLabel::In)?)
    }
}

impl Debug for Expander {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        let spi = &self.spi.lock().unwrap();
        let in_byte = read_port(spi, PortLabel::In)
            .map(|b| b.to_string())
            .unwrap_or("NONE".to_string());
        let out_byte = read_port(spi, PortLabel::Out)
            .map(|b| b.to_string())
            .unwrap_or("NONE".to_string());
        write!(f, "{{ In: {}, Out: {} }}", in_byte, out_byte)
    }
}

#[derive(Clone)]
pub struct Pin {
    spi: Arc<Mutex<Spidev>>,
    label: PortLabel,
    num: u8,
}

impl Reader for Pin {
    fn read_value(&self) -> Result<IoValue> {
        let mask = 1 << self.num;
        let spi = self.spi.lock().unwrap();
        let read = read_port(&spi, self.label)?;
        Ok(match read & mask {
            0_u8 => IoValue::Low,
            _ => IoValue::High,
        })
    }
}

impl Writer for Pin {
    fn set_low(&self) -> Result {
        self.set_value(IoValue::Low)
    }

    fn set_high(&self) -> Result {
        self.set_value(IoValue::High)
    }

    fn set_value(&self, value: IoValue) -> Result {
        let mut spi = self.spi.lock().unwrap();
        let did_read = read_port(&spi, self.label)?;

        // calculate the state to write (to_write)
        let mask = 1 << self.num;
        let to_write = match value {
            IoValue::Low => did_read & !mask,
            IoValue::High => did_read | mask,
        };

        // write
        if did_read != to_write {
            write_port(&mut spi, self.label, to_write)?;
        }

        Ok(())
    }
}

fn read_port(spi: &Spidev, label: PortLabel) -> Result<u8> {
    read_byte(spi, label.address())
}

fn read_byte(spi: &Spidev, address: u8) -> Result<u8> {
    let tx_buf = [read_cmd(), address, 0];
    let mut rx_buf = [0u8; 3];
    let mut transfer = SpidevTransfer::read_write(&tx_buf, &mut rx_buf);
    spi.transfer(&mut transfer)?;
    Ok(rx_buf[2])
}

fn write_port(spi: &mut Spidev, label: PortLabel, byte: u8) -> Result {
    write_byte(spi, label.address(), byte)
}

fn write_byte(spi: &mut Spidev, address: u8, byte: u8) -> Result {
    let tx_buf = [write_cmd(), address, byte];
    spi.write(&tx_buf)?;
    Ok(())
}

fn write_cmd() -> u8 {
    0x40 | (HARDWARE_ADDRESS << 1) | 0
}

fn read_cmd() -> u8 {
    0x40 | (HARDWARE_ADDRESS << 1) | 1
}
