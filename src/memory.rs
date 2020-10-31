use crate::cpu::FLAG;
use crate::interupt::Interrupt;
use crate::timer::Timer;

pub struct Memory {
    memory: [u8; 0x10000],
    timer: Timer,
    interrupt_e: u8,
    interrupt_f: u8,
}

impl Memory {
    pub fn new() -> Self {
        Memory {
           memory: [0; 0x10000],
           interrupt_e: 0,
           interrupt_f: 0,
           timer: Timer::new()
       }
    }

    pub fn read_memory(&self, address: u16) -> u8 {
        match address {
            0xFF04 ..= 0xFF07 => self.timer.read_byte(address),
            0xFF0F => self.interrupt_f,
            0xFFFF => self.interrupt_e,
            _ => self.memory[address as usize]
        }
    }

    pub fn write_memory(&mut self, address: u16, value: u8) {
        if FLAG {
            match address {
                0xFF04 ..= 0xFF07 =>
                    self.timer.write_byte(address, value),
                0xFF0F => self.interrupt_f = value,
                0xFFFF => self.interrupt_e = value,
                _ => self.memory[address as usize] = value
            }
        } else {
            match address {
                0xFF01 => print!("{}", value as char),
                0xFF04 ..= 0xFF07 =>
                    self.timer.write_byte(address, value),
                0xFF0F => self.interrupt_f = value,
                0xFFFF => self.interrupt_e = value,
                _ => self.memory[address as usize] = value
            }
        }
    }

    pub fn load_rom(&mut self, rom: &[u8]) {
        for (i, byte) in rom.iter().enumerate() {
            self.memory[i] = *byte;
        }
        // println!("{}", self.memory[65348]);
    }

    pub fn get_first_active_interrupt(&self) -> Option<Interrupt> {
        Interrupt::first_from(self.interrupt_e & self.interrupt_f)
    }

    pub fn clear_interrupt(&mut self, interrupt: &Interrupt) {
        let index = interrupt.get_index();
        self.interrupt_f &= !(1 << index);
    }

    pub fn stub(&mut self) {
        self.memory[0xFF44] = 0x90;
    }

    pub fn tick(&mut self, elapsed: u32) {
        if self.timer.tick(elapsed) {
            self.set_timer_interrupt();
        }
    }
    fn set_timer_interrupt(&mut self) {
        self.interrupt_f |= 0b1 << 2;
    }
}

