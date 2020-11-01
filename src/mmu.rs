use crate::interupt::Interrupt;
use crate::timer::Timer;
use crate::memory_bank::{MemoryBank, instantiate_memory_bank};

const WORKING_RAM_SIZE: usize = 0x2000;

pub struct MMU {
    memory_bank: Box<dyn MemoryBank>,
    working_ram: [u8; WORKING_RAM_SIZE],
    memory: [u8; 0x10000],
    timer: Timer,
    interrupt_e: u8,
    interrupt_f: u8,
}

impl MMU {
    pub fn new(rom: &[u8]) -> Self {
        MMU {
            memory_bank: instantiate_memory_bank(rom),
            working_ram: [0; WORKING_RAM_SIZE],
            memory: [0; 0x10000],
            interrupt_e: 0,
            interrupt_f: 0,
            timer: Timer::new()
       }
    }

    pub fn read_memory(&self, address: u16) -> u8 {
        match address {
            0x0000 ..= 0x7FFE | 0xA000 ..= 0xBFFE => self.memory_bank.read_memory(address),
            0xC000 ..= 0xDFFE => self.working_ram[(address as usize) - 0xC000],
            // shadow copy of working ram
            0xE000 ..= 0xFDFE => self.working_ram[(address as usize) - 0xE000],

            0xFF04 ..= 0xFF07 => self.timer.read_byte(address),
            0xFF0F =>
                self.interrupt_f,
            0xFFFF => self.interrupt_e,
            _ => self.memory[address as usize]
        }
    }

    pub fn write_memory(&mut self, address: u16, value: u8) {
        match address {
            0x0000 ..= 0x7FFF | 0xA000 ..= 0xBFFF => self.memory_bank.write_memory(address, value),
            0xC000 ..= 0xDFFF => self.working_ram[(address as usize) - 0xC000] = value,
            // shadow copy of working ram
            0xE000 ..= 0xFDFF => self.working_ram[(address as usize) - 0xE000] = value,
            0xFF01 => print!("{}", value as char),
            0xFF04 ..= 0xFF07 =>
                self.timer.write_byte(address, value),
            0xFF0F => self.interrupt_f = value,
            0xFFFF => self.interrupt_e = value,
            _ => self.memory[address as usize] = value
        }
    }

    pub fn is_interrupt_waiting(&self) -> bool {
        match self.get_first_active_interrupt() {
            Some(_) => true,
            None => false
        }
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

