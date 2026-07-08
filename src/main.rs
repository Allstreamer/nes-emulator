struct CPU {
    halted: bool,
    current_step_cycles: u32,

    program_counter: u16,
    stack_pointer: u8,
    a_reg: u8,
    x_reg: u8,
    y_reg: u8,

    flag_carry: bool,
    flag_zero: bool,
    flag_interrupt_disable: bool,
    flag_decimal: bool,
    flag_overflow: bool,
    flag_negative: bool,

    header: [u8; 0x10],
    ram: [u8; 0x800],
    rom: [u8; 0x8000],
}

impl CPU {
    fn new() -> Self {
        Self {
            halted: false,
            current_step_cycles: 0,

            program_counter: 0,
            stack_pointer: 0xFD,
            a_reg: 0,
            x_reg: 0,
            y_reg: 0,

            flag_carry: false,
            flag_zero: false,
            flag_interrupt_disable: true,
            flag_decimal: false,
            flag_overflow: false,
            flag_negative: false,

            header: [0; 0x10],
            ram: [0; 0x800],
            rom: [0; 0x8000],
        }
    }

    fn reset(&mut self, rom_file: &[u8]) {
        // Clear ram
        self.ram = [0; 0x800];

        // Setup ROM from file
        self.rom[0x00..0x8000].clone_from_slice(&rom_file[0x10..0x8010]);
        self.header.copy_from_slice(&rom_file[0x0..0x10]);

        // Read PC
        let pc_lower = self.read(0xFFFC);
        let pc_upper = self.read(0xFFFD);
        self.program_counter = ((pc_upper as u16) << 8) + pc_lower as u16;

        // Setup Stack Stack pointer
        self.stack_pointer = 0xFD;

        // Reset flags
        self.flag_interrupt_disable = true;
    }

    fn read(&self, address: u16) -> u8 {
        if address < 0x2000 {
            // Mirrored RAM
            self.ram[(address & 0x07FF) as usize]
        } else if address >= 0x8000 {
            self.rom[(address - 0x8000) as usize]
        } else {
            todo!("read not implmented: {}", address)
        }
    }

    fn write(&mut self, address: u16, value: u8) {
        println!("Writing: ${:02x} -> ${:04x}", value, address);
        if address < 0x2000 {
            // Mirrored RAM
            self.ram[(address & 0x07FF) as usize] = value;
        } else if address >= 0x8000 {
            panic!(
                "Trying to write to rom at address: {} with value: {}",
                address, value
            );
        } else {
            todo!("write not implmented: {} with value: {}", address, value)
        }
    }

    fn read_from_and_advance_pc(&mut self) -> u8 {
        let result = self.read(self.program_counter);
        self.program_counter += 1;
        result
    }

    fn branch_if(&mut self, condition: bool) {
        let branch_addr = self.read_from_and_advance_pc();
        self.current_step_cycles += 1;

        if condition {
            let signed_branch_addr = branch_addr as i8 as i16;

            let old_pc = self.program_counter;

            self.program_counter = self.program_counter.wrapping_add_signed(signed_branch_addr);

            // Lower PC write
            self.current_step_cycles += 1;

            // Possible upper PC write
            if (old_pc & 0xFF00) != (self.program_counter & 0xFF00) {
                self.current_step_cycles += 1;
            }
        }
    }

    fn update_zero_and_negative_flags(&mut self, result: u8) {
        self.flag_zero = result == 0;
        self.flag_negative = (result & 0b1000_0000) != 0;
    }

    fn push(&mut self, value: u8) {
        self.write(0x100 + self.stack_pointer as u16, value);
        self.stack_pointer = self.stack_pointer.wrapping_sub(1);
    }

    fn pull(&mut self) -> u8 {
        self.stack_pointer = self.stack_pointer.wrapping_add(1);
        self.read(0x100 + self.stack_pointer as u16)
    }

    fn status_as_bytes(&self, b_flag: bool) -> u8 {
        let mut status: u8 = 0;
        if self.flag_carry {
            status |= 1 << 0;
        }
        if self.flag_zero {
            status |= 1 << 1;
        }
        if self.flag_interrupt_disable {
            status |= 1 << 2;
        }
        if self.flag_decimal {
            status |= 1 << 3;
        }

        if b_flag {
            status |= 1 << 4;
        }
        status |= 1 << 5; // Bit 5: Always 1

        if self.flag_overflow {
            status |= 1 << 6;
        }
        if self.flag_negative {
            status |= 1 << 7;
        }

        status
    }

    fn set_status_from_byte(&mut self, status: u8) {
        self.flag_carry = (status & (1 << 0)) != 0;
        self.flag_zero = (status & (1 << 1)) != 0;
        self.flag_interrupt_disable = (status & (1 << 2)) != 0;
        self.flag_decimal = (status & (1 << 3)) != 0;

        self.flag_overflow = (status & (1 << 6)) != 0;
        self.flag_negative = (status & (1 << 7)) != 0;
    }

    fn step(&mut self) {
        let op_code = self.read_from_and_advance_pc();
        // Take one cycle to read
        self.current_step_cycles = 1;

        match op_code {
            // HLT
            0x02 => {
                self.halted = true;
            }
            // PHP
            0x08 => {
                self.push(self.status_as_bytes(true));
                self.current_step_cycles += 2;
            }
            // BPL
            0x10 => self.branch_if(!self.flag_negative),
            // CLC
            0x18 => {
                self.flag_carry = false;
                self.current_step_cycles += 1;
            }

            // JSR
            0x20 => {
                let jmp_low = self.read_from_and_advance_pc();
                let jmp_high = self.read(self.program_counter);
                self.push((self.program_counter >> 8) as u8);
                self.push((self.program_counter & 0xFF) as u8);
                self.program_counter = ((jmp_high as u16) << 8) | jmp_low as u16;
                self.current_step_cycles += 5;
            }

            // PLP
            0x28 => {
                let status = self.pull();
                self.set_status_from_byte(status);

                self.current_step_cycles += 3;
            }

            // BMI
            0x30 => self.branch_if(self.flag_negative),

            // SEC
            0x38 => {
                self.flag_carry = true;
                self.current_step_cycles += 1;
            }

            // PHA
            0x48 => {
                self.push(self.a_reg);
                self.current_step_cycles += 2;
            }

            // JMP
            0x4C => {
                let jmp_low = self.read_from_and_advance_pc();
                let jmp_high = self.read(self.program_counter);
                self.program_counter = ((jmp_high as u16) << 8) | jmp_low as u16;
                self.current_step_cycles += 2;
            }

            // BVC (Overflow Clear)
            0x50 => self.branch_if(!self.flag_overflow),
            // CLI
            0x58 => {
                // Delay by one cycle
                self.flag_interrupt_disable = false;
                self.current_step_cycles += 1;
            }

            // RTS
            0x60 => {
                let jmp_low = self.pull();
                let jmp_high = self.pull();
                self.program_counter = ((jmp_high as u16) << 8) | jmp_low as u16;
                self.program_counter += 1;
                self.current_step_cycles += 5;
            }

            // PLA
            0x68 => {
                let result = self.pull();
                self.a_reg = result;
                self.update_zero_and_negative_flags(result);

                self.current_step_cycles += 3;
            }

            // BVS (Overflow Set)
            0x70 => self.branch_if(self.flag_overflow),
            // SEI
            0x78 => {
                // Needs to be delayed by 1 cycle?
                self.flag_interrupt_disable = true;
                self.current_step_cycles += 1;
            }
            // STY Zero Page
            0x84 => {
                let write_addr = self.read_from_and_advance_pc();
                self.current_step_cycles += 1;

                self.write(write_addr as u16, self.y_reg);
                self.current_step_cycles += 1;
            }
            // STA Zero Page
            0x85 => {
                let write_addr = self.read_from_and_advance_pc();
                self.current_step_cycles += 1;

                self.write(write_addr as u16, self.a_reg);
                self.current_step_cycles += 1;
            }
            // STX Zero Page
            0x86 => {
                let write_addr = self.read_from_and_advance_pc();
                self.current_step_cycles += 1;

                self.write(write_addr as u16, self.x_reg);
                self.current_step_cycles += 1;
            }
            // DEY
            0x88 => {
                self.y_reg = self.y_reg.wrapping_sub(1);
                self.update_zero_and_negative_flags(self.y_reg);
                self.current_step_cycles += 1;
            }
            // TXA
            0x8A => {
                self.a_reg = self.x_reg;
                self.update_zero_and_negative_flags(self.a_reg);
                self.current_step_cycles += 1;
            }
            // STY Absoulute
            0x8C => {
                let lower_addr = self.read_from_and_advance_pc();
                self.current_step_cycles += 1;

                let upper_addr = self.read_from_and_advance_pc();
                self.current_step_cycles += 1;

                self.write(((upper_addr as u16) << 8) | lower_addr as u16, self.y_reg);
                self.current_step_cycles += 1;
            }
            // STA Absoulute
            0x8D => {
                let lower_addr = self.read_from_and_advance_pc();
                self.current_step_cycles += 1;

                let upper_addr = self.read_from_and_advance_pc();
                self.current_step_cycles += 1;

                self.write(((upper_addr as u16) << 8) | lower_addr as u16, self.a_reg);
                self.current_step_cycles += 1;
            }
            // STX Absoulute
            0x8E => {
                let lower_addr = self.read_from_and_advance_pc();
                self.current_step_cycles += 1;

                let upper_addr = self.read_from_and_advance_pc();
                self.current_step_cycles += 1;

                self.write(((upper_addr as u16) << 8) | lower_addr as u16, self.x_reg);
                self.current_step_cycles += 1;
            }
            // BCC
            0x90 => self.branch_if(!self.flag_carry),
            // TYA
            0x98 => {
                self.a_reg = self.y_reg;
                self.update_zero_and_negative_flags(self.a_reg);
                self.current_step_cycles += 1;
            }
            // TXS
            0x9A => {
                self.stack_pointer = self.x_reg;
                self.current_step_cycles += 1;
            }
            // LDY Imm
            0xA0 => {
                let result = self.read_from_and_advance_pc();
                self.current_step_cycles += 1;

                self.y_reg = result;
                self.update_zero_and_negative_flags(result);
            }
            // LDX Imm
            0xA2 => {
                let result = self.read_from_and_advance_pc();
                self.current_step_cycles += 1;

                self.x_reg = result;
                self.update_zero_and_negative_flags(result);
            }
            // LDA Zero Page
            0xA5 => {
                let read_addr = self.read_from_and_advance_pc();
                self.current_step_cycles += 1;

                let result = self.read(read_addr as u16);
                self.current_step_cycles += 1;

                self.a_reg = result;
                self.update_zero_and_negative_flags(result);
            }
            // LDA Absoulute
            0xAD => {
                let lower_addr = self.read_from_and_advance_pc();
                self.current_step_cycles += 1;

                let upper_addr = self.read_from_and_advance_pc();
                self.current_step_cycles += 1;

                let addr = ((upper_addr as u16) << 8) | lower_addr as u16;
                let result = self.read(addr);
                self.current_step_cycles += 1;

                self.a_reg = result;
                self.update_zero_and_negative_flags(result);
            }
            // TAY
            0xA8 => {
                self.y_reg = self.a_reg;
                self.update_zero_and_negative_flags(self.y_reg);
                self.current_step_cycles += 1;
            }
            // LDA Imm
            0xA9 => {
                let result = self.read_from_and_advance_pc();
                self.current_step_cycles += 1;

                self.a_reg = result;
                self.update_zero_and_negative_flags(result);
            }
            // TAX
            0xAA => {
                self.x_reg = self.a_reg;
                self.update_zero_and_negative_flags(self.x_reg);
                self.current_step_cycles += 1;
            }
            // BCS
            0xB0 => self.branch_if(self.flag_carry),
            // CLV
            0xB8 => {
                self.flag_overflow = false;
                self.current_step_cycles += 1;
            }
            // TSX
            0xBA => {
                self.x_reg = self.stack_pointer;
                self.update_zero_and_negative_flags(self.x_reg);
                self.current_step_cycles += 1;
            }
            // INY
            0xC8 => {
                self.y_reg = self.y_reg.wrapping_add(1);
                self.update_zero_and_negative_flags(self.y_reg);
                self.current_step_cycles += 1;
            }
            // DEX
            0xCA => {
                self.x_reg = self.x_reg.wrapping_sub(1);
                self.update_zero_and_negative_flags(self.x_reg);
                self.current_step_cycles += 1;
            }
            // BNE
            0xD0 => self.branch_if(!self.flag_zero),
            // CLD
            0xD8 => {
                self.flag_decimal = false;
                self.current_step_cycles += 1;
            }
            // INX
            0xE8 => {
                self.x_reg = self.x_reg.wrapping_add(1);
                self.update_zero_and_negative_flags(self.x_reg);
                self.current_step_cycles += 1;
            }
            // NOP
            0xEA => {
                self.current_step_cycles += 1;
            }
            // BEQ
            0xF0 => self.branch_if(self.flag_zero),
            // SED
            0xF8 => {
                self.flag_decimal = true;
                self.current_step_cycles += 1;
            }
            _ => {
                todo!("Unimplemented op_code: ${:02x}", op_code);
            }
        }
    }
}

fn main() {
    let raw_rom = include_bytes!("../TestRoms/5_Instructions1.nes");
    let mut cpu = CPU::new();
    cpu.reset(raw_rom);
    println!("Starting PC: ${:x}", cpu.program_counter);

    while !cpu.halted {
        cpu.step();
    }
    println!("PC: ${:04x}", cpu.program_counter);
    println!("A: ${:02x}", cpu.a_reg);
    println!("X: ${:02x}", cpu.x_reg);
    println!("Y: ${:02x}", cpu.y_reg);
}
