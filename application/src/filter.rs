
pub trait Filter {
    fn feed(&mut self, value:f32);
    fn get(&mut self) -> f32;
}

const MOVING_AVERAGE_SIZE:usize = 200;
pub struct MovingAverage {
    values: [f32; MOVING_AVERAGE_SIZE],
    sum: f32,
    last_result: f32,
    cached_result: bool,
}

impl MovingAverage {
    pub fn new() -> Self {
        Self{values: [0.0; MOVING_AVERAGE_SIZE], sum: 0.0, last_result: 0.0, cached_result: false}
    }
}

impl Filter for MovingAverage {
    fn feed(&mut self, value:f32) {
        self.sum -= self.values[0];
        self.values.rotate_left(1);
        self.values[MOVING_AVERAGE_SIZE-1] = value;
        self.sum += value;
        self.cached_result = false;
    }
    fn get(&mut self) -> f32 {
        if self.cached_result {
            return self.last_result;
        }
        self.last_result = self.sum / MOVING_AVERAGE_SIZE as f32;
        self.cached_result = true;
        return self.last_result;
    }
}