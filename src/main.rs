mod register;
mod cpu;
mod opcode;
mod mmu;
mod interupt;
mod timer;
mod memory_bank;

use cpu::CPU;

fn main() {
    let rom = std::fs::read("rom").unwrap();
    let mut cpu = CPU::new(&rom);
    cpu.mmu.stub();
    loop {
        cpu.tick();
    }
}
