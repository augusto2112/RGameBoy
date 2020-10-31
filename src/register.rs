use std::fmt::{Display, Formatter};

#[derive(Debug)]
pub enum CPUFlag {
    Z = 0x80,
    N = 0x40,
    H = 0x20,
    C = 0x10
}

#[derive(Debug)]
pub struct Registers {
    pub a: u8,
    pub b: u8,
    pub d: u8,
    pub h: u8,
    pub f: u8,
    pub c: u8,
    pub e: u8,
    pub l: u8,
    pub sp: u16,
    pub pc: u16
}

impl Registers {
    pub fn new() -> Registers {
        Self {
            a: 1,
            f: 0xB0,
            b: 0x00,
            c: 0x13,
            d: 0x00,
            e: 0xD8,
            h: 0x01,
            l: 0x4D,
            pc: 0x0100,
            sp: 0xFFFE,
        }
    }

    pub fn set_flag(&mut self, flag: CPUFlag, set: bool) {
        if set {
            self.f |= flag as u8;
        } else {
            self.f &= !(flag as u8)
        }
    }

    pub fn get_flag(&self, flag: CPUFlag) -> u8 {
        if (self.f & flag as u8) != 0 { 1 } else { 0 }
    }

    pub fn af(&self) -> u16 {
        (self.a as u16) << 8 | ((self.f & 0xF0) as u16)
    }

    pub fn write_af(&mut self, value: u16) {
        self.a = (value >> 8) as u8;
        self.f = (value & 0x00F0) as u8;
    }

    pub fn bc(&self) -> u16 {
        (self.b as u16) << 8 | self.c as u16
    }

    pub fn write_bc(&mut self, value: u16) {
        self.b = (value >> 8) as u8;
        self.c = (value & 0xFF) as u8;
    }


    pub fn de(&self) -> u16 {
        (self.d as u16) << 8 | self.e as u16
    }

    pub fn write_de(&mut self, value: u16) {
        self.d = (value >> 8) as u8;
        self.e = (value & 0xFF) as u8;
    }


    pub fn hl(&self) -> u16 {
        (self.h as u16) << 8 | self.l as u16
    }

    pub fn write_hl(&mut self, value: u16) {
        self.h = (value >> 8) as u8;
        self.l = (value & 0xFF) as u8;
    }

}

impl Display for Registers {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "A: {:02X} ", self.a)?;
        write!(f, "F: {:02X} ", self.f)?;

        write!(f, "B: {:02X} ", self.b)?;
        write!(f, "C: {:02X} ", self.c)?;

        write!(f, "D: {:02X} ", self.d)?;
        write!(f, "E: {:02X} ", self.e)?;

        write!(f, "H: {:02X} ", self.h)?;
        write!(f, "L: {:02X} ", self.l)?;
        write!(f, "SP: {:02X} ", self.sp)?;
        write!(f, "PC: 00:{:04X} ", self.pc)
    }
}