




pub trait MemoryBank {
    fn read_memory(&self, address: u16) -> u8;
    fn write_memory(&mut self, address: u16, value: u8);
}

struct MemoryBankZero {
    rom: [u8; 0x8000],
    ram: [u8; 0x2000]
}

impl MemoryBankZero {
    fn new(rom: &[u8]) -> MemoryBankZero {
        let mut memory_bank = Self {
            rom: [0; 0x8000],
            ram: [0; 0x2000]
        };
        for (i, byte) in rom.iter().enumerate() {
            memory_bank.rom[i] = *byte;
        }
        memory_bank
    }
}

impl MemoryBank for MemoryBankZero {
    fn read_memory(&self, address: u16) -> u8 {
        match address {
            0x0 ..= 0x7FFF => self.rom[address as usize],
            0xA000 ..= 0xBFFF => self.ram[address as usize - 0xA000],
            _ => unreachable!("Memory bank accessed outside of valid ranges")
        }
    }

    fn write_memory(&mut self, address: u16, value: u8) {
        match address {
            0x4000 ..= 0x7FFF =>  {}
            0xA000 ..= 0xBFFF => self.ram[address as usize - 0xA000] = value,
            _ => unreachable!("Memory bank accessed outside of valid ranges")
        }
    }
}


enum MemoryBankMode {
    ROM,
    RAM,
}

const ROM_BANK_SIZE: usize = 0x4000;
const ROM_BANK_COUNT: usize = 125;

struct MemoryBankOne {
    base_rom_bank: [u8; ROM_BANK_SIZE],
    rom_banks: [u8; ROM_BANK_SIZE * ROM_BANK_COUNT],
    selected_rom_bank: usize,
    selected_rom_grouping: usize,
    ram_banks: [[u8; 0x2000]; 4],
    ram_enabled: bool,
    selected_ram_bank: usize,
    mode: MemoryBankMode
}

impl MemoryBankOne {
    fn new(rom: &[u8]) -> MemoryBankOne {
        let mut memory_bank = Self {
            base_rom_bank: [0; ROM_BANK_SIZE],
            rom_banks: [0; ROM_BANK_SIZE * ROM_BANK_COUNT],
            selected_rom_bank: 0,
            selected_rom_grouping: 0,
            ram_banks: [[0; 0x2000]; 4],
            ram_enabled: true,
            selected_ram_bank: 0,
            mode: MemoryBankMode::ROM
        };

        for (i, byte) in rom.iter().enumerate() {
            match i {
                0x000 ..= 0x3FFF => memory_bank.base_rom_bank[i] = *byte,
                _ => {
                    memory_bank.rom_banks[i - ROM_BANK_SIZE] = *byte
                }
            }
        }
        memory_bank
    }
}

impl MemoryBank for MemoryBankOne {
    fn read_memory(&self, address: u16) -> u8 {
        match address {
            0x0000 ..= 0x3FFF =>
                self.base_rom_bank[address as usize],
            0x4000 ..= 0x7FFF =>
                self.rom_banks[(self.selected_rom_grouping + self.selected_rom_bank) * ROM_BANK_SIZE + address as usize - ROM_BANK_SIZE],
            0xA000 ..= 0xBFFF => {
                if self.ram_enabled {
                    self.ram_banks[self.selected_ram_bank][address as usize - 0xA000]
                } else {
                    0
                }
            }
            _ => unreachable!("Address {} outisde of memory bank's range", address)

        }
    }

    fn write_memory(&mut self, address: u16, value: u8) {
        match address {
            0x0000 ..= 0x1FFF => {
                self.ram_enabled = (value & 0xF) == 0xA
            }
            0x2000 ..= 0x3FFF => {
                let masked = value & 0b11111;
                if masked == 0 {
                    self.selected_rom_bank = 0;
                } else {
                    self.selected_rom_bank = masked as usize - 1;
                }
            }
            0x4000 ..= 0x5FFF => {
                let selected = value & 0xC0; // 2 high bits
                match self.mode {
                    MemoryBankMode::ROM => self.selected_rom_grouping = selected as usize * 31,
                    MemoryBankMode::RAM => self.selected_ram_bank = selected as usize
                }
            }
            0x6000 ..= 0x7FFF => {
                self.mode = match value {
                    0 => MemoryBankMode::ROM,
                    1 => MemoryBankMode::RAM,
                    _ => panic!("Unsupported mode")
                }
            }
            0xA000 ..= 0xBFFF => {
                if self.ram_enabled {
                    self.ram_banks[self.selected_ram_bank][address as usize - 0xA000] = value
                }
            }
            _ => unreachable!("Address {} outisde of memory bank's range", address)

        }
    }
}

pub fn instantiate_memory_bank(rom: &[u8]) -> Box<dyn MemoryBank> {
    match rom[0x147] {
        0x0 => Box::new(MemoryBankZero::new(rom)),
        0x1 ..= 0x3 => Box::new(MemoryBankOne::new(rom)),
        _ => panic!("Unsupported memory bank type {}", rom[0x147])
    }
}