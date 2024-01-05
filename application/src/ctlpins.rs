
use stm32f4xx_hal::gpio::DynamicPin;
use embedded_hal::digital::v2::OutputPin;
use embedded_hal::digital;

// create an enum with 3 possibilities: High, Low, and Floating
// this is used to set the CTL pins to a specific state
#[derive(Copy, Clone)]
pub enum PinState {
    High,
    Low,
    Floating,
}

// the power_on/power_off sequences processed by _run_sequence
// have the following format:
// coma separated orders which could be:
// ord[,ord]*
// where ord is:
//   - a,b,c,d,r followed by a state: h,l,z
//   - w followed by a natural number, which is the number of 100ms to wait
//   - p followed by 0 or 1, which is the desired power state
//  , is ignored and used as a visual separator of orders
//
// i.e. assuming A=REC , B=POWER_BTN for a jetson board, we could have:
//
//   enter flashing mode:
//   "p1,aL,rL,w1,rZ,w1"  => Power on, REC low, reset LOW, wait 100ms, reset HiZ, wait 100ms
//
//   power off via signal:
//   "p1,bL,w110,bZ" => Power on, POWER_BTN low, wait 11s, POWER_BTN HiZ
//
//   power on via signal:
//   "p1,bL,w5,bZ" => Power on, POWER_BTN low, wait 500ms, POWER_BTN HiZ

pub trait CTLPinsTrait {
    fn set_ctl_a(&mut self, state:PinState);
    fn set_ctl_b(&mut self, state:PinState);
    fn set_ctl_c(&mut self, state:PinState);
    fn set_ctl_d(&mut self, state:PinState);
    fn set_reset(&mut self, state:PinState);
    fn power_on(&mut self, on_seq: &[u8]);
    fn power_off(&mut self, off_seq: &[u8]);

}

pub struct CTLPins<PWPin>
where
    PWPin: OutputPin, {
    ctl_a: DynamicPin<'A', 5>,
    stored_a: PinState,
    ctl_b: DynamicPin<'A', 6>,
    stored_b: PinState,
    ctl_c: DynamicPin<'A', 7>,
    stored_c: PinState,
    ctl_d: DynamicPin<'A', 8>,
    stored_d: PinState,
    reset: DynamicPin<'A', 9>,
    stored_reset: PinState,
    power: PWPin,
    on: bool,
}

impl<PWPin> CTLPins<PWPin>
where
    PWPin: OutputPin,
{
    pub fn new(ctl_a:DynamicPin<'A', 5>,
               ctl_b:DynamicPin<'A', 6>,
               ctl_c:DynamicPin<'A', 7>,
               ctl_d:DynamicPin<'A', 8>,
               reset:DynamicPin<'A', 9>,
               power:PWPin) -> Self {
        let mut instance = Self{ctl_a, stored_a: PinState::Floating,
                                ctl_b, stored_b: PinState::Floating,
                                ctl_c, stored_c: PinState::Floating,
                                ctl_d, stored_d: PinState::Floating,
                                reset, stored_reset: PinState::Floating,
                                power, on: false};
        instance.set_ctl_a(PinState::Floating);
        instance.set_ctl_b(PinState::Floating);
        instance.set_ctl_c(PinState::Floating);
        instance.set_ctl_d(PinState::Floating);
        instance.set_reset(PinState::Floating);
        let empty: [u8; 0] = [];
        instance.power_off(&empty);
        instance
    }

    fn _set_ctl_a(&mut self, state:PinState) {
        match state {
            PinState::High      => self.ctl_a.make_push_pull_output_in_state(digital::v2::PinState::High),
            PinState::Low       => self.ctl_a.make_push_pull_output_in_state(digital::v2::PinState::Low),
            PinState::Floating  => self.ctl_a.make_floating_input(),
        }
    }

    fn _set_ctl_b(&mut self, state: PinState) {
        match state {
            PinState::High      => self.ctl_b.make_push_pull_output_in_state(digital::v2::PinState::High),
            PinState::Low       => self.ctl_b.make_push_pull_output_in_state(digital::v2::PinState::Low),
            PinState::Floating  => self.ctl_b.make_floating_input(),
        }
    }

    fn _set_ctl_c(&mut self, state: PinState) {
        match state {
            PinState::High      => self.ctl_c.make_push_pull_output_in_state(digital::v2::PinState::High),
            PinState::Low       => self.ctl_c.make_push_pull_output_in_state(digital::v2::PinState::Low),
            PinState::Floating  => self.ctl_c.make_floating_input(),
        }
    }

    fn _set_ctl_d(&mut self, state: PinState) {
        match state {
            PinState::High      => self.ctl_d.make_push_pull_output_in_state(digital::v2::PinState::High),
            PinState::Low       => self.ctl_d.make_push_pull_output_in_state(digital::v2::PinState::Low),
            PinState::Floating  => self.ctl_d.make_floating_input(),
        }
    }

    fn _set_reset(&mut self, state: PinState) {
        match state {
            PinState::High      => self.reset.make_push_pull_output_in_state(digital::v2::PinState::High),
            PinState::Low       => self.reset.make_push_pull_output_in_state(digital::v2::PinState::Low),
            PinState::Floating  => self.reset.make_floating_input(),
        }
    }

    fn _float_all(&mut self) {
        self._set_ctl_a(PinState::Floating);
        self._set_ctl_b(PinState::Floating);
        self._set_ctl_c(PinState::Floating);
        self._set_ctl_d(PinState::Floating);
        self._set_reset(PinState::Floating);
    }

    fn _float_not_off_tolerant(&mut self) {
        if !off_tolerant(self.stored_a) {
            self._set_ctl_a(PinState::Floating);
        }
        if !off_tolerant(self.stored_b) {
            self._set_ctl_b(PinState::Floating);
        }
        if !off_tolerant(self.stored_c) {
            self._set_ctl_c(PinState::Floating);
        }
        if !off_tolerant(self.stored_d) {
            self._set_ctl_d(PinState::Floating);
        }
        if !off_tolerant(self.stored_reset) {
            self._set_reset(PinState::Floating);
        }
    }

    fn _lower(&self, ch:u8) -> u8 {
        // ensure lowcase ascii
        if (ch>64) && (ch<91) {
            return ch ^0x20;
        }
        return ch
    }
    fn _status_from_u8(&self, ch:u8) -> PinState {
        match self._lower(ch) {
            b'h' => PinState::High,
            b'l' => PinState::Low,
            b'z' => PinState::Floating,
            _ => PinState::Floating,
        }
    }
    fn _run_sequence(&mut self, sequence: &[u8]) {
        let mut p = 0;
        while p + 1 < sequence.len() {
            let pin = self._lower(sequence[p]);

            p+=1;
            match pin {
                b'\0' => break,
                b'a' => { self._set_ctl_a(self._status_from_u8(sequence[p])); p+=1},
                b'b' => { self._set_ctl_b(self._status_from_u8(sequence[p])); p+=1},
                b'c' => { self._set_ctl_c(self._status_from_u8(sequence[p])); p+=1},
                b'd' => { self._set_ctl_d(self._status_from_u8(sequence[p])); p+=1},
                b'r' => { self._set_reset(self._status_from_u8(sequence[p])); p+=1},
                b'w' => p = self._wait(sequence, p),
                b'p' => { let pw = sequence[p];
                          p+=1;
                          if pw == b'1' {
                             self.power_on(&[0u8; 0])
                          } else {
                             self.power_off(&[0u8; 0])
                          }
                        }
                b',' => {}, // ignore commas
                _ => {}, // ignore unknown commands
            }
        }
    }

    fn _wait(&self, sequence: &[u8], mut p: usize) -> usize {
        // parse for 100ms increments
        let mut wait:u32 = 0;
        while p < sequence.len() {
            let ch = sequence[p];
            if (ch < b'0') || (ch > b'9') {
                break;
            }
            wait = wait * 10 + (ch - b'0') as u32;
            p += 1;
        }
        // This value has been calculated experimentally, it would need to be
        // updated for different clock speeds
        cortex_m::asm::delay(3_300_000 * wait);
        p
    }
}

// High output state is not ok when the board is not powered on
// because it will draw power from the output pins into the carried board
fn off_tolerant(state: PinState) -> bool {
    match state {
        PinState::Floating => true,
        PinState::Low => true,
        _ => false,
    }
}

impl<PWPin> CTLPinsTrait for CTLPins<PWPin>
where
    PWPin: OutputPin,
{
    fn set_ctl_a(&mut self, state:PinState) {
        self.stored_a = state;
        if self.on || off_tolerant(state){
            self._set_ctl_a(state)
        }
    }

    fn set_ctl_b(&mut self, state: PinState) {
        self.stored_b = state;
        if self.on || off_tolerant(state){
            self._set_ctl_b(state)
        }
    }

    fn set_ctl_c(&mut self, state: PinState) {
        self.stored_c = state;
        if self.on || off_tolerant(state){
            self._set_ctl_c(state)
        }
    }


    fn set_ctl_d(&mut self, state: PinState) {
        self.stored_d = state;
        if self.on || off_tolerant(state){
            self._set_ctl_d(state)
        }
    }

    fn set_reset(&mut self, state: PinState) {
        self.stored_reset = state;
        if self.on || off_tolerant(state){
            self._set_reset(state)
        }
    }

    fn power_on(&mut self, on_seq: &[u8]) {
        self._set_ctl_a(self.stored_a);
        self._set_ctl_b(self.stored_b);
        self._set_ctl_c(self.stored_c);
        self._set_ctl_d(self.stored_d);
        self._set_reset(self.stored_reset);
        if on_seq.len() == 0 || (on_seq.len()>0 && on_seq[0] == b'\0') {
            self.power.set_high().ok();
        } else {
            self._run_sequence(on_seq);
        }
        self.on = true;
    }

    fn power_off(&mut self, on_seq: &[u8]) {
        if on_seq.len() == 0 || (on_seq.len()>0 && on_seq[0] == b'\0') {
            // we set the control pins to floating while in power off, so power is not drawn
            // from the output pins into the carried board
            self._float_not_off_tolerant();
            self.power.set_low().ok();
        } else {
            self._run_sequence(on_seq);
            self._float_not_off_tolerant();
        }
        self.on = false;
    }
}

